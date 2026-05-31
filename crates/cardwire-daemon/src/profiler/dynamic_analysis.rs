//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use std::fs;

use anyhow::Result;
/// Check cmdline to see if it's an electron app
pub async fn check_electron(pid: u32) -> Result<bool> {
    let path = format!("/proc/{}/cmdline", pid);
    let cmdline = fs::read_to_string(path)?;
    println!("cmdline is : {}", cmdline);
    if cmdline.contains("--type=zygote") {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub async fn check_gamemode(pid: u32) -> Result<bool> {
    let path = format!("/proc/{}/map", pid);
    let maps = fs::read_to_string(path)?;
    if maps.contains("gamemode") {
        Ok(true)
    } else {
        Ok(false)
    }
}
