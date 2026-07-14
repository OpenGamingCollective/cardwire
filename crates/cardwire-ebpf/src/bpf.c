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
	// If hybrid skip
	if (is_hybrid())
		return 0;

	struct dentry *d = BPF_CORE_READ(file, f_path.dentry);
	return is_blocked_device(d);
}
/*
	To prevent flatpak from crashing
*/
SEC("lsm/inode_permission")
int BPF_PROG(inode_permission, struct inode *inode, int mask)
{
	// If hybrid skip
	if (is_hybrid())
		return 0;

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
	if (is_hybrid())
		return 0;
	struct dentry *d = BPF_CORE_READ(path, dentry);
	return is_blocked_device(d);
}

/*
	To analyze the app before it's launch if the mode is smart or enforce, send event_t to cardwire
*/
SEC("tracepoint/sched/sched_process_exec")
int trace_exec(void *ctx)
{
	// if mode not smart, skip
	if (!is_smart())
		return 0;

	// Init the struct
	struct event_t *rb_data = {};
	rb_data =
		bpf_ringbuf_reserve(&cw_exec_events, sizeof(struct event_t), 0);
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
	// if mode not smart, skip
	if (!is_smart())
		return 0;

	// we get the pid_tgdi
	__u64 pid_tgid = bpf_get_current_pid_tgid();
	// extract the tgid
	__u32 tgid = pid_tgid >> 32;
	// and the pid
	__u32 pid = pid_tgid & 0xFFFFFFFF;

	// Only send close event if the main thread is exiting
	if (pid != tgid)
		return 0;

	struct close_t *rb_data = {};
	rb_data = bpf_ringbuf_reserve(&cw_close_events, sizeof(struct close_t),
				      0);
	// if struct error, exit
	if (!rb_data) {
		return 0;
	}
	rb_data->pid = tgid;

	bpf_ringbuf_submit(rb_data, 0);
	return 0;
}

SEC("tp/syscalls/sys_enter_getdents64")
int cardwire_sys_enter_getdents64(struct trace_event_raw_sys_enter *ctx)
{
	// If hybrid skip
	if (is_hybrid())
		return 0;

	__u32 pid = bpf_get_current_pid_tgid() >> 32;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	__u32 ppid = BPF_CORE_READ(task, real_parent, tgid);

	// if it's cardwire skip it
	if (is_cardwire_process(pid))
		return 0;

	// skip if whitelisted
	if (is_process_whitelisted())
		return 0;

	// if we in smart mode and the pid is allowed, skip
	if (is_smart()) {
		if (is_pid_allowed(pid, ppid)) {
			// if allowed, skip
			return 0;
		}
	}

	// Get the memory address of the buffer where the list of entry will be stored
	__u64 dirents_buf = ctx->args[1];
	if (!dirents_buf) {
		return 0;
	}
	// Save addr into map
	bpf_map_update_elem(&cw_dirent, &pid, &dirents_buf, BPF_ANY);
	return 0;
}

SEC("tp/syscalls/sys_exit_getdents64")
int cardwire_sys_exit_getdents64(struct trace_event_raw_sys_exit *ctx)
{
	// If hybrid skip
	if (is_hybrid())
		return 0;

	__u32 pid = bpf_get_current_pid_tgid() >> 32;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	__u32 ppid = BPF_CORE_READ(task, real_parent, tgid);

	// if it's cardwire skip it
	if (is_cardwire_process(pid))
		return 0;
	// skip if whitelisted
	if (is_process_whitelisted())
		return 0;
	// if we in smart mode and the pid is allowed, skip
	if (is_smart()) {
		if (is_pid_allowed(pid, ppid)) {
			// if allowed, skip
			return 0;
		}
	}

	__u64 *dirents_buf_ptr = bpf_map_lookup_elem(&cw_dirent, &pid);

	if (!dirents_buf_ptr)
		return 0;

	__u64 dirents_buf = *dirents_buf_ptr;

	// Clean up the map immediately so it doesn't fill up
	bpf_map_delete_elem(&cw_dirent, &pid);

	// If getdents64 return 0 bytes
	if (ctx->ret <= 0) {
		return 0;
	}

	struct dirents_data_t dirents_data = {
		.bpos = 0,
		.dirents_buf = dirents_buf,
		.buff_size = ctx->ret,
		.d_reclen = 0,
		.last_visible_bpos = 0xFFFFFFFF,
	};

	// Run the loop
	bpf_loop(10000, patch_dirent_if_found, &dirents_data, 0);

	return 0;
}