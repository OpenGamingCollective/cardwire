//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use log::debug;
use std::collections::HashMap;
use tokio::{
    fs::{self, File}, io::{AsyncBufReadExt, BufReader}
};

pub async fn check_steam_environ(pid: u32) -> bool {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    // Check if the byte array contains the substring
    bytes.windows(11).any(|window| window == b"SteamAppId=")
}
pub async fn check_gamemode(pid: u32) -> bool {
    let path = format!("/proc/{}/maps", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    bytes
        .windows(18)
        .any(|window| window == b"libgamemodeauto.so")
}
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

// maps has a ton of lines
pub async fn check_llm_maps(pid: u32) -> bool {
    let path = format!("/proc/{}/maps", pid);
    let file = match File::open(&path).await {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut line: Vec<u8> = Vec::with_capacity(512);
    while let Ok(bytes_read) = reader.read_until(b'\n', &mut line).await {
        if bytes_read == 0 {
            break;
        }

        let is_match = line.windows(17).any(|w| w == b"libggml-vulkan.so");

        if is_match {
            return true;
        }
        line.clear();
    }

    false
}
pub async fn check_cardwire_allow(pid: u32) -> bool {
    let path = format!("/proc/{}/environ", pid);
    let Ok(bytes) = fs::read(path).await else {
        return false;
    };
    // Check if the byte array contains the substring
    for var in bytes.split(|&b| b == 0) {
        if var.starts_with(b"CARDWIRE_ALLOW=") {
            if var.get(15) == Some(&b'1') {
                debug!("huge");
                return true;
            } else {
                let val = std::str::from_utf8(&var[15..]).unwrap_or("<invalid utf8>");
                debug!("not huge... {}", val);
            }
        }
    }
    false
}
