use std::ptr;

use aya::maps::ring_buf::RingBufItem;

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
