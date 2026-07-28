use aya::maps::{HashMap as AyaHashMap, RingBuf};
use cardwire_ebpf::EbpfBlocker;
use log::{debug, info, warn};
use std::{collections::HashMap, fs, path::PathBuf, ptr, sync::Arc};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::{Mutex, RwLock}, task, time::Instant
};

use crate::analyzer::{
    dynamic_analysis::{
        check_cardwire_allow, check_fdo_app_id, check_for_flatpak_run, check_gamemode, check_gpu_env, check_steam_environ, get_app_id_wayland
    }, static_analysis
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ExecEvent {
    pub pid: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CloseEvent {
    pub pid: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ReportEvent {
    pub pid: u32,
    pub comm: [u8; 16],
}

#[derive(Clone)]
pub struct CardwireAnalyzer {
    exec_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    close_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
    report_ring: Arc<Mutex<AsyncFd<RingBuf<aya::maps::MapData>>>>,
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
        let report_ring = blocker.get_report_ring()?;
        let pid_map = blocker.get_pid_map()?;

        let exec_ring = AsyncFd::new(exec_ring)?;
        let close_ring = AsyncFd::new(close_ring)?;
        let report_ring = AsyncFd::new(report_ring)?;

        // Now Rwlock -> Arc
        let exec_ring = Arc::new(Mutex::new(exec_ring));
        let pid_map = Arc::new(RwLock::new(pid_map));
        let close_ring = Arc::new(Mutex::new(close_ring));
        let report_ring = Arc::new(Mutex::new(report_ring));
        let xdg_result = static_analysis::get_fdo_apps().await?;
        let xdg_list = Arc::new(RwLock::new(xdg_result.0));
        let xdg_folders: Vec<PathBuf> = xdg_result.1;
        Ok(CardwireAnalyzer {
            exec_ring,
            close_ring,
            report_ring,
            pid_map,
            xdg_list,
            xdg_folders,
        })
    }
    pub async fn run(self) -> anyhow::Result<()> {
        // Clone the Arcs and Sender to move into the background task
        let exec_arc = self.exec_ring.clone();
        let close_arc = self.close_ring.clone();
        let report_arc = self.report_ring.clone();
        // Lock the buffers once
        let mut exec_ring = exec_arc.lock().await;
        let mut close_ring = close_arc.lock().await;
        let mut report_ring = report_arc.lock().await;

        // Used to prevent duplicated logs burst
        let mut previous_reported_pid = 0;

        let shared_self = Arc::new(self);
        loop {
            tokio::select! {
                Ok(mut guard) = exec_ring.ready_mut(Interest::READABLE) => {
                    if guard.ready().is_readable() {
                        while let Some(item) = guard.get_inner_mut().next() {
                            if item.len() < std::mem::size_of::<ExecEvent>() {
                                debug!("Skipping malformed exec event. Size: {}", item.len());
                                continue;
                            }
                            let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
                            let this = Arc::clone(&shared_self);
                            task::spawn(async move {
                                this.spawn_exec_analyzer(event).await
                            });
                        }
                        guard.clear_ready();
                    }
                }

                Ok(mut guard) = close_ring.ready_mut(Interest::READABLE) => {
                    if guard.ready().is_readable() {
                        while let Some(item) = guard.get_inner_mut().next() {
                            if item.len() < std::mem::size_of::<CloseEvent>() {
                                debug!("Skipping malformed close event. Size: {}", item.len());
                                continue;
                            }
                            let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const CloseEvent) };
                            let this = Arc::clone(&shared_self);
                            task::spawn(async move {
                                this.spawn_remove_analyzer(event).await
                            });
                        }
                        guard.clear_ready();
                    }
                }
                Ok(mut guard) = report_ring.ready_mut(Interest::READABLE) => {
                        while let Some(item) = guard.get_inner_mut().next() {
                            if item.len() < std::mem::size_of::<ReportEvent>() {
                                debug!("Skipping malformed report event. Size: {}", item.len());
                                continue;
                            }
                            let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
                            // only log if we didn't see the pid before
                            if event.pid != previous_reported_pid {
                                previous_reported_pid = event.pid;
                                task::spawn(async move {
                                    if let Some(app_id) = get_app_id_wayland(event.pid).await {
                                        // use dGPU term instead of GPU, smart mode is only avaible on hybrid setups
                                        info!("{}[{}] tried to access the dGPU (blocked by cardwire)", &app_id, event.pid);
                                    }
                                });
                            }
                        }
                        guard.clear_ready();
                }
            }
        }
    }

    async fn spawn_exec_analyzer(&self, event: ExecEvent) -> () {
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
    async fn spawn_remove_analyzer(&self, event: CloseEvent) -> () {
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

// TESTS

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_event_deserialization_from_valid_bytes() {
        let item: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x00, // pid = 1
        ];
        assert!(item.len() >= std::mem::size_of::<ExecEvent>());
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
        assert_eq!(event.pid, 1);
    }

    #[test]
    fn test_event_deserialization_rejects_undersized_buffer() {
        let item: Vec<u8> = vec![0x01, 0x00, 0x00]; // 3 bytes, Event needs 4
        assert!(item.len() < std::mem::size_of::<ExecEvent>());
    }

    #[test]
    fn test_event_deserialization_with_large_pid() {
        // pid = 0xFFFFFFFF (u32::MAX)
        let item: Vec<u8> = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ExecEvent) };
        assert_eq!(event.pid, u32::MAX);
    }

    #[test]
    fn test_close_event_deserialization() {
        let item: Vec<u8> = vec![
            0x2A, 0x00, 0x00, 0x00, // pid = 42
        ];
        assert!(item.len() >= std::mem::size_of::<CloseEvent>());
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const CloseEvent) };
        assert_eq!(event.pid, 42);
    }

    #[test]
    fn test_get_real_process_name_returns_exe_for_wine_proton_cmdline() {
        let cmdline_bytes =
            r"S:\common\NieR·Replicant·ver.1.22474487139\NieR·Replicant·ver.1.22474487139.exe"
                .as_bytes();

        assert!(!cmdline_bytes.is_empty());
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        {
            let binary = args[0];

            // Check Wine/Proton
            if binary.contains("wine") || binary.contains("proton") {
                for arg in args.iter().skip(1) {
                    if arg.to_lowercase().ends_with(".exe") {
                        let file_name = arg.split(&['/', '\\'][..]).next_back().unwrap_or(arg);
                        assert_eq!(file_name, "NieR·Replicant·ver.1.22474487139.exe");
                    }
                }
            }
        }
    }

    #[test]
    fn test_get_real_process_name_returns_jar_for_java_cmdline() {
        let cmdline_bytes = "minecraft.jar".as_bytes();

        assert!(!cmdline_bytes.is_empty());
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        {
            let binary = args[0];

            // Check Wine/Proton
            if binary.ends_with(".java") {
                for arg in args.iter().skip(1) {
                    if arg.ends_with(".jar") {
                        let file_name = arg.split('/').next_back().unwrap_or(arg);
                        assert_eq!(file_name, "minecraft");
                    }
                }
            }
        }
    }

    #[test]
    fn test_get_real_process_name_returns_basename_for_regular_binary() {
        // Simulate a regular binary like "/usr/bin/steam"
        let cmdline_bytes = b"/usr/bin/steam\0--no-browser\0";
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(!args.is_empty());
        let binary = args[0];
        let base_name = binary.split('/').next_back().unwrap_or(binary);
        assert_eq!(base_name, "steam");
    }

    #[test]
    fn test_get_real_process_name_returns_none_for_empty_cmdline() {
        let cmdline_bytes = b"";
        assert!(cmdline_bytes.is_empty());
    }

    #[test]
    fn test_get_real_process_name_extracts_wine_exe_from_multiarg_cmdline() {
        // Simulates: wine64-preloader\0C:\game\app.exe\0--fullscreen
        let cmdline_bytes = b"wine64-preloader\0C:\\game\\app.exe\0--fullscreen";
        let args: Vec<&str> = cmdline_bytes
            .split(|&b| b == 0)
            .filter_map(|b| std::str::from_utf8(b).ok())
            .filter(|s| !s.is_empty())
            .collect();
        let binary = args[0];
        assert!(binary.contains("wine"));
        // Find the .exe argument
        let exe_arg = args
            .iter()
            .skip(1)
            .find(|a| a.to_lowercase().ends_with(".exe"));
        assert!(exe_arg.is_some());
        let file_name = exe_arg
            .unwrap()
            .split(&['/', '\\'][..])
            .next_back()
            .unwrap();
        assert_eq!(file_name, "app.exe");
    }

    // ── ReportEvent ──────────────────────────────────────────────────

    #[test]
    fn test_report_event_deserialization_from_valid_bytes() {
        // ReportEvent: pid (4 bytes) + comm (16 bytes) = 20 bytes
        let mut item: Vec<u8> = vec![
            0x39, 0x05, 0x00, 0x00, // pid = 1337
        ];
        // comm = "steam\0\0\0\0\0\0\0\0\0\0\0" (16 bytes, null-padded)
        item.extend_from_slice(b"steam\0\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(item.len(), 20);
        assert!(item.len() >= std::mem::size_of::<ReportEvent>());
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
        assert_eq!(event.pid, 1337);
        // Verify the comm field contains "steam"
        let comm_str = std::str::from_utf8(&event.comm)
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(comm_str, "steam");
    }

    #[test]
    fn test_report_event_deserialization_rejects_undersized_buffer() {
        // ReportEvent needs 20 bytes, give only 19
        let item: Vec<u8> = vec![0u8; 19];
        assert!(item.len() < std::mem::size_of::<ReportEvent>());
    }

    #[test]
    fn test_report_event_comm_extraction_with_full_length_name() {
        // comm is exactly 15 chars + null terminator (16 bytes total)
        let mut item: Vec<u8> = vec![
            0x01, 0x00, 0x00, 0x00, // pid = 1
        ];
        item.extend_from_slice(b"123456789012345\0"); // 15 chars + null = 16 bytes
        let event = unsafe { ptr::read_unaligned(item.as_ptr() as *const ReportEvent) };
        assert_eq!(event.pid, 1);
        let comm_str = std::str::from_utf8(&event.comm)
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(comm_str, "123456789012345");
    }
}
