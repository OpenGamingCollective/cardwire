//! Functions for on-the-fly process analysis

use std::{fs, path::Path};

use crate::process::errors::Result;

/// Read the /proc/pid/environ of a process
pub fn read_proc_environ(pid: u32) -> Result<Vec<u8>> {
    let path = format!("/proc/{}/environ", pid);
    Ok(fs::read(path)?)
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

/// Get the SteamAppId by reading the environ
pub fn get_steam_app_id(environ: &[u8]) -> Option<String> {
    let prefix = b"SteamAppId=";
    for var in environ.split(|&b| b == 0) {
        if let Some(value_bytes) = var.strip_prefix(prefix)
            && let Ok(id_str) = std::str::from_utf8(value_bytes)
            && id_str != "0"
            // SteamWebHelper
            // <https://steamdb.info/app/769/>
            && id_str != "769"
        {
            return Some(format!("steam_app_{}", id_str));
        }
    }
    None
}

/// Read the process cmdline from `/proc/{pid}/cmdline`
pub fn read_proc_cmdline(pid: u32) -> Result<Vec<u8>> {
    let path = format!("/proc/{}/cmdline", pid);
    Ok(fs::read(path)?)
}

/// Read the real process name from the process cmdline
pub fn get_real_process_name(cmdline: &[u8]) -> Option<String> {
    if cmdline.is_empty() {
        return None;
    }

    // Split at null byte
    let args: Vec<&str> = cmdline
        .split(|&b| b == 0)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .filter(|s| !s.is_empty())
        .collect();

    if args.is_empty() {
        return None;
    }

    let binary = args[0];

    match binary {
        _ if (binary.contains("wine") || binary.contains("proton")) => {
            if let Some(res) = extract_wine_exe(&args) {
                return Some(res);
            }
        }
        _ if binary.ends_with(".java") => {
            if let Some(res) = extract_java_bin(&args) {
                return Some(res);
            }
        }
        _ => {}
    }

    if let Some(bin_name) = binary.split('/').next_back() {
        match bin_name {
            _ if bin_name == "steam" => {
                for arg in args.iter().skip(1) {
                    if arg.starts_with("steam://rungameid/") {
                        return Some(arg.to_string());
                    }
                }
            }
            _ if (bin_name == "flatpak"
                || bin_name == ".flatpak-wrapped"
                || bin_name == "bwrap") =>
            {
                if let Some(res) = extract_flatpak_id(&args) {
                    return Some(res);
                }
            }
            // For electron apps, the real app name is in the .asar path argument
            _ if (bin_name == "electron" || bin_name.ends_with("-electron")) => {
                if let Some(res) = extract_electron_name(&args) {
                    return Some(res);
                }
            }
            // Fix for discord or other apps:
            _ if bin_name.contains("--") => {
                return bin_name.split_whitespace().next().map(|s| s.to_string());
            }
            _ => {}
        }
        return Some(bin_name.to_string());
    }
    None
}

#[inline(always)]
fn extract_wine_exe(args: &Vec<&str>) -> Option<String> {
    for arg in args.iter().skip(1) {
        if arg.to_lowercase().contains(".exe")
            && let Some(file_name) = arg.split(&['/', '\\'][..]).next_back()
        {
            return Some(file_name.to_string());
        }
    }
    None
}

#[inline(always)]
fn extract_java_bin(args: &Vec<&str>) -> Option<String> {
    for arg in args.iter().skip(1) {
        if arg.ends_with(".jar")
            && let Some(file_name) = arg.split('/').next_back()
        {
            return Some(file_name.to_string());
        }
    }
    None
}

#[inline(always)]
fn extract_flatpak_id(args: &Vec<&str>) -> Option<String> {
    for arg in args.iter().skip(1) {
        if let Some(exec) = arg.strip_prefix("--command=") {
            return Some(exec.to_string());
        }
        if !arg.starts_with('-') && *arg != "run" && arg.contains('.') {
            return Some(arg.to_string());
        }
    }
    None
}

#[inline(always)]
fn extract_electron_name(args: &Vec<&str>) -> Option<String> {
    for arg in args.iter().skip(1) {
        if arg.starts_with('-') {
            continue;
        }
        if arg.ends_with(".asar") || arg.contains("resources/app") {
            let path = Path::new(arg);
            for component in path.components().rev() {
                let part = component.as_os_str().to_string_lossy();
                if part == "app.asar" || part == "resources" || part == "app" || part == "share" {
                    continue;
                }
                return Some(part.to_string());
            }
        }
    }
    None
}

/// Strip the wrap from a nix wrapped binary
pub fn strip_nix_wrap(name: &str) -> String {
    // eg: ".discord-wrapped"
    let trimmed = name.trim_start_matches('.');
    trimmed
        .strip_suffix("-wrapped")
        .unwrap_or(trimmed)
        .to_string()
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
    fn test_parse_cmdline_name_returns_exe_for_wine_proton_cmdline() {
        let cmdline_bytes = b"proton\0Z:\\home\\user\\GAME\\game.exe\0--fullscreen";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("game.exe".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_returns_java_binary_for_game_launcher() {
        // Java launchers fall back to the binary name so that Close events can
        // be attributed to the java process
        let cmdline_bytes = b"/usr/lib/jvm/default/bin/java\0-p\0minecraft.jar";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("java".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_returns_basename_for_regular_binary() {
        // Simulate a regular binary like "/usr/bin/steam"
        let cmdline_bytes = b"/usr/bin/steam\0--no-browser\0";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("steam".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_returns_none_for_empty_cmdline() {
        assert_eq!(get_real_process_name(b""), None);
    }

    #[test]
    fn test_parse_cmdline_name_extracts_wine_exe_from_multiarg_cmdline() {
        // Simulates: wine64-preloader\0C:\game\app.exe\0--fullscreen
        let cmdline_bytes = b"wine64-preloader\0C:\\game\\app.exe\0--fullscreen";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("app.exe".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_extracts_flatpak_id() {
        let cmdline_bytes = b"/usr/bin/flatpak\0run\0com.valvesoftware.Steam";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("com.valvesoftware.Steam".to_string())
        );
    }

    #[test]
    fn test_comm_to_string_trims_trailing_nuls() {
        let comm = *b"bash\0\0\0\0\0\0\0\0\0\0\0\0";
        assert!(get_real_process_name(&comm).is_some_and(|s| s == "bash"));
    }

    #[test]
    fn test_comm_to_string_full_length() {
        let comm = *b"a-very-long-comm";
        assert!(get_real_process_name(&comm).is_some_and(|s| s == "a-very-long-comm"));
    }

    #[test]
    fn test_comm_to_string_invalid_utf8() {
        let comm = [0xFFu8; 16];
        assert_eq!(get_real_process_name(&comm), None);
    }

    #[test]
    fn test_parse_cmdline_name_extracts_electron_app_from_asar() {
        // NixOS-style Obsidian wrapped in electron
        let cmdline_bytes = b"/nix/store/abc-electron-41.10.3/libexec/electron/electron\0/nix/store/xyz-obsidian-1.12.7/share/obsidian/app.asar\0--ozone-platform=wayland";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("obsidian".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_electron_without_asar_falls_back() {
        // Plain electron without .asar argument
        let cmdline_bytes = b"/usr/bin/electron\0--no-sandbox";
        assert_eq!(
            get_real_process_name(cmdline_bytes),
            Some("electron".to_string())
        );
    }

    #[test]
    fn test_normalized_candidates_unwraps_nix_wrapper() {
        assert_eq!(strip_nix_wrap(".discord-wrapped"), "discord");
    }

    #[test]
    fn test_normalized_candidates_plain_name_unchanged() {
        assert_eq!(strip_nix_wrap("steamwebhelper"), "steamwebhelper");
        assert_eq!(strip_nix_wrap("steam"), "steam");
    }
}
