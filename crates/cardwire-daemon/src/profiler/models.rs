use std::{ffi::CStr, fs, ptr, sync::Arc};

use aya::maps::{
    HashMap, Map::{self}, RingBuf
};
use log::{info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::RwLock
};

use crate::{file::CardwireDatabase, profiler::dynamic_analysis::check_cmdline};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Event {
    pub pid: u32,
    pub parent_pid: u32,
    pub comm: [u8; 32],
}

pub struct CardwireProfiler {
    event_map: AsyncFd<RingBuf<aya::maps::MapData>>,
    app_map: HashMap<aya::maps::MapData, u32, u8>,
    database: Arc<RwLock<CardwireDatabase>>,
    // the key should be the comm name for fast lookup
    //database_app_cached: std::collections::HashMap<String, App>
}

pub struct App {}

impl CardwireProfiler {
    pub fn build(
        ring_buffer: RingBuf<aya::maps::MapData>,
        app_map: HashMap<aya::maps::MapData, u32, u8>,
        database: Arc<RwLock<CardwireDatabase>>,
        //database_app_cached: std::collections::HashMap<String, App>,
    ) -> anyhow::Result<CardwireProfiler> {
        let event_map = AsyncFd::new(ring_buffer)?;
        Ok(CardwireProfiler {
            event_map,
            app_map,
            database,
            //database_app_cached,
        })
    }
    pub async fn spawn_profiler(mut self) -> anyhow::Result<()> {
        loop {
            let mut events_batch: Vec<Event> = Vec::new();
            let mut guard = self.event_map.ready_mut(Interest::READABLE).await?;
            if guard.ready().is_readable() {
                while let Some(item) = guard.get_inner_mut().next() {
                    // Ensure size matches our 40-byte struct
                    if item.len() < std::mem::size_of::<Event>() {
                        warn!("Skipping malformed event. Size: {}", item.len());
                        continue;
                    }

                    let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const Event) };
                    events_batch.push(event);
                }
                guard.clear_ready();
            }
            drop(guard);
            for event in events_batch {
                let comm = CStr::from_bytes_until_nul(&event.comm)
                    .map(|c| c.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| String::from("unknown"));

                let child_path = format!("/proc/{}/comm", event.pid);
                if !std::path::Path::new(&child_path).exists() {
                    continue;
                }

                let parent_path = format!("/proc/{}/comm", event.parent_pid);
                let parent_name = fs::read_to_string(parent_path)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| String::from("<exited>"));

                //info!(target: "cardwired-snitch",
                //  "App launched | PID: {} | Name: {} | Parent PID: {} | Parent Name: {}",
                //  event.pid, comm, event.parent_pid, parent_name);
                if self.evaluate_app(event.pid, &comm).await {
                    info!("added pid: {} to the blocklist", event.pid);
                    self.app_map.insert(event.pid, 1, 0)?;
                }
            }
        }
        Ok(())
    }

    /*
       Uses two types of analysis:
           Static (made at startup)
           Dynamic (on-the-fly)

       Dynamic > Static

       Database is used to store the static analysis for faster lookup + comm name matching
    */

    /// Default app are blocked, try to find if it's a game or a gpu intensive app
    async fn evaluate_app(&self, pid: u32, comm: &str) -> bool {
        // Phase 1, dynamic score
        // if it's a game, allow it
        if check_cmdline(pid).await {
            log::info!("Dynamic: {pid}:{comm} ALLOWED, Reason: cmdline detected");
            return true;
        }

        //if check_gamemode(pid).await {
        //    log::info!("Dynamic: {comm} ALLOWED, Reason: Gamemode detected");
        //    return Ok(true);
        //}

        return false;
    }
}
