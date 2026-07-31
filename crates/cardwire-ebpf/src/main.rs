#![no_std]
#![no_main]

use aya_ebpf::{bindings::xdp_action, macros::xdp, programs::XdpContext};
use aya_log_ebpf::info;

#[xdp] // (4)
pub fn xdp_hello(ctx: XdpContext) -> u32 {
    // (5)
    match unsafe { try_xdp_hello(ctx) } {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

unsafe fn try_xdp_hello(ctx: XdpContext) -> Result<u32, u32> {
    // (6)
    info!(&ctx, "received a packet");
    // (7)
    Ok(xdp_action::XDP_PASS)
}

#[cfg(not(test))]
#[panic_handler] // (3)
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
