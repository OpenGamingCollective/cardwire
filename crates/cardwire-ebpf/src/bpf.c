/// This file only contain the bpf programs, functions are defined in helpers.c
#include <linux/bpf.h>
#include <linux/types.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <stdbool.h>
#include "bpf.h"
#include "helpers.h"

char __license[] SEC("license") = "GPL";

/*
	LSM to prevent open on DRM
*/

SEC("lsm/file_open")
int BPF_PROG(file_open, struct file *file)
{
	struct dentry *d = BPF_CORE_READ(file, f_path.dentry);
	return is_blocked_device(d);
}
/*
	To prevent flatpak from crashing
*/
SEC("lsm/inode_permission")
int BPF_PROG(inode_permission, struct inode *inode, int mask)
{
	char filename[16] = {};
	const unsigned char *name_ptr = NULL;
	/*
		I do not understand this part but it works
	*/
	struct hlist_node *first = BPF_CORE_READ(inode, i_dentry.first);
	if (!first) {
		return 0;
	}

	unsigned long offset;

	// This is for kernel compatibility
	if (bpf_core_field_exists(((struct dentry___old *)0)->d_u.d_alias)) {
		offset =
			bpf_core_field_offset(struct dentry___old, d_u.d_alias);
	} else {
		offset = bpf_core_field_offset(struct dentry, d_alias);
	}
	struct dentry *d = (struct dentry *)((void *)first - offset);
	//
	return is_blocked_device(d);
}
/*
	To prevent flatpak from crashing, 
*/
SEC("lsm/inode_getattr")
int BPF_PROG(inode_getattr, const struct path *path)
{
	struct dentry *d = BPF_CORE_READ(path, dentry);
	return is_blocked_device(d);
}

/*
	To analyze the app before it's launch if the mode is smart or enforce, send event_t to cardwire
*/
SEC("tracepoint/sched/sched_process_exec")
int trace_exec(void *ctx)
{
	// get current cardwired mode, key should always be 0
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
	if (!mode) {
		return 0;
	}
	//if mode is not smart or enforce, skip
	if (*mode != 3) {
		return 0;
	}
	// Init the struct
	struct event_t *rb_data = {};
	rb_data = bpf_ringbuf_reserve(&EXEC_EVENTS, sizeof(struct event_t), 0);
	// Check if present
	if (!rb_data) {
		return 0;
	}

	// Read PID
	rb_data->pid = bpf_get_current_pid_tgid() >> 32;

	bpf_ringbuf_submit(rb_data, 0);
	return 0;
}

SEC("tracepoint/sched/sched_process_exit")
int trace_process_exit(void *ctx)
{
	// get current cardwired mode, key should always be 0
	__u32 key = 0;
	__u8 *mode = bpf_map_lookup_elem(&CURRENT_MODE, &key);
	if (!mode) {
		return 0;
	}
	//if mode is not smart, skip
	if (*mode != 3) {
		return 0;
	}
	struct close_t *rb_data = {};
	rb_data = bpf_ringbuf_reserve(&CLOSE_EVENTS, sizeof(struct close_t), 0);
	if (!rb_data) {
		return 0;
	}
	rb_data->pid = bpf_get_current_pid_tgid() >> 32;

	bpf_ringbuf_submit(rb_data, 0);
	return 0;
}

SEC("tp/syscalls/sys_enter_getdents64")
int cardwire_sys_enter_getdents64(struct trace_event_raw_sys_enter *ctx)
{
	__u32 pid = bpf_get_current_pid_tgid() >> 32;

	__u64 dirents_buf = ctx->args[1];
	if (!dirents_buf) {
		return 0;
	}
	bpf_map_update_elem(&map_dirent, &pid, &dirents_buf, BPF_ANY);
	return 0;
}

SEC("tp/syscalls/sys_exit_getdents64")
int cardwire_sys_exit_getdents64(struct trace_event_raw_sys_exit *ctx)
{
	__u32 pid = bpf_get_current_pid_tgid() >> 32; // Fixed: use __u32
	__u64 *dirents_buf = bpf_map_lookup_elem(&map_dirent, &pid);
	if (!dirents_buf)
		return 0;

	// If getdents64 returned an error or 0 bytes, clean up and exit
	if (ctx->ret <= 0) {
		bpf_map_delete_elem(&map_dirent, &pid);
		return 0;
	}

	struct dirents_data_t dirents_data = {
		.bpos = 0,
		.dirents_buf = dirents_buf,
		.buff_size = ctx->ret,
		.d_reclen = 0,
		.d_reclen_prev = 0,
		.patch_succeded = false,
	};

	bpf_loop(10000, patch_dirent_if_found, &dirents_data, 0);

	// CRITICAL FIX: Clean up the map so it doesn't fill up and block future hooks
	bpf_map_delete_elem(&map_dirent, &pid);

	return 0; // Fix: return 0 from tracepoint
}