//! Generic proc/cmdline helpers used by the analyzer

use std::{fs, path::Path};

/// Read the real process name from `/proc/{pid}/cmdline`
pub fn get_real_process_name(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_bytes = match fs::read(&cmdline_path) {
        Ok(b) => b,
        Err(_) => return None, // process died
    };
    parse_cmdline_name(&cmdline_bytes)
}

/// Parse a NUL-separated cmdline and find the real process name
pub fn parse_cmdline_name(cmdline_bytes: &[u8]) -> Option<String> {
    if cmdline_bytes.is_empty() {
        return None;
    }
    let args: Vec<&str> = cmdline_bytes
        .split(|&b| b == 0)
        .filter_map(|b| std::str::from_utf8(b).ok())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    let binary = args[0];

    // Check Wine/Proton
    if (binary.contains("wine") || binary.contains("proton"))
        && let Some(name) = extract_wine_exe(&args)
    {
        return Some(name);
    }

    // Minecraft/Java games, return java instead of the real name to allow Close event bypass
    if binary.ends_with(".java")
        && let Some(name) = extract_java_bin(&args)
    {
        return Some(name);
    }

    // Fallback, just use the binary name
    let base_name = binary.split('/').next_back().unwrap_or(binary);

    // Flatpak/Brwap
    if (base_name == "flatpak" || base_name == ".flatpak-wrapped" || base_name == "bwrap")
        && let Some(name) = extract_flatpak_id(&args)
    {
        return Some(name);
    }

    if base_name == "steam" {
        for arg in args.iter().skip(1) {
            if arg.starts_with("steam://rungameid/") {
                return Some(arg.to_string());
            }
        }
    }

    // Electron apps, the real app name is in the .asar path argument
    if (base_name == "electron" || base_name.ends_with("-electron"))
        && let Some(name) = extract_electron_name(&args)
    {
        return Some(name);
    }

    // Fix for discord or other apps:
    if base_name.contains("--") {
        return base_name.split_whitespace().next().map(|s| s.to_string());
    }

    Some(base_name.to_string())
}

#[inline(always)]
fn extract_wine_exe(args: &Vec<&str>) -> Option<String> {
    for arg in args {
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

#[allow(dead_code)]
pub fn is_proc_still_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
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

/// Decode the 16-byte kernel comm into a String
#[allow(dead_code)]
pub fn comm_to_string(comm: [u8; 16]) -> Option<String> {
    match String::from_utf8(comm.to_vec()) {
        Ok(str) => Some(str.trim_end_matches('\0').to_string()),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cmdline_name_returns_exe_for_wine_proton_cmdline() {
        let cmdline_bytes = b"proton\0Z:\\home\\user\\GAME\\game.exe\0--fullscreen";
        assert_eq!(
            parse_cmdline_name(cmdline_bytes),
            Some("game.exe".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_returns_java_binary_for_game_launcher() {
        // Java launchers fall back to the binary name so that Close events can
        // be attributed to the java process
        let cmdline_bytes = b"/usr/lib/jvm/default/bin/java\0-p\0minecraft.jar";
        assert_eq!(parse_cmdline_name(cmdline_bytes), Some("java".to_string()));
    }

    #[test]
    fn test_parse_cmdline_name_returns_basename_for_regular_binary() {
        // Simulate a regular binary like "/usr/bin/steam"
        let cmdline_bytes = b"/usr/bin/steam\0--no-browser\0";
        assert_eq!(parse_cmdline_name(cmdline_bytes), Some("steam".to_string()));
    }

    #[test]
    fn test_parse_cmdline_name_returns_none_for_empty_cmdline() {
        assert_eq!(parse_cmdline_name(b""), None);
    }

    #[test]
    fn test_parse_cmdline_name_extracts_wine_exe_from_multiarg_cmdline() {
        // Simulates: wine64-preloader\0C:\game\app.exe\0--fullscreen
        let cmdline_bytes = b"wine64-preloader\0C:\\game\\app.exe\0--fullscreen";
        assert_eq!(
            parse_cmdline_name(cmdline_bytes),
            Some("app.exe".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_extracts_flatpak_id() {
        let cmdline_bytes = b"/usr/bin/flatpak\0run\0com.valvesoftware.Steam";
        assert_eq!(
            parse_cmdline_name(cmdline_bytes),
            Some("com.valvesoftware.Steam".to_string())
        );
    }

    #[test]
    fn test_comm_to_string_trims_trailing_nuls() {
        let comm = *b"bash\0\0\0\0\0\0\0\0\0\0\0\0";
        assert!(comm_to_string(comm).is_some_and(|s| s == "bash"));
    }

    #[test]
    fn test_comm_to_string_full_length() {
        let comm = *b"a-very-long-comm";
        assert!(comm_to_string(comm).is_some_and(|s| s == "a-very-long-comm"));
    }

    #[test]
    fn test_comm_to_string_invalid_utf8() {
        let comm = [0xFFu8; 16];
        assert_eq!(comm_to_string(comm), None);
    }

    #[test]
    fn test_is_proc_still_alive() {
        assert!(is_proc_still_alive(std::process::id()));
        assert!(!is_proc_still_alive(0));
    }

    #[test]
    fn test_parse_cmdline_name_extracts_electron_app_from_asar() {
        // NixOS-style Obsidian wrapped in electron
        let cmdline_bytes = b"/nix/store/abc-electron-41.10.3/libexec/electron/electron\0/nix/store/xyz-obsidian-1.12.7/share/obsidian/app.asar\0--ozone-platform=wayland";
        assert_eq!(
            parse_cmdline_name(cmdline_bytes),
            Some("obsidian".to_string())
        );
    }

    #[test]
    fn test_parse_cmdline_name_electron_without_asar_falls_back() {
        // Plain electron without .asar argument
        let cmdline_bytes = b"/usr/bin/electron\0--no-sandbox";
        assert_eq!(
            parse_cmdline_name(cmdline_bytes),
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
