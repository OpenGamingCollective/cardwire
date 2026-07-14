//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use std::collections::HashMap;

/// Read the proc `environ` file to find the `SteamAppId=` string
/// used to identify both native and proton games
pub fn check_steam_environ(environ: &[u8]) -> bool {
    environ.windows(11).any(|window| window == b"SteamAppId=")
}

/// Read the proc `maps` file to find the gamemodeauto.so
pub fn check_gamemode(map: &[u8]) -> bool {
    map.windows(18)
        .any(|window| window == b"libgamemodeauto.so")
}

/// Check if the comm is in the xdg list
pub fn check_fdo_app_id(comm: &str, xdg_list: &HashMap<String, bool>) -> bool {
    xdg_list.contains_key(comm)
}

pub fn check_for_flatpak_run(cmdline: &str, xdg_list: &HashMap<String, bool>) -> bool {
    let mut args = cmdline.split('\0').filter(|s| !s.is_empty());

    if let Some(arg0) = args.next() {
        // Ensure the actual executable is flatpak or bwrap, not a wrapper like 'niri msg'
        if !arg0.ends_with("flatpak")
            && !arg0.ends_with(".flatpak-wrapped")
            && !arg0.ends_with("bwrap")
        {
            return false;
        }
    } else {
        return false;
    }

    // Now check if any of the arguments match our allowed app
    for arg in args {
        if let Some(exec) = arg.strip_prefix("--command=") {
            if xdg_list.contains_key(exec) {
                return true;
            }
        } else if xdg_list.contains_key(arg) {
            return true;
        }
    }

    false
}

pub fn check_cardwire_allow(environ: &[u8]) -> Option<bool> {
    for var in environ.split(|&b| b == 0) {
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
pub fn check_gpu_env(environ: &[u8]) -> bool {
    for var in environ.split(|&b| b == 0) {
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
