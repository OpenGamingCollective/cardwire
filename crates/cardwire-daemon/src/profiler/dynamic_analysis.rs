//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use tokio::fs;

pub async fn check_maps(pid: u32) -> bool {
    let path = format!("/proc/{}/map", pid);
    let map = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return false,
    };
    check_gamemode(&map)
}

fn check_gamemode(map: &str) -> bool {
    map.contains("gamemode")
}

/// Check cmdline for common string like SteamLibrary
pub async fn check_cmdline(pid: u32) -> bool {
    let path = format!("/proc/{}/cmdline", pid);
    let cmdline = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return false,
    };
    check_steam(&cmdline)
}
fn check_steam(cmdline: &str) -> bool {
    cmdline.contains("SteamLibrary/steamapps/common/")
        || cmdline.contains("SteamLaunch")
        || cmdline.contains(r"S:\common")
        || cmdline.contains(r"c:\windows\system32\steam.exe")
        || cmdline.contains("steam-runtime-tools")
}
