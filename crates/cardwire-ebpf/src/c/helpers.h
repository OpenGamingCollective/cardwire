
static __always_inline int is_hybrid()
{
	// get current cardwired mode, key should always be 0
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&cw_mode, &key);
	if (!mode) {
		return false;
	}
	//if mode is hybrid, return true
	if (*mode == 1) {
		return true;
	}
	return false;
}

static __always_inline int is_smart()
{
	// get current cardwired mode, key should always be 0
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&cw_mode, &key);
	if (!mode) {
		return false;
	}
	//if mode is smart, true
	if (*mode == 3) {
		return true;
	}
	return false;
}

static __always_inline int is_cardwire_process(__u32 pid)
{
	// key 0 contain cardwire pid, if pid/ppid = cardwire's pid then allow
	__u8 cardwire_key = 0;
	__u32 *cardwire_pid =
		bpf_map_lookup_elem(&cw_daemon_pid, &cardwire_key);
	if (cardwire_pid && *cardwire_pid == pid) {
		return true;
	}
	return false;
}

/// get if the process is whitelisted using comm name
static __always_inline int is_process_whitelisted()
{
	char comm[16] = {};
	bpf_get_current_comm(comm, sizeof(comm));
	if (bpf_map_lookup_elem(&cw_allowed_comm, &comm)) {
		return true;
	}
	return false;
}

/// check if the pid is in the allow list, smart mode only
static __always_inline int is_pid_allowed(__u32 pid, __u32 ppid)
{
	return bpf_map_lookup_elem(&cw_allowed_pid, &pid) ||
	       bpf_map_lookup_elem(&cw_allowed_pid, &ppid);
}

/// check if experimental nvidia blocking is enabled
static __always_inline int is_nvidia_enabled()
{
	__u8 key = 0;
	__u8 *value = bpf_map_lookup_elem(&cw_settings, &key);
	if (!value)
		return false;
	return *value;
}

static __always_inline int is_blocked_device(struct dentry *d)
{
	if (!d) {
		return 0;
	}
	// get pid and ppid
	__u32 pid = bpf_get_current_pid_tgid() >> 32;
	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	__u32 ppid = BPF_CORE_READ(task, real_parent, tgid);

	// if it's cardwire skip it
	if (is_cardwire_process(pid))
		return 0;
	// skip if whitelisted
	if (is_process_whitelisted())
		return 0;

	bool blocked = false;

	struct inode *inode = BPF_CORE_READ(d, d_inode);
	__u8 *map_val = 0;
	// Match card/render/nvidia minor
	if (inode) {
		__u64 d_ino = BPF_CORE_READ(inode, i_ino);
		if (d_ino) {
			map_val = bpf_map_lookup_elem(&cw_blocked_ino, &d_ino);
			// if inode is in the map, blocked
			// we store the value to identify the dGPU/iGPU
			if (map_val) {
				blocked = true;
				goto end;
			}
			if (is_nvidia_enabled() &&
			    bpf_map_lookup_elem(&cw_exp_blk_ino, &d_ino)) {
				blocked = true;
				goto end;
			}
		}
	}
end:

	// If not blocked, return 0(allowed)
	if (!blocked) {
		return 0;
	}
	// get mode
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&cw_mode, &key);
	// if map lookup fails, or we are not blocking, or it's hybrid mode, allow
	if (!mode || *mode == 1) {
		return 0;
	}

	// if is integrated/manual mode, block
	if (*mode == 0 || *mode == 2) {
		return -ENOENT;
	}

	// if smart, check the pid list
	if (*mode == 3) {
		// Check if the inode is linked to the iGPU
		if (map_val && *map_val == 0) {
			// Check if the PID is in the map
			__u8 *allow_map_value_pid =
				bpf_map_lookup_elem(&cw_allowed_pid, &pid);

			// if the isn't in the map, check the ppid
			if (!allow_map_value_pid) {
				allow_map_value_pid = bpf_map_lookup_elem(
					&cw_allowed_pid, &ppid);
			}
			// IF the inode is linked to a iGPU and the map_key exist
			if (allow_map_value_pid) {
				// This mean the inode is allowed, but to check if we should force the dGPU or not,value should be at 0
				if (*allow_map_value_pid == 0) {
					// We should hide the iGPU
					return -ENOENT;
				}
			}
			return 0;
		}

		if (!bpf_map_lookup_elem(&cw_allowed_pid, &pid) &&
		    !bpf_map_lookup_elem(&cw_allowed_pid, &ppid)) {
			// Neither pid nor ppid is allowed, block and report the event
			// Reserve space, return if it fails
			struct report_t *rb_report = bpf_ringbuf_reserve(
				&cw_report_events, sizeof(struct report_t), 0);
			if (!rb_report) {
				return -ENOENT;
			}
			// Store the pid and the comm inside the event
			rb_report->pid = pid;
			bpf_get_current_comm(rb_report->comm,
					     sizeof(rb_report->comm));
			bpf_ringbuf_submit(rb_report, 0);
			return -ENOENT;
		}
	}
	return 0;
}

static __always_inline int patch_dirent_if_found(__u32 _,
						 struct dirents_data_t *data)
{
	// Check if we reached the end of the buffer
	if (data->bpos >= data->buff_size) {
		return 1; // 1 = stop loop
	}

	// Get the current directory entry
	struct linux_dirent64 *dirent =
		(struct linux_dirent64 *)(data->dirents_buf + data->bpos);

	if (bpf_probe_read(&data->d_reclen, sizeof(data->d_reclen),
			   &dirent->d_reclen) < 0) {
		return 1; // Read error, break loop
	}

	__u64 d_inode = 0;
	if (bpf_probe_read(&d_inode, sizeof(d_inode), &dirent->d_ino) < 0) {
		return 1; // Read error, break loop
	}

	if (!d_inode) {
		data->bpos += data->d_reclen;
		return 0; // Skip and continue
	}

	//Read the name of this entry
	char dirname[64] = {};
	bpf_probe_read_user_str(dirname, sizeof(dirname), dirent->d_name);

	// Check if this is a file we want to hide
	__u8 *map_val = bpf_map_lookup_elem(&cw_blocked_ino, &d_inode);
	if (map_val || (is_nvidia_enabled() &&
			bpf_map_lookup_elem(&cw_exp_blk_ino, &d_inode))) {
		if (data->last_visible_bpos != 0xFFFFFFFF) {
			struct linux_dirent64 *visible_dirent =
				(struct linux_dirent64
					 *)(data->dirents_buf +
					    data->last_visible_bpos);
			__u16 visible_reclen;
			bpf_probe_read(&visible_reclen, sizeof(visible_reclen),
				       &visible_dirent->d_reclen);

			__u16 new_reclen = visible_reclen + data->d_reclen;
			// check for iGPU
			if (is_smart()) {
				// Get the process pid and ppid
				__u32 pid = bpf_get_current_pid_tgid() >> 32;
				struct task_struct *task =
					(struct task_struct *)
						bpf_get_current_task();
				__u32 ppid =
					BPF_CORE_READ(task, real_parent, tgid);

				// Read the map to check if the pid is present
				__u8 *allow_map_value = bpf_map_lookup_elem(
					&cw_allowed_pid, &pid);
				// if pid is not present, try with ppid
				if (!allow_map_value) {
					allow_map_value = bpf_map_lookup_elem(
						&cw_allowed_pid, &ppid);
				}
				// check if it's an inode linked to the iGPU
				if (map_val && *map_val == 0) {
					// It's the iGPU
					if (allow_map_value) {
						// PID exist and value isn't 1 (dGPU)
						if (*allow_map_value != 1) {
							// force dGPU is active: hide the iGPU
							bpf_probe_write_user(
								&visible_dirent
									 ->d_reclen,
								&new_reclen,
								sizeof(new_reclen));
							goto end;
						} else {
							// allowed process, no force: show iGPU
							goto not_hidden;
						}
					} else {
						goto not_hidden;
					}
				} else {
					// It's the dGPU
					if (allow_map_value) {
						// Allowed process: must see the dGPU!
						goto not_hidden;
					}
					// Unallowed process: fall through to hide the dGPU
				}
			}
			// Overwrite the visible file's length so it skips over the hidden file
			bpf_probe_write_user(&visible_dirent->d_reclen,
					     &new_reclen, sizeof(new_reclen));
		}
end:
		data->bpos += data->d_reclen;
		// Reserve space, return if it fails
		struct report_t *rb_report = bpf_ringbuf_reserve(
			&cw_report_events, sizeof(struct report_t), 0);
		if (!rb_report) {
			bpf_printk("bpf_ringbuf_reserve failed\n");
			return 0;
		}
		// Store the pid and the comm inside the event
		rb_report->pid = bpf_get_current_pid_tgid() >> 32;
		bpf_get_current_comm(rb_report->comm, sizeof(rb_report->comm));
		bpf_ringbuf_submit(rb_report, 0);
		return 0; // Continue loop
	}

not_hidden:
	// Not a hidden file, update last_visible_bpos and advance
	data->last_visible_bpos = data->bpos;
	data->bpos += data->d_reclen;
	return 0; // Continue loop
}
