use aya::maps::{RingBuf, ring_buf};
use log::info;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::Mutex
};

#[repr(C)]
pub struct Event {
    pid: u32,
    comm: [char; 32],
}

pub async fn bpf_snitch(ring_buffer: RingBuf<aya::maps::MapData>) -> anyhow::Result<()> {
    let mut poll = AsyncFd::new(ring_buffer)?;
    loop {
        let mut guard = poll.ready_mut(Interest::READABLE).await?;
        if guard.ready().is_readable() {
            // try to read the data
            while let Some(item) = guard.get_inner_mut().next() {
                let comm: String = String::from_utf8(item.to_ascii_lowercase())?;
                info!(target: "cardwired-snitch", "found this app {}", comm);
            }
            guard.clear_ready();
        }
    }
}
