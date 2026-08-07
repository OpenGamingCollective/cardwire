//! Functions for dynamic analysis, contains:
//! - gamemoderun analysis
//! - library analysis
use std::{
    collections::HashMap, env, fs, path::{Path, PathBuf}, time::{Duration, Instant}
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, net::UnixStream
};

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Desktop {
    Niri,
    Gnome,
    Plasma,
    Cosmic,
}

impl Desktop {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "niri" => Some(Desktop::Niri),
            "gnome" => Some(Desktop::Gnome),
            "plasma" => Some(Desktop::Plasma),
            "cosmic" => Some(Desktop::Cosmic),
            _ => None,
        }
    }
}

/// Read the proc `environ` file to find the `SteamAppId=` string
/// used to identify both native and proton games
pub fn check_steam_environ(environ: &[u8]) -> bool {
    environ.windows(11).any(|window| window == b"SteamAppId=")
}

/// Find the process name inside the flatpak cmdline, and match if it's inside our xdg_list
#[allow(dead_code)]
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

pub fn check_env(env_var: &str, environ: &[u8]) -> Option<u32> {
    let prefix = format!("{}=", env_var);
    let prefix_bytes = prefix.as_bytes();

    for var in environ.split(|&b| b == 0) {
        if let Some(value_bytes) = var.strip_prefix(prefix_bytes) {
            let value_str = std::str::from_utf8(value_bytes).ok()?;
            return value_str.parse::<u32>().ok();
        }
    }

    None
}

pub fn check_gpu_env(environ: &[u8]) -> bool {
    if let Some(val) = check_env("DRI_PRIME", environ) {
        return val == 1;
    } else if let Some(val) = check_env("__NV_PRIME_RENDER_OFFLOAD", environ) {
        return val == 1;
    }
    // Not present
    false
}

/// How long a reported pid keeps getting retried before falling back to the
/// process name
pub const APP_ID_LOOKUP_TIMEOUT: Duration = Duration::from_millis(2000);

/// pid to wayland app id, needs to be async to wait
pub async fn get_app_id_wayland(pid: u32) -> Option<String> {
    let desktop_str: String = match env::var("XDG_CURRENT_DESKTOP") {
        Ok(value) => value,
        Err(_) => return None,
    };
    let desktop: Desktop = Desktop::from_str(&desktop_str)?;

    #[allow(clippy::single_match)]
    match desktop {
        // We use the niri ipc to get the window real name
        Desktop::Niri => {
            if let Some(socket_path) = find_niri_socket() {
                return query_niri_window(&socket_path, pid).await;
            }
        }
        _ => {}
    }

    None
}

/// Retry `get_app_id_wayland` until the lookup timeout expires, the window
/// of a freshly launched process can take a moment to be mapped by the
/// compositor. Breaks early if the process exits.
pub async fn get_app_id_wayland_with_retry(pid: u32) -> Option<String> {
    let deadline = Instant::now() + APP_ID_LOOKUP_TIMEOUT;
    let delay = Duration::from_millis(50);
    loop {
        // The process is gone, we will never find a window for it
        if !Path::new(&format!("/proc/{}", pid)).exists() {
            return None;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        if let Ok(Some(app_id)) = tokio::time::timeout(remaining, get_app_id_wayland(pid)).await {
            return Some(app_id);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        tokio::time::sleep(delay.min(remaining)).await;
    }
}

/// Query niri IPC for a window's app_id by pid
/// Returns None on any error
async fn query_niri_window(socket_path: &Path, pid: u32) -> Option<String> {
    let mut socket = UnixStream::connect(socket_path).await.ok()?;
    socket.write_all(b"{\"Windows\":null}\n").await.ok()?;
    socket.flush().await.ok()?;

    let mut reader = BufReader::new(socket);
    let mut reply = String::new();
    reader.read_line(&mut reply).await.ok()?;

    let json: serde_json::Value = serde_json::from_str(&reply).ok()?;
    json["Ok"]["Windows"]
        .as_array()?
        .iter()
        .find(|w| w["pid"].as_u64() == Some(pid as u64))
        .and_then(|w| w["app_id"].as_str())
        .map(|s| s.to_string())
}

fn find_niri_socket() -> Option<PathBuf> {
    let run_path = Path::new("/run/user");
    for user in run_path.read_dir().ok()? {
        if let Ok(user) = user
            && let Ok(dir_content) = fs::read_dir(user.path())
        {
            for entry in dir_content.flatten() {
                if entry.file_name().to_string_lossy().contains("niri.wayland") {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /*
        check_steam_environ
    */
    #[test]
    fn test_check_steam_environ_detects_steam_app_id() {
        let environ = b"HOME=/home/user\0SteamAppId=12345\0DISPLAY=:0";
        assert!(check_steam_environ(environ));
    }

    #[test]
    fn test_check_steam_environ_returns_false_when_absent() {
        let environ = b"HOME=/home/user\0DISPLAY=:0\0TERM=xterm";
        assert!(!check_steam_environ(environ));
    }

    #[test]
    fn test_check_steam_environ_returns_false_for_empty_input() {
        assert!(!check_steam_environ(b""));
    }

    #[test]
    fn test_check_steam_environ_detects_at_start_of_environ() {
        let environ = b"SteamAppId=999";
        assert!(check_steam_environ(environ));
    }

    /*
        check_for_flatpak_run
    */
    #[test]
    fn test_check_for_flatpak_run_detects_flatpak_binary() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("com.valvesoftware.Steam".to_string(), true);
        let cmdline = "/usr/bin/flatpak\0run\0com.valvesoftware.Steam";
        assert!(check_for_flatpak_run(cmdline, &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_detects_bwrap_binary() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("com.valvesoftware.Steam".to_string(), true);
        let cmdline = "/usr/bin/bwrap\0--arg\0com.valvesoftware.Steam";
        assert!(check_for_flatpak_run(cmdline, &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_detects_command_flag() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("steam".to_string(), true);
        let cmdline = "/usr/bin/flatpak\0run\0--command=steam\0com.example.App";
        assert!(check_for_flatpak_run(cmdline, &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_rejects_non_flatpak_binary() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("steam".to_string(), true);
        let cmdline = "/usr/bin/niri\0msg\0steam";
        assert!(!check_for_flatpak_run(cmdline, &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_rejects_empty_cmdline() {
        let xdg_list = HashMap::new();
        assert!(!check_for_flatpak_run("", &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_returns_false_for_unknown_app() {
        let xdg_list = HashMap::new();
        let cmdline = "/usr/bin/flatpak\0run\0com.unknown.App";
        assert!(!check_for_flatpak_run(cmdline, &xdg_list));
    }

    #[test]
    fn test_check_for_flatpak_run_detects_flatpak_wrapped() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("com.valvesoftware.Steam".to_string(), true);
        let cmdline = "/app/bin/.flatpak-wrapped\0com.valvesoftware.Steam";
        assert!(check_for_flatpak_run(cmdline, &xdg_list));
    }

    /*
        check_env
    */

    #[test]
    fn test_check_env_returns_true_for_allow_1() {
        let environ = b"HOME=/home\0CARDWIRE_ALLOW=1\0DISPLAY=:0";
        assert_eq!(check_env("CARDWIRE_ALLOW", environ), Some(1));
    }

    #[test]
    fn test_check_env_returns_true_for_allow_1_dgpu() {
        let environ = b"HOME=/home\0CARDWIRE_FORCE_DGPU=1\0DISPLAY=:0";
        assert_eq!(check_env("CARDWIRE_FORCE_DGPU", environ), Some(1));
    }

    #[test]
    fn test_check_env_returns_false_for_allow_0() {
        let environ = b"HOME=/home\0CARDWIRE_ALLOW=0\0DISPLAY=:0";
        assert_eq!(check_env("CARDWIRE_ALLOW", environ), Some(0));
    }

    #[test]
    fn test_check_env_returns_none_when_absent() {
        let environ = b"HOME=/home\0DISPLAY=:0";
        assert_eq!(check_env("CARDWIRE_ALLOW", environ), None);
    }

    #[test]
    fn test_check_env_returns_none_for_empty_input() {
        assert_eq!(check_env("CARDWIRE_ALLOW", b""), None);
    }

    #[test]
    fn test_check_env_returns_none_for_unexpected_value() {
        // "CARDWIRE_ALLOW=x" — value at index 15 is 'x', not '1'
        let environ = b"CARDWIRE_ALLOW=x";
        assert_eq!(check_env("CARDWIRE_ALLOW", environ), None);
    }

    /*
        check_gpu_env
    */

    #[test]
    fn test_check_gpu_env_detects_dri_prime_1() {
        let environ = b"HOME=/home\0DRI_PRIME=1\0DISPLAY=:0";
        assert!(check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_rejects_dri_prime_0() {
        let environ = b"HOME=/home\0DRI_PRIME=0\0DISPLAY=:0";
        assert!(!check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_detects_nv_prime_render_offload_1() {
        let environ = b"__NV_PRIME_RENDER_OFFLOAD=1";
        assert!(check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_rejects_nv_prime_render_offload_0() {
        let environ = b"__NV_PRIME_RENDER_OFFLOAD=0";
        assert!(!check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_returns_false_when_neither_present() {
        let environ = b"HOME=/home\0DISPLAY=:0\0TERM=xterm";
        assert!(!check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_returns_false_for_empty_input() {
        assert!(!check_gpu_env(b""));
    }

    #[test]
    fn test_desktop_from_str_all_known_variants() {
        assert!(matches!(Desktop::from_str("niri"), Some(Desktop::Niri)));
        assert!(matches!(Desktop::from_str("gnome"), Some(Desktop::Gnome)));
        assert!(matches!(Desktop::from_str("plasma"), Some(Desktop::Plasma)));
        assert!(matches!(Desktop::from_str("cosmic"), Some(Desktop::Cosmic)));
    }

    #[test]
    fn test_desktop_from_str_unknown_returns_none() {
        assert!(Desktop::from_str("sway").is_none());
        assert!(Desktop::from_str("hyprland").is_none());
        assert!(Desktop::from_str("i3").is_none());
        assert!(Desktop::from_str("").is_none());
    }
}
