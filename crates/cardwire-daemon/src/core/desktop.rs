#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Desktop {
    Niri,
    Gnome,
    Plasma,
    Cosmic,
}

impl Desktop {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "niri" => Some(Desktop::Niri),
            "gnome" => Some(Desktop::Gnome),
            "plasma" => Some(Desktop::Plasma),
            "cosmic" => Some(Desktop::Cosmic),
            _ => None,
        }
    }
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
