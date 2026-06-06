//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use std::collections::HashMap;
use tokio::{
    fs::{self, File}, io::{AsyncBufReadExt, BufReader}
};

/// Read the proc `environ` file to find the `SteamAppId=` string
/// used to identify both native and proton games
pub async fn check_steam_environ(pid: u32) -> bool {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    bytes.windows(11).any(|window| window == b"SteamAppId=")
}

/// Read the proc `maps` file to find the gamemodeauto.so
pub async fn check_gamemode(pid: u32) -> bool {
    let path = format!("/proc/{}/maps", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    bytes
        .windows(18)
        .any(|window| window == b"libgamemodeauto.so")
}

/// Read the environ map to file the FLATPAK_ID and compare with .desktop apps
pub async fn check_flatpak_environ(pid: u32, xdg_list: &HashMap<String, bool>) -> bool {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    // Check if the byte array contains the substring
    for var in bytes.split(|&b| b == 0) {
        if var.starts_with(b"FLATPAK_ID=")
            && let Ok(str) = std::str::from_utf8(&var[11..])
            && xdg_list.contains_key(str)
        {
            return true;
        }
    }
    false
}

pub async fn check_cardwire_allow(pid: u32) -> Option<bool> {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return None;
    };

    for var in bytes.split(|&b| b == 0) {
        if var.starts_with(b"CARDWIRE_ALLOW=") {
            if var.get(15) == Some(&b'1') {
                return Some(true); // CARDWIRE_ALLOW=1
            } else {
                return Some(false); // CARDWIRE_ALLOW=0
            }
        }
    }
    // Not present
    None
}
pub async fn check_gpu_env(pid: u32) -> bool {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };

    for var in bytes.split(|&b| b == 0) {
        if var.starts_with(b"DRI_PRIME==") {
            if var.get(11) == Some(&b'1') {
                return true; // DRI_PRIME=1
            } else {
                return false; // DRI_PRIME=0
            }
        } else if var.starts_with(b"__NV_PRIME_RENDER_OFFLOAD=") {
            if var.get(26) == Some(&b'1') {
                return true; // =1
            } else {
                return false; // = 0
            }
        }
    }
    // Not present
    false
}
