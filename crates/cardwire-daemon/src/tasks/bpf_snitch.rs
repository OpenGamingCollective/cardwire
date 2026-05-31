use std::{ffi::CStr, fs, path::Path, ptr, sync::Arc};

use aya::maps::RingBuf;
use log::info;
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::RwLock
};

use crate::file::CardwireDatabase;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Event {
    pid: u32,
    comm: [u8; 32],
    parent_pid: u32,
}

pub async fn bpf_snitch(
    ring_buffer: RingBuf<aya::maps::MapData>,
    database: Arc<RwLock<CardwireDatabase>>,
) -> anyhow::Result<()> {
    let mut poll = AsyncFd::new(ring_buffer)?;
    loop {
        let mut guard = poll.ready_mut(Interest::READABLE).await?;
        if guard.ready().is_readable() {
            // try to read the data
            while let Some(item) = guard.get_inner_mut().next() {
                // check if our event can contain the bpf one
                if item.len() < std::mem::size_of::<Event>() {
                    continue;
                }
                // TODO: find an unsafe way
                let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const Event) };
                let comm = CStr::from_bytes_until_nul(&event.comm)
                    .map(|c_str| c_str.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| String::from("unknown"));
                let parent_path = format!("/proc/{}/comm", event.parent_pid);
                let parent_name: String = fs::read_to_string(parent_path)?;
                info!(target: "cardwired-snitch", "found this app with pid: {:?} and name: {:?} and parent: {} and parent name: {}", event.pid, comm, event.parent_pid, parent_name);
                // Now analysis for debug purpose
                println!(
                    "is electron: {}",
                    crate::profiler::check_electron(event.parent_pid).await?
                );
                let mut database_lock = database.write().await;
                database_lock
                    .insert_app(comm.clone(), comm.clone(), true)
                    .await?;
                drop(database_lock);
            }
            guard.clear_ready();
        }
    }
}
