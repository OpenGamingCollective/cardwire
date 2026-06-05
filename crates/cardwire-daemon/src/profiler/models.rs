use aya::maps::{HashMap as AyaHashMap, RingBuf};
use cardwire_ebpf::EbpfBlocker;
use log::{debug, warn};
use std::{collections::HashMap, ptr, sync::Arc};
use tokio::{
    fs, io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock, mpsc}
};

use crate::profiler::dynamic_analysis::{check_environ, check_gamemode};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Event {
    pub pid: u32,
    pub parent_pid: u32,
    pub comm: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Close {
    pub pid: u32,
    pub parent_pid: u32,
    pub comm: [u8; 32],
    pub exit_code: u32,
}

pub enum EventMsg {
    Exec(Event),
    Close(Close),
}

#[derive(Clone)]
pub struct CardwireProfiler {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    close_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u8>>>,
}

impl CardwireProfiler {
    pub async fn build(blocker: Arc<RwLock<EbpfBlocker>>) -> anyhow::Result<CardwireProfiler> {
        let mut blocker = blocker.write().await;
        let exec_ring = blocker.get_exec_ring()?;
        let close_ring = blocker.get_close_ring()?;
        let pid_map = blocker.get_pid_map()?;

        let exec_ring = AsyncFd::new(exec_ring)?;
        let close_ring = AsyncFd::new(close_ring)?;

        // Now Rwlock -> Arc
        let exec_ring = Arc::new(Mutex::new(exec_ring));
        let pid_map = Arc::new(RwLock::new(pid_map));
        let close_ring = Arc::new(Mutex::new(close_ring));

        Ok(CardwireProfiler {
            exec_ring,
            close_ring,
            pid_map,
            //database_app_cached,
        })
    }
    pub async fn spawn_profiler(self) -> anyhow::Result<()> {
        // Create the channel
        let (tx, mut rx) = mpsc::channel::<EventMsg>(10_000);

        // Clone the Arcs and Sender to move into the background task
        let exec_arc = self.exec_ring.clone();
        let close_arc = self.close_ring.clone();
        let tx_task = tx.clone();

        tokio::spawn(async move {
            // Lock the buffers once
            let mut exec_ring = exec_arc.lock().await;
            let mut close_ring = close_arc.lock().await;

            loop {
                tokio::select! {
                    Ok(mut guard) = exec_ring.ready_mut(Interest::READABLE) => {
                        if guard.ready().is_readable() {
                            while let Some(item) = guard.get_inner_mut().next() {
                                if item.len() < std::mem::size_of::<Event>() {
                                    debug!("Skipping malformed exec event. Size: {}", item.len());
                                    continue;
                                }
                                let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const Event) };
                                let _ = tx_task.try_send(EventMsg::Exec(event));
                            }
                            guard.clear_ready();
                        }
                    }

                    Ok(mut guard) = close_ring.ready_mut(Interest::READABLE) => {
                        if guard.ready().is_readable() {
                            while let Some(item) = guard.get_inner_mut().next() {
                                if item.len() < std::mem::size_of::<Close>() {
                                    debug!("Skipping malformed close event. Size: {}", item.len());
                                    continue;
                                }
                                let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const Close) };
                                let _ = tx_task.try_send(EventMsg::Close(event));
                            }
                            guard.clear_ready();
                        }
                    }
                }
            } // end loop
        });

        // Garbage collector to not overflow the map and to keep it clean
        let gc_map = self.pid_map.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                debug!("Running Garbage Collector...");
                let keys: Vec<u32> = {
                    let map = gc_map.read().await;
                    map.keys().flatten().collect()
                };
                let mut dead_pids: Vec<u32> = Vec::new();
                for pid in keys {
                    let proc_path = format!("/proc/{}", pid);
                    if tokio::fs::metadata(&proc_path).await.is_err() {
                        dead_pids.push(pid);
                    }
                }
                if !dead_pids.is_empty() {
                    let mut map = gc_map.write().await;
                    let dead_pids_len = dead_pids.len();
                    for pid in dead_pids {
                        let _ = map.remove(&pid);
                    }
                    debug!("Garbage Collector removed {} pids", dead_pids_len);
                } else {
                    debug!("Garbage Collector found 0 ghost pids");
                }
            }
        });

        let mut pid_cache: HashMap<u32, u32> = HashMap::new();

        while let Some(msg) = rx.recv().await {
            match msg {
                EventMsg::Exec(event) => {
                    if pid_cache.contains_key(&event.pid) {
                        continue;
                    }
                    //tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    let real_app_name = match get_real_process_name(event.pid).await {
                        Some(name) => name,
                        None => continue,
                    };
                    if self.evaluate_app(event.pid).await {
                        debug!("ALLOW: pid: {}, name: {}", event.pid, real_app_name);
                        // Only acquire the write lock right when we need it
                        let mut app_map = self.pid_map.write().await;
                        if let Err(e) = app_map.insert(event.pid, 1, 0) {
                            warn!("Failed to insert into eBPF map: {}", e);
                        }
                        pid_cache.insert(event.pid, event.parent_pid);
                    }
                }
                EventMsg::Close(event) => {
                    pid_cache.remove(&event.pid);
                    let mut pid_map = self.pid_map.write().await;
                    let _ = pid_map.remove(&event.pid);
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
    async fn evaluate_app(&self, pid: u32) -> bool {
        // experimentation
        // TODO: Replace
        let check = |check: bool| if check { Err(()) } else { Ok(()) };
        let result = tokio::try_join!(async { check(check_environ(pid).await) }, async {
            check(check_gamemode(pid).await)
        },);

        result.is_err()
    }
}

async fn get_real_process_name(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_bytes = match fs::read(&cmdline_path).await {
        Ok(b) => b,
        Err(_) => return None, // process died
    };
    if cmdline_bytes.is_empty() {
        return None;
    }
    let args: Vec<&str> = cmdline_bytes
        .split(|&b| b == 0)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    let binary = args[0];

    // Check Wine/Proton
    if binary.contains("wine") || binary.contains("proton") {
        for arg in args.iter().skip(1) {
            if arg.to_lowercase().ends_with(".exe") {
                let file_name = arg.split(&['/', '\\'][..]).next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }

    // Minecraft/Java games
    if binary.ends_with(".java") {
        for arg in args.iter().skip(1) {
            if arg.ends_with(".jar") {
                let file_name = arg.split('/').next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }
    // Fallback
    let base_name = binary.split('/').next_back().unwrap_or(binary);
    Some(base_name.to_string())
}
