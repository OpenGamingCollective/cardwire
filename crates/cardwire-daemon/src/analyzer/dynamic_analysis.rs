//! Functions for dynamic analysis, contains:
//! - environment analysis
//! - wayland app id lookup

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
}
