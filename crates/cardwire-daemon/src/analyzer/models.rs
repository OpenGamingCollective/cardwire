use aya::maps::{HashMap as AyaHashMap, RingBuf};
use cardwire_ebpf::EbpfBlocker;
use log::{debug, info, warn};
use std::{collections::HashMap, fs, path::PathBuf, ptr, sync::Arc};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock}, task, time::Instant
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

#[derive(Clone)]
pub struct CardwireAnalyzer {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    close_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    pid_map: Arc<RwLock<AyaHashMap<aya::maps::MapData, u32, u8>>>,
    xdg_list: Arc<RwLock<HashMap<String, bool>>>,
    #[allow(dead_code)]
    xdg_folders: Vec<PathBuf>,
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
        let xdg_result = static_analysis::get_fdo_apps().await?;
        let xdg_list = Arc::new(RwLock::new(xdg_result.0));
        let xdg_folders: Vec<PathBuf> = xdg_result.1;
        Ok(CardwireAnalyzer {
            exec_ring,
            close_ring,
            pid_map,
            xdg_list,
            xdg_folders,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
        // Clone the Arcs and Sender to move into the background task
        let exec_arc = self.exec_ring.clone();
        let close_arc = self.close_ring.clone();
        // Lock the buffers once
        let mut exec_ring = exec_arc.lock().await;
        let mut close_ring = close_arc.lock().await;
        let shared_self = Arc::new(self);
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
                            let this = Arc::clone(&shared_self);
                            task::spawn(async move {
                                this.spawn_open_analyzer(event).await
                            });
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
                                                        let this = Arc::clone(&shared_self);
                            task::spawn(async move {
                                this.spawn_remove_analyzer(event).await
                            });
                        }
                        guard.clear_ready();
                    }
                }
            }
        }
    }

    async fn spawn_open_analyzer(&self, event: Event) -> () {
        let time = Instant::now();
        let pid_map = self.pid_map.read().await;
        if pid_map.get(&event.pid, 0).is_ok() {
            return;
        }
        drop(pid_map);
        let real_app_name = match get_real_process_name(event.pid) {
            Some(name) => name,
            None => return,
        };
        if let Some(result) = self.evaluate_app(event.pid, &real_app_name).await
            && result
        {
            info!(
                "ALLOW: pid: {} process: {} in {}us",
                event.pid,
                &real_app_name,
                time.elapsed().as_micros()
            );
            let mut pid_map = self.pid_map.write().await;
            if let Err(e) = pid_map.insert(event.pid, 1, 0) {
                warn!("Failed to insert into eBPF map: {}", e);
            }
        }
    }
    async fn spawn_remove_analyzer(&self, event: Close) -> () {
        let mut pid_map = self.pid_map.write().await;
        if pid_map.remove(&event.pid).is_ok() {
            debug!("REMOVE: pid: {}", event.pid);
        }
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

    #[allow(dead_code)]
    pub fn xdg_list(&self) -> Arc<RwLock<HashMap<String, bool>>> {
        Arc::clone(&self.xdg_list)
    }
    #[allow(dead_code)]
    pub fn xdg_folders(&self) -> &Vec<PathBuf> {
        &self.xdg_folders
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
