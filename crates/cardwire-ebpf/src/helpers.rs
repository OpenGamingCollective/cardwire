use aya_ebpf::programs::LsmContext;

use crate::{
    maps::CW_BLOCKED_INO, vmlinux::{dentry, inode}
};

use crate::ENOENT;

use aya_log_ebpf::info;

pub unsafe fn is_device_blocked(ctx: &LsmContext, d: *mut dentry) -> Result<i32, i32> {
    // Get a mutable ptr to the inode
    let inode_ptr: *mut inode = unsafe { (*d).d_inode };

    if inode_ptr.is_null() {
        return Ok(0);
    }

    let inode: u64 = unsafe { (*inode_ptr).i_ino };

    // Check if the inode is in the blocked list
    if unsafe { CW_BLOCKED_INO.get(inode).is_some() } {
        info!(ctx, "inode blocked");
        return Ok(ENOENT);
    }

    Ok(0)
}
