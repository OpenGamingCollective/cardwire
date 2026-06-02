//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use std::fs;

use anyhow::Result;
pub async fn check_gamemode(pid: u32) -> bool {
    let path = format!("/proc/{}/map", pid);
    if let Ok(maps) = fs::read_to_string(path) {
        if maps.contains("gamemode") {
            true
        } else {
            false
        }
    } else {
        false
    }
}

/// Check cmdline for common string like SteamLibrary
pub async fn check_cmdline(pid: u32) -> bool {
    let path = format!("/proc/{}/cmdline", pid);
    if let Ok(cmdline) = fs::read_to_string(path) {
        if cmdline.contains("SteamLibrary/steamapps/common/")
            || cmdline.contains("SteamLaunch")
            || cmdline.contains(r"S:\common")
            || cmdline.contains(r"c:\windows\system32\steam.exe")
            || cmdline.contains("steam-runtime-tools")
        {
            true
        } else {
            false
        }
    } else {
        false
    }
}
