//! Functions for dynamic analysis, contains:
//! - environment analysis
//! - wayland app id lookup
use std::{
    env, fs, path::{Path, PathBuf}, time::{Duration, Instant}
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

pub fn get_steam_app_id(environ: &[u8]) -> Option<String> {
    let prefix = b"SteamAppId=";
    for var in environ.split(|&b| b == 0) {
        if let Some(value_bytes) = var.strip_prefix(prefix)
            && let Ok(id_str) = std::str::from_utf8(value_bytes)
            && id_str != "0"
            && id_str != "769"
        {
            return Some(format!("steam_app_{}", id_str));
        }
    }
    None
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
        // "CARDWIRE_ALLOW=x" value at index 15 is 'x', not '1'
        let environ = b"CARDWIRE_ALLOW=x";
        assert_eq!(check_env("CARDWIRE_ALLOW", environ), None);
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
