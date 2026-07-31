#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::{bpf_get_current_comm, bpf_get_current_pid_tgid}, macros::lsm, programs::LsmContext
};
use aya_log_ebpf::{error, info, warn};

use crate::{
    helpers::{is_dentry_blocked, is_hybrid}, vmlinux::{dentry, file, inode}
};

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

struct ReturnCode {}
impl ReturnCode {
    // Succes means we didn't block the process
    const SUCCESS: Result<i32, i32> = Ok(0);
    // ENOENT means we blocked the process, it can't see the file
    const ENOENT: Result<i32, i32> = Ok(-2);
}

// Used of CW_DAEMON_PID array
const DAEMON_INDEX: u32 = 0;

struct CardwiredSetting {}
impl CardwiredSetting {
    const EXP_NVIDIA: u8 = 0;
}
/*
    Modes
*/
const MODE_INDEX: u32 = 0;
const INTEGRATED: u8 = 0;
const HYBRID: u8 = 1;
const MANUAL: u8 = 2;
const SMART: u8 = 3;

#[lsm(hook = "file_open")]
pub fn lsm_file_open(ctx: LsmContext) -> i32 {
    match unsafe { try_file_open(ctx) } {
        Ok(ret) => ret,
        Err(ret) => ret,
    }
}

unsafe fn try_file_open(ctx: LsmContext) -> Result<i32, i32> {
    // If the mode is hybrid, we exit
    match unsafe { is_hybrid() } {
        Some(res) => {
            if res {
                return ReturnCode::SUCCESS;
            }
        }
        None => {
            // This error happen if either the array is not available or the index 0 of the array is
            // empty
            error!(&ctx, "EBPF is_hybrid produced an error, exiting");
            return ReturnCode::SUCCESS;
        }
    }

    let d: *mut dentry = unsafe {
        // arg.0 of file_open is a file
        let file_ptr: *const file = ctx.arg(0);
        (*file_ptr).__bindgen_anon_1.f_path.dentry
    };

    // if no dentry, exit
    if d.is_null() {
        return ReturnCode::SUCCESS;
    }

    unsafe { is_dentry_blocked(&ctx, d) }
}

#[cfg(not(test))]
#[panic_handler] // (3)
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
