use anyhow::Context;
use aya::{
    Btf, maps::HashMap, programs::{Lsm, TracePoint, Xdp, XdpMode}
};
use aya_log::EbpfLogger;
use clap::Parser;
use log::{info, warn};
use tokio::signal; // (1)

#[tokio::main] // (3)
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    // This will include your eBPF object file as raw bytes at compile-time and load it at
    // runtime. This approach is recommended for most real-world use cases. If you would
    // like to specify the eBPF program at runtime rather than at compile-time, you can
    // reach for `Ebpf::load_file` instead.
    // (4)
    // (5)
    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/cardwire-ebpf"
    )))?;
    match EbpfLogger::init(&mut ebpf) {
        Err(e) => {
            // This can happen if you remove all log statements from your eBPF program.
            warn!("failed to initialize eBPF logger: {e}");
        }
        Ok(logger) => {
            let mut logger =
                tokio::io::unix::AsyncFd::with_interest(logger, tokio::io::Interest::READABLE)?;
            tokio::task::spawn(async move {
                loop {
                    let mut guard = logger.readable_mut().await.unwrap();
                    guard.get_inner_mut().flush();
                    guard.clear_ready();
                }
            });
        }
    }
    // (6)
    let program: &mut Lsm = ebpf.program_mut("lsm_file_open").unwrap().try_into()?;
    let btf = Btf::from_sys_fs()?;
    program.load("file_open", &btf)?; // (7)
    // (8)
    program.attach()?;

    let program: &mut Lsm = ebpf
        .program_mut("lsm_inode_permission")
        .unwrap()
        .try_into()?;
    let btf = Btf::from_sys_fs()?;
    program.load("inode_permission", &btf)?; // (7)
    // (8)
    program.attach()?;

    let program: &mut Lsm = ebpf.program_mut("lsm_inode_getattr").unwrap().try_into()?;
    let btf = Btf::from_sys_fs()?;
    program.load("inode_getattr", &btf)?; // (7)
    // (8)
    program.attach()?;

    let program: &mut TracePoint = ebpf
        .program_mut("tracepoint_enter_getdents64")
        .unwrap()
        .try_into()?;
    program.load()?;
    program.attach("syscalls", "sys_enter_getdents64")?;

    let program: &mut TracePoint = ebpf
        .program_mut("tracepoint_exit_getdents64")
        .unwrap()
        .try_into()?;
    program.load()?;
    program.attach("syscalls", "sys_exit_getdents64")?;
    let mut blocklist: HashMap<_, u64, u32> =
        HashMap::try_from(ebpf.map_mut("CW_BLOCKED_INO").unwrap())?;
    if let Ok(_) = blocklist.insert(613, 1, 0) {
        info!("inserted /dev/dri/renderD128 into map!");
    }

    let ctrl_c = signal::ctrl_c();
    info!("Waiting for Ctrl-C...");
    ctrl_c.await?;
    info!("Exiting...");

    Ok(())
}
