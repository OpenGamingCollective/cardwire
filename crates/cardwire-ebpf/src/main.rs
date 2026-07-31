#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::{bpf_get_current_comm, bpf_get_current_pid_tgid}, macros::lsm, programs::LsmContext
};
use aya_log_ebpf::info;

use crate::{
    helpers::is_device_blocked, vmlinux::{dentry, file, inode}
};

const ENOENT: i32 = -2;

#[allow(
    clippy::all,
    dead_code,
    improper_ctypes_definitions,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unnecessary_transmutes,
    unsafe_op_in_unsafe_fn,
)]
#[rustfmt::skip]
mod vmlinux;

mod helpers;

mod maps;

#[lsm(hook = "file_open")]
pub fn lsm_file_open(ctx: LsmContext) -> i32 {
    match unsafe { try_file_open(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    let d: *mut dentry = unsafe {
        // arg.0 of file_open is a file
        let file_ptr: *const file = ctx.arg(0);
        (*file_ptr).__bindgen_anon_1.f_path.dentry
    };

    if d.is_null() {
        return Ok(0);
    }

    unsafe { is_device_blocked(&ctx, d) }
}

#[cfg(not(test))]
#[panic_handler] // (3)
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
