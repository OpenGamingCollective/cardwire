use aya::maps::{HashMap as AyaHashMap, RingBuf};
use cardwire_ebpf::EbpfBlocker;
use log::{debug, info, warn};
use std::{collections::HashMap, fs, ptr, sync::Arc};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock, mpsc}, time::Instant
};

use crate::analyzer::{
    dynamic_analysis::{
        check_cardwire_allow, check_fdo_app_id, check_for_flatpak_run, check_gamemode, check_gpu_env, check_steam_environ
    }, static_analysis
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Event {
    pub pid: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Close {
    pub pid: u32,
}

pub enum EventMsg {
    Exec(Event),
    Close(Close),
}

#[derive(Clone)]
pub struct CardwireAnalyzer {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    close_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u8>>>,
    xdg_list: Arc<RwLock<HashMap<String, bool>>>,
}

impl CardwireAnalyzer {
    pub async fn build(blocker: Arc<RwLock<EbpfBlocker>>) -> anyhow::Result<CardwireAnalyzer> {
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
        let xdg_list = Arc::new(RwLock::new(static_analysis::get_fdo_apps().await?));
        Ok(CardwireAnalyzer {
            exec_ring,
            close_ring,
            pid_map,
            xdg_list,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
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

        while let Some(msg) = rx.recv().await {
            match msg {
                EventMsg::Exec(event) => {
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        let time = Instant::now();
                        let pid_map = self_clone.pid_map.read().await;
                        if pid_map.get(&event.pid, 0).is_ok() {
                            return;
                        }
                        drop(pid_map);
                        let real_app_name = match get_real_process_name(event.pid) {
                            Some(name) => name,
                            None => return,
                        };
                        if let Some(result) =
                            self_clone.evaluate_app(event.pid, &real_app_name).await
                            && result
                        {
                            info!(
                                "ALLOW: pid: {} process: {} in {}us",
                                event.pid,
                                &real_app_name,
                                time.elapsed().as_micros()
                            );
                            let mut pid_map = self_clone.pid_map.write().await;
                            if let Err(e) = pid_map.insert(event.pid, 1, 0) {
                                warn!("Failed to insert into eBPF map: {}", e);
                            }
                        }
                    });
                }
                // On close event, remove from map, i made a garbage collector just in case a pid
                // didn't get removed
                EventMsg::Close(event) => {
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        let real_app_name = match get_real_process_name(event.pid) {
                            Some(name) => name,
                            None => "unknown".to_string(),
                        };
                        let mut pid_map = self_clone.pid_map.write().await;
                        // we keep java in the map, and let the garbage collector take care of it,
                        // this fix minecraft not using the dgpu
                        if !real_app_name.contains("java") && pid_map.remove(&event.pid).is_ok() {
                            debug!("REMOVE: pid: {}", event.pid);
                        }
                    });
                }
            }
        }

        Ok(())
    }

    /// Default app are blocked, try to find if it's a game or a gpu intensive app
    async fn evaluate_app(&self, pid: u32, comm: &str) -> Option<bool> {
        let path = format!("/proc/{}/environ", pid);
        let environ = match fs::read(path) {
            Ok(content) => content,
            Err(_) => return None,
        };
        // First check CARDWIRE_ALLOW, if  None continue
        if let Some(allow) = check_cardwire_allow(&environ) {
            return Some(allow);
        }
        let xdg_list = self.xdg_list.read().await;

        let mut result = check_fdo_app_id(comm, &xdg_list)
            || check_steam_environ(&environ)
            || check_gpu_env(&environ);
        // if no result with environ file, read cmdline
        // The goal is to reduce unnecessary reads
        if !result {
            let path_cmd = format!("/proc/{}/cmdline", pid);
            let cmdline = match fs::read_to_string(path_cmd) {
                Ok(content) => content,
                Err(_) => return None,
            };
            result = check_for_flatpak_run(&cmdline, &xdg_list);
        }
        // reading map is slow, should be done if every test are false
        if !result {
            let path_map = format!("/proc/{}/map", pid);
            let map = match fs::read(path_map) {
                Ok(content) => content,
                Err(_) => return None,
            };
            result = check_gamemode(&map);
        }
        Some(result)
    }
}

fn get_real_process_name(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_bytes = match fs::read(&cmdline_path) {
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

    // Minecraft/Java games, return java instead of the real name to allow Close event bypass
    if binary.ends_with(".java") {
        for arg in args.iter().skip(1) {
            if arg.ends_with(".jar") {
                let file_name = arg.split('/').next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }
    // Fallback, just use the binary name
    let base_name = binary.split('/').next_back().unwrap_or(binary);
    Some(base_name.to_string())
}
