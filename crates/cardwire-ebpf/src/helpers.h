static __always_inline int get_pci_addr(struct dentry *dentry, char *pci_addr,
					int size)
{
	struct dentry *parent;
	const unsigned char *parent_name;
	int ret;

	if (!dentry)
		return 1;

	parent = BPF_CORE_READ(dentry, d_parent);
	if (!parent)
		return 1;

	parent_name = BPF_CORE_READ(parent, d_name.name);
	ret = bpf_core_read_str(pci_addr, size, parent_name);

	// PCI address string is 12 chars + 1 null byte
	if (ret < 13)
		return 1;

	// Check for PCI address format (eg: 0000:00:00.0)
	if (pci_addr[4] == ':' && pci_addr[7] == ':' && pci_addr[10] == '.') {
		return 0;
	}

	return 1;
}
static __always_inline int check_backlight_path(struct dentry *dentry)
{
	if (!dentry)
		return 0;

	// current dir/file
	struct qstr q = BPF_CORE_READ(dentry, d_name);
	char name[16] = {};
	if (bpf_core_read_str(name, sizeof(name), q.name) < 0) {
		return 0;
	}

	// get parent folder
	char p_name[16] = {};
	struct dentry *parent = BPF_CORE_READ(dentry, d_parent);
	if (parent) {
		const unsigned char *p_name_ptr =
			BPF_CORE_READ(parent, d_name.name);
		bpf_core_read_str(p_name, sizeof(p_name), p_name_ptr);
	}

	// NVIDIA
	char *t = (__builtin_memcmp(name, "nvidia_", 7) == 0)	? name :
		  (__builtin_memcmp(p_name, "nvidia_", 7) == 0) ? p_name :
								  NULL;

	if (t) {
		if (t[7] >= '0' && t[7] <= '9') {
			__u32 id = 0;

#pragma unroll
			for (int i = 7; i < 10 && t[i] >= '0' && t[i] <= '9';
			     i++) {
				id = id * 10 + (t[i] - '0');
			}

			if (bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id)) {
				return 1;
			}
		}
	}

	return 0;
}

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
	// same for udev
	if (__builtin_memcmp(comm, "(udev-worker)", 13) == 0) {
		return 0;
	}
	bool blocked = false;

	struct inode *inode = BPF_CORE_READ(d, d_inode);
	// Match card/render/nvidia minor
	if (inode) {
		__u64 d_ino = BPF_CORE_READ(inode, i_ino);
		if (d_ino && d_ino == 431) {
			bpf_printk("found this number: %d", d_ino);
		}
		__u16 i_mode = BPF_CORE_READ(inode, i_mode);
		if ((i_mode & 00170000) == 00020000) {
			__u32 i_rdev = BPF_CORE_READ(inode, i_rdev);
			unsigned int major = i_rdev >> 20;
			unsigned int minor = i_rdev & 0xFFFFF;
			if (major == 226) {
				__u32 id = minor;
				if (bpf_map_lookup_elem(&BLOCKED_CARDID, &id)) {
					blocked = true;
					goto end;
				}
				if (bpf_map_lookup_elem(&BLOCKED_RENDERID,
							&id)) {
					blocked = true;
					goto end;
				}
			} else if (major == 195) {
				__u32 id = minor;
				if (bpf_map_lookup_elem(&BLOCKED_NVIDIAID,
							&id)) {
					blocked = true;
					goto end;
				}
			}
		}
	}
	struct qstr q = BPF_CORE_READ(d, d_name);
	// ignore long files
	if (!q.name || q.len > 30) {
		goto end;
	}
	char buf[32] = {};
	if (bpf_core_read_str(buf, sizeof(buf), q.name) < 0) {
		goto end;
	}
	// Blocks specific NVIDIA files, it's dangerous and will only work if one nvidia gpu is blocked
	__u32 block_nvidia_key = 0;
	if (bpf_map_lookup_elem(&SETTINGS, &block_nvidia_key)) {
		if (bpf_map_lookup_elem(&BLOCKED_NVIDIA_FILES, buf)) {
			__u32 id0 = 0, id1 = 1;
			if (bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id0) &&
			    !bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id1)) {
				blocked = true;
				goto end;
			}
		}
	}
	// PCI Part
	if (bpf_map_lookup_elem(&BLOCKED_PCI_FILES, buf)) {
		char pci_addr[16] = {};
		if (get_pci_addr(d, pci_addr, sizeof(pci_addr)) != 0) {
			goto end;
		}
		pci_addr[12] = '\0';

		if (bpf_map_lookup_elem(&BLOCKED_PCI, pci_addr)) {
			blocked = true;
			goto end;
		}
	}
	// backlight
	if (check_backlight_path(d) == 1) {
		blocked = true;
		goto end;
	}

end:
	if (!blocked) {
		return 0;
	}
	// get mode
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
	// get pid and ppid
	__u32 pid = bpf_get_current_pid_tgid() >> 32;
	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	__u32 ppid = BPF_CORE_READ(task, real_parent, tgid);
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

static __always_inline bool is_dirname_to_hide(const char *dirname,
					       const char *dirname_to_hide,
					       int target_len)
{
	int i = 0;
	for (; i < target_len; i++) {
		if (dirname[i] != dirname_to_hide[i])
			return false;
	}
	return dirname[i] == 0x00;
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
	if (d_inode == 431) {
		if (data->last_visible_bpos != 0xFFFFFFFF) {
			struct linux_dirent64 *visible_dirent =
				(struct linux_dirent64
					 *)(data->dirents_buf +
					    data->last_visible_bpos);
			__u16 visible_reclen;
			bpf_printk("blocking %s with inode %d", dirname,
				   d_inode);
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