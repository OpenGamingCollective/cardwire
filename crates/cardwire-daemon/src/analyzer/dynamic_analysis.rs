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
        check_gamemode
    */

    #[test]
    fn test_check_gamemode_detects_library_in_maps() {
        let map = b"/usr/lib/libgamemodeauto.so\n/usr/lib/libc.so";
        assert!(check_gamemode(map));
    }

    #[test]
    fn test_check_gamemode_returns_false_when_absent() {
        let map = b"/usr/lib/libc.so.6\n/usr/lib/libm.so.6";
        assert!(!check_gamemode(map));
    }

    #[test]
    fn test_check_gamemode_returns_false_for_empty_input() {
        assert!(!check_gamemode(b""));
    }

    #[test]
    fn test_check_gamemode_rejects_partial_library_name() {
        let map = b"libgamemodeaut.so";
        assert!(!check_gamemode(map));
    }

    /*
        check_fdo_app_id
    */

    #[test]
    fn test_check_fdo_app_id_found_in_list() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("steam".to_string(), true);
        xdg_list.insert("lutris".to_string(), true);
        assert!(check_fdo_app_id("steam", &xdg_list));
    }

    #[test]
    fn test_check_fdo_app_id_not_found_in_list() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("steam".to_string(), true);
        assert!(!check_fdo_app_id("firefox", &xdg_list));
    }

    #[test]
    fn test_check_fdo_app_id_empty_map_returns_false() {
        let xdg_list = HashMap::new();
        assert!(!check_fdo_app_id("anything", &xdg_list));
    }

    #[test]
    fn test_check_fdo_app_id_is_case_sensitive() {
        let mut xdg_list = HashMap::new();
        xdg_list.insert("Steam".to_string(), true);
        assert!(!check_fdo_app_id("steam", &xdg_list));
        assert!(check_fdo_app_id("Steam", &xdg_list));
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
        check_cardwire_allow
    */

    #[test]
    fn test_check_cardwire_allow_returns_true_for_allow_1() {
        let environ = b"HOME=/home\0CARDWIRE_ALLOW=1\0DISPLAY=:0";
        assert_eq!(check_cardwire_allow(environ), Some(true));
    }

    #[test]
    fn test_check_cardwire_allow_returns_false_for_allow_0() {
        let environ = b"HOME=/home\0CARDWIRE_ALLOW=0\0DISPLAY=:0";
        assert_eq!(check_cardwire_allow(environ), Some(false));
    }

    #[test]
    fn test_check_cardwire_allow_returns_none_when_absent() {
        let environ = b"HOME=/home\0DISPLAY=:0";
        assert_eq!(check_cardwire_allow(environ), None);
    }

    #[test]
    fn test_check_cardwire_allow_returns_none_for_empty_input() {
        assert_eq!(check_cardwire_allow(b""), None);
    }

    #[test]
    fn test_check_cardwire_allow_returns_false_for_unexpected_value() {
        // "CARDWIRE_ALLOW=x" — value at index 15 is 'x', not '1'
        let environ = b"CARDWIRE_ALLOW=x";
        assert_eq!(check_cardwire_allow(environ), Some(false));
    }

    /*
        check_gpu_env
    */

    #[test]
    fn test_check_gpu_env_detects_dri_prime_1() {
        let environ = b"HOME=/home\0DRI_PRIME==1\0DISPLAY=:0";
        assert!(check_gpu_env(environ));
    }

    #[test]
    fn test_check_gpu_env_rejects_dri_prime_0() {
        let environ = b"HOME=/home\0DRI_PRIME==0\0DISPLAY=:0";
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
}
