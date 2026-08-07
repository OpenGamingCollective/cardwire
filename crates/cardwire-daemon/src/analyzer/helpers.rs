//! Generic proc/cmdline helpers used by the analyzer

use std::{fs, path::Path};

/// Read the real process name from `/proc/{pid}/cmdline`, taking into account
/// wrappers like Wine/Proton, Java, Flatpak and Steam
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
    if binary.contains("wine") || binary.contains("proton") {
        for arg in args.iter().skip(1) {
            if arg.to_lowercase().ends_with(".exe") {
                let file_name = arg.split(&['/', '\\'][..]).next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }

    // Minecraft/Java games, return java instead of the real name to allow Close event bypass
    if binary.ends_with(".java") {
        for arg in args.iter().skip(1) {
            if arg.ends_with(".jar") {
                let file_name = arg.split('/').next_back().unwrap_or(arg);
                return Some(file_name.to_string());
            }
        }
    }

    // Fallback, just use the binary name
    let base_name = binary.split('/').next_back().unwrap_or(binary);

    // Flatpak/Brwap
    if base_name == "flatpak" || base_name == ".flatpak-wrapped" || base_name == "bwrap" {
        for arg in args.iter().skip(1) {
            if let Some(exec) = arg.strip_prefix("--command=") {
                return Some(exec.to_string());
            }
            // Extract the flatpak ID
            if !arg.starts_with('-') && *arg != "run" && arg.contains('.') {
                return Some(arg.to_string());
            }
        }
    }

    if base_name == "steam" {
        for arg in args.iter().skip(1) {
            if arg.starts_with("steam://rungameid/") {
                return Some(arg.to_string());
            }
        }
    }

    // Fix for discord or other apps:
    if base_name.contains("--") {
        return base_name.split_whitespace().next().map(|s| s.to_string());
    }

    Some(base_name.to_string())
}

pub fn is_proc_still_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Decode the 16-byte kernel comm into a String, trimming trailing NULs
pub fn comm_to_string(comm: [u8; 16]) -> String {
    match String::from_utf8(comm.to_vec()) {
        Ok(str) => str.trim_end_matches('\0').to_string(),
        Err(_) => "no_comm_err".to_string(),
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
        assert_eq!(comm_to_string(comm), "bash");
    }

    #[test]
    fn test_comm_to_string_full_length() {
        let comm = *b"a-very-long-comm";
        assert_eq!(comm_to_string(comm), "a-very-long-comm");
    }

    #[test]
    fn test_comm_to_string_invalid_utf8() {
        let comm = [0xFFu8; 16];
        assert_eq!(comm_to_string(comm), "no_comm_err");
    }

    #[test]
    fn test_is_proc_still_alive() {
        assert!(is_proc_still_alive(std::process::id()));
        assert!(!is_proc_still_alive(0));
    }
}
