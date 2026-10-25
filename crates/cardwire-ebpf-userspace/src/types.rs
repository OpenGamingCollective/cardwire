use std::ptr;

use aya::maps::ring_buf::RingBufItem;
use log::warn;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ExecEvent {
    pub pid: u32,
    pub mode: u8,
    pub _padding: [u8; 3],
}

impl ExecEvent {
    pub fn from_item(item: RingBufItem<'_>) -> Option<Self> {
        if item.len() < std::mem::size_of::<ExecEvent>() {
            return None;
        }

        Some(unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) })
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub struct ReportEvent {
    pub pid: u32,
    pub gpu_id: u32,
    pub comm: [u8; 16],
}

impl ReportEvent {
    pub fn from_item(item: RingBufItem<'_>) -> Option<Self> {
        if item.len() < std::mem::size_of::<ReportEvent>() {
            return None;
        }

        Some(unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) })
    }
}

/// Layout must stay identical to the eBPF side's InodeKey, the kernel hashes
/// the raw key bytes so any drift turns every lookup into a silent miss
#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InodeKey {
    /// Entry name (dentry d_name / dirent d_name), zero-padded to 64 bytes
    pub name: [u8; 64],
    pub ino: u64,
}
#[cfg(feature = "bpf")]
unsafe impl aya::Pod for InodeKey {}

impl InodeKey {
    /// Build a key from an entry's name and inode number
    ///
    /// The name is truncated to 63 bytes: the eBPF side reads names through
    /// bpf_probe_read_*_str, which reserves the last byte for a NUL
    pub fn new(name: &str, ino: u64) -> Self {
        let mut key = Self {
            name: [0u8; 64],
            ino,
        };

        let len = name.len().min(63);
        key.name[..len].copy_from_slice(&name.as_bytes()[..len]);
        if name.len() > 63 {
            warn!(
                "inode key name {} is longer than 63 bytes, truncating",
                name
            );
        }

        key
    }
}
