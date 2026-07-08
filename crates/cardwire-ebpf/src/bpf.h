
// Return value used for file blocking
#define ENOENT 2

// kernel type definitions

// For inode
struct hlist_node {
	struct hlist_node *next, **pprev;
} __attribute__((preserve_access_index));

struct hlist_head {
	struct hlist_node *first;
} __attribute__((preserve_access_index));

struct inode {
	__u16 i_mode;
	__u32 i_rdev;
	__u64 i_ino;
	struct hlist_head i_dentry;
} __attribute__((preserve_access_index));

struct qstr {
	union {
		struct {
			__u32 hash;
			__u32 len;
		};
		__u64 hash_len;
	};
	const unsigned char *name;
} __attribute__((preserve_access_index));

struct dentry___old {
	struct qstr d_name;
	struct dentry *d_parent;
	struct inode *d_inode;
	union {
		struct hlist_node d_alias;
	} d_u;
} __attribute__((preserve_access_index));

struct dentry {
	struct qstr d_name;
	struct dentry *d_parent;
	struct inode *d_inode;
	struct hlist_node d_alias;
} __attribute__((preserve_access_index));

struct path {
	struct dentry *dentry;
} __attribute__((preserve_access_index));

struct file {
	struct path f_path;
} __attribute__((preserve_access_index));

struct dirents_data_t {
	__u32 bpos;
	__u64 dirents_buf;
	long buff_size;
	__u16 d_reclen;
	__u32 last_visible_bpos;
};

struct linux_dirent64 {
	__u64 d_ino;
	__s64 d_off;
	short unsigned int d_reclen;
	unsigned char d_type;
	char d_name[0];
};

struct trace_event_raw_sys_enter {
	__u64 unused_common_fields;
	long id;
	unsigned long args[6];
	char __data[0];
};

struct trace_event_raw_sys_exit {
	__u64 unused_common_fields;
	long id;
	long ret;
} __attribute__((preserve_access_index));

struct task_struct {
	int tgid;
	struct task_struct *real_parent;
	int exit_code;
} __attribute__((preserve_access_index));

// Ring related struct
struct event_t {
	__u32 pid;
};

struct close_t {
	__u32 pid;
};

struct report_t {
	__u32 pid;
	char comm[32];
};

// EBPF maps
// This one is to report the events to cardwire
struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} EXEC_EVENTS SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} CLOSE_EVENTS SEC(".maps");

// This one is to report the app block to cardwire
struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} REPORT SEC(".maps");

// List of blocked comm
// Used for smart mode
struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 16384);
	__type(key, __u32);
	__type(value, __u8);
} ALLOWED_PID SEC(".maps");

/*
	mode map, mode should be stored in key 0
	possible values:
	integrated = 0
	hybrid = 1
	manual = 2
	enforce = 3
	smart = 4
*/
struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1);
	__type(key, __u8);
	__type(value, __u8);
} CURRENT_MODE SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, __u32);
	__type(value, __u8);
} BLOCKED_RENDERID SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, __u32);
	__type(value, __u8);
} BLOCKED_NVIDIAID SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, __u32);
	__type(value, __u8);
} BLOCKED_CARDID SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, char[16]);
	__type(value, __u8);
} BLOCKED_PCI SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, char[30]);
	__type(value, __u8);
} BLOCKED_PCI_FILES SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 64);
	__type(key, __u32);
	__type(value, __u8);
} SETTINGS SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, char[30]);
	__type(value, __u8);
} BLOCKED_NVIDIA_FILES SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 1024);
	__type(key, __u32);
	__type(value, __u64);
} map_dirent SEC(".maps");
