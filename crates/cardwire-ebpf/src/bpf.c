#include <linux/bpf.h>
#include <linux/types.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

char __license[] SEC("license") = "GPL";

#define ENOENT 2

// kernel type definitions
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

struct dentry {
	struct qstr d_name;
	struct dentry *d_parent;
} __attribute__((preserve_access_index));

struct path {
	struct dentry *dentry;
} __attribute__((preserve_access_index));

struct file {
	struct path f_path;
} __attribute__((preserve_access_index));

// EBPF maps
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

/* Safely read and compare kernel qstr, may be useless */
static __always_inline int qstr_eq(struct qstr q, const char *name, __u32 len)
{
	if (!q.name || q.len != len) {
		return 0;
	}

	char buf[8] = {}; // Big enough for "dri", "dev", "/"
	if (len >= sizeof(buf)) {
		return 0;
	}

	if (bpf_core_read_str(buf, sizeof(buf), q.name) < 0) {
		return 0;
	}

	return __builtin_memcmp(buf, name, len) == 0;
}

/*
Check if file belongs to /dev/dri/, this prevent blocking normal files named
renderD128 or card1
*/
static __always_inline int is_dev_dri(struct dentry *dentry)
{
	struct dentry *parent;
	struct qstr q;

	if (!dentry)
		return 1;

	// First parent must be "dri" (e.g., /dev/dri/card0)
	parent = BPF_CORE_READ(dentry, d_parent);
	if (!parent)
		return 1;
	q = BPF_CORE_READ(parent, d_name);
	if (!qstr_eq(q, "dri", 3))
		return 1;

	// Check for the next parent in the path hierarchy.
	parent = BPF_CORE_READ(parent, d_parent);
	if (!parent)
		return 1;
	q = BPF_CORE_READ(parent, d_name);

	if (qstr_eq(q, "dev", 3)) {
		// If the parent was 'dev', the next parent must be the root '/'.
		parent = BPF_CORE_READ(parent, d_parent);
		if (!parent)
			return 1;
		q = BPF_CORE_READ(parent, d_name);
		if (!qstr_eq(q, "/", 1))
			return 1;
	} else if (qstr_eq(q, "/", 1)) {
		// Parent is already '/', which means 'dri' is at the root of its fs
	} else {
		return 1;
	}

	return 0;
}

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

SEC("lsm/file_open")
int BPF_PROG(file_open, struct file *file)
{
	char filename[16] = {};
	struct dentry *d = BPF_CORE_READ(file, f_path.dentry);
	const unsigned char *name_ptr = NULL;

	if (d) {
		name_ptr = BPF_CORE_READ(d, d_name.name);
	}

	if (name_ptr) {
		if (bpf_core_read_str(filename, sizeof(filename), name_ptr) <
		    0) {
			return 0;
		}
		// CardID Check
		if (__builtin_memcmp(filename, "card", 4) == 0) {
			if (is_dev_dri(d) != 0)
				return 0;

			__u32 id = 0;
			int i = 4;
			int is_match = 0;
#pragma unroll
			for (int j = 0; j < 9; j++) {
				if (i >= sizeof(filename))
					break;
				char c = filename[i];
				if (c >= '0' && c <= '9') {
					id = id * 10 + (c - '0');
					i++;
					is_match = 1;
				} else {
					break;
				}
			}
			if (is_match &&
			    bpf_map_lookup_elem(&BLOCKED_CARDID, &id)) {
				return -ENOENT;
			}
		}
		// RenderID Check
		else if (__builtin_memcmp(filename, "renderD", 7) == 0) {
			if (is_dev_dri(d) != 0)
				return 0;

			__u32 id = 0;
			int i = 7;
			int is_match = 0;
#pragma unroll
			for (int j = 0; j < 9; j++) {
				if (i >= sizeof(filename))
					break;
				char c = filename[i];
				if (c >= '0' && c <= '9') {
					id = id * 10 + (c - '0');
					i++;
					is_match = 1;
				} else {
					break;
				}
			}
			if (is_match &&
			    bpf_map_lookup_elem(&BLOCKED_RENDERID, &id)) {
				return -ENOENT;
			}
		}
		// NVIDIA Check
		else if (__builtin_memcmp(filename, "nvidia", 6) == 0) {
			__u32 id = 0;
			int i = 6;
			int is_match = 0;
#pragma unroll
			for (int j = 0; j < 9; j++) {
				if (i >= sizeof(filename))
					break;
				char c = filename[i];
				if (c >= '0' && c <= '9') {
					id = id * 10 + (c - '0');
					i++;
					is_match = 1;
				} else {
					break;
				}
			}
			if (is_match &&
			    bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id)) {
				return -ENOENT;
			}
		}
		// Config Check via pci
		else if (__builtin_memcmp(filename, "config", 6) == 0) {
			char pci_addr[16] = {};
			if (get_pci_addr(d, pci_addr, sizeof(pci_addr)) != 0) {
				return 0;
			}

			pci_addr[11] = '0';
			pci_addr[12] = '\0';

			if (bpf_map_lookup_elem(&BLOCKED_PCI, pci_addr)) {
				return -ENOENT;
			}
		}
	}
	return 0;
}
