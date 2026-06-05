//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use tokio::fs;

pub async fn check_environ(pid: u32) -> bool {
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
