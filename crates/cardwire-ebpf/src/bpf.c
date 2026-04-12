#include <linux/bpf.h>
#include <linux/types.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

char __license[] SEC("license") = "GPL";

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

struct dentry {
	struct qstr d_name;
	struct dentry *d_parent;
	struct inode *d_inode;
	union {
		struct hlist_node d_alias;
	} d_u;
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

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 64);
	__type(key, __u32);
	__type(value, __u8);
} SETTINGS SEC(".maps");

/* Safely read and compare kernel qstr */
static __always_inline int qstr_eq(struct qstr q, const char *name, __u32 len)
{
	if (!q.name || q.len != len) {
		return 0;
	}

	char buf[32] = {};
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

static __always_inline int is_blocked_device(struct dentry *d)
{
	if (!d) {
		return 0;
	}

	char comm[16] = {};
	bpf_get_current_comm(comm, sizeof(comm));
	if (__builtin_memcmp(comm, "cardwired", 9) == 0) {
		return 0;
	}

	struct inode *inode = BPF_CORE_READ(d, d_inode);
	if (inode) {
		__u16 i_mode = BPF_CORE_READ(inode, i_mode);
		if ((i_mode & 00170000) == 00020000) {
			__u32 i_rdev = BPF_CORE_READ(inode, i_rdev);
			unsigned int major = i_rdev >> 20;
			unsigned int minor = i_rdev & 0xFFFFF;
			if (major == 226) {
				__u32 id = minor;
				if (bpf_map_lookup_elem(&BLOCKED_CARDID, &id)) {
					return -ENOENT;
				}
				if (bpf_map_lookup_elem(&BLOCKED_RENDERID,
							&id)) {
					return -ENOENT;
				}
			} else if (major == 195) {
				__u32 id = minor;
				if (bpf_map_lookup_elem(&BLOCKED_NVIDIAID,
							&id)) {
					return -ENOENT;
				}
			}
			return 0;
		}
	}
	struct qstr q = BPF_CORE_READ(d, d_name);
	// Blocks vulkan nvidia_icd, it's dangerous and will only work if one nvidia gpu is blocked
	__u32 block_vulkan_key = 0;
	if (bpf_map_lookup_elem(&SETTINGS, &block_vulkan_key)) {
		if (qstr_eq(q, "nvidia_icd.json", 15) ||
		    qstr_eq(q, "nvidia_icd.x86_64.json", 22)) {
			__u32 id0 = 0, id1 = 1;
			if (bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id0) &&
			    !bpf_map_lookup_elem(&BLOCKED_NVIDIAID, &id1)) {
				return -ENOENT;
			}
		}
	}

	if (qstr_eq(q, "config", 6)) {
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

	return 0;
}

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

	unsigned long offset =
		bpf_core_field_offset(struct dentry, d_u.d_alias);
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