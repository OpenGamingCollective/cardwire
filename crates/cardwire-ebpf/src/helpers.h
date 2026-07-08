static __always_inline int is_blocked_device(struct dentry *d)
{
	if (!d) {
		return 0;
	}
	// if it's cardwired, exit
	char comm[16] = {};
	bpf_get_current_comm(comm, sizeof(comm));
	if (__builtin_memcmp(comm, "cardwired", 9) == 0) {
		return 0;
	}
	// get pid and ppid
	__u32 pid = bpf_get_current_pid_tgid() >> 32;
	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	__u32 ppid = BPF_CORE_READ(task, real_parent, tgid);
	// key 0 contain cardwire pid, if ppid = cardwire allow
	__u8 cardwire_key = 0;
	__u32 *cardwire_pid = bpf_map_lookup_elem(&CARDWIRE_PID, &cardwire_key);
	if (cardwire_pid && *cardwire_pid == pid) {
		return 0;
	}

	// same for udev
	if (__builtin_memcmp(comm, "(udev-worker)", 13) == 0) {
		return 0;
	}
	bool blocked = false;

	struct inode *inode = BPF_CORE_READ(d, d_inode);
	// Match card/render/nvidia minor
	if (inode) {
		__u64 d_ino = BPF_CORE_READ(inode, i_ino);
		if (d_ino && bpf_map_lookup_elem(&BLOCKED_INODES, &d_ino)) {
			blocked = true;
			goto end;
		}
	}

end:
	if (!blocked) {
		return 0;
	}
	// get mode
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
	// if map lookup fails, or we are not blocking, or it's hybrid mode, allow
	if (!mode || *mode == 1) {
		return 0;
	}

	// if is hybrid/manual mode, block
	if (*mode == 0 || *mode == 2) {
		return -ENOENT;
	}

	// if smart, check the pid list
	if (*mode == 3) {
		if (!bpf_map_lookup_elem(&ALLOWED_PID, &pid) &&
		    !bpf_map_lookup_elem(&ALLOWED_PID, &ppid)) {
			// Neither pid nor ppid is allowed, block
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
	__u64 d_inode = 0;
	bpf_probe_read(&d_inode, sizeof(d_inode), &dirent->d_ino);
	if (!d_inode)
		return 0;
	bpf_probe_read(&data->d_reclen, sizeof(data->d_reclen),
		       &dirent->d_reclen);

	//Read the name of this entry
	char dirname[64] = {};
	bpf_probe_read_user_str(dirname, sizeof(dirname), dirent->d_name);

	// Check if this is a file we want to hide
	if (bpf_map_lookup_elem(&BLOCKED_INODES, &d_inode)) {
		if (data->last_visible_bpos != 0xFFFFFFFF) {
			struct linux_dirent64 *visible_dirent =
				(struct linux_dirent64
					 *)(data->dirents_buf +
					    data->last_visible_bpos);
			__u16 visible_reclen;
			bpf_probe_read(&visible_reclen, sizeof(visible_reclen),
				       &visible_dirent->d_reclen);

			__u16 new_reclen = visible_reclen + data->d_reclen;

			// Overwrite the visible file's length so it skips over the hidden file
			bpf_probe_write_user(&visible_dirent->d_reclen,
					     &new_reclen, sizeof(new_reclen));
		}

		data->bpos += data->d_reclen;
		return 0; // Continue loop
	}

	// Not a hidden file, update last_visible_bpos and advance
	data->last_visible_bpos = data->bpos;
	data->bpos += data->d_reclen;
	return 0; // Continue loop
}

static __always_inline int is_hybrid()
{
	// get current cardwired mode, key should always be 0
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
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
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
	if (!mode) {
		return false;
	}
	//if mode is smart, true
	if (*mode == 3) {
		return true;
	}
	return false;
}