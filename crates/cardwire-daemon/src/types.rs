//! Core domain types shared across the daemon.
//!
//! The `Modes` enum encoding (`Integrated=0`, `Hybrid=1`, `Manual=2`, `Smart=3`) is the
//! contract between the daemon and the eBPF program. The eBPF side defines the same values
//! as constants in `cardwire-ebpf/src/main.rs`:
//!
//! ```c
//! const INTEGRATED: u8 = 0;
//! const HYBRID: u8     = 1;
//! const MANUAL: u8     = 2;
//! const SMART: u8      = 3;
//! ```
//!
//! When adding a mode, update both sides of this contract.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Modes {
    Integrated,
    #[default]
    Hybrid,
    Manual,
    Smart,
}

impl fmt::Display for Modes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Modes::Integrated => write!(f, "Integrated"),
            Modes::Hybrid => write!(f, "Hybrid"),
            Modes::Manual => write!(f, "Manual"),
            Modes::Smart => write!(f, "Smart"),
        }
    }
}

/// Error returned when a u32 does not map to a known GPU mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidModeError {
    value: u32,
}

impl InvalidModeError {
    pub fn value(&self) -> u32 {
        self.value
    }
}

impl fmt::Display for InvalidModeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unknown mode: {}", self.value)
    }
}

impl std::error::Error for InvalidModeError {}

/// Try to convert a u32 into a mode.
///
/// This is the deserialization side of the D-Bus/eBPF encoding contract.
impl TryFrom<u32> for Modes {
    type Error = InvalidModeError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            3 => Ok(Self::Smart),
            _ => Err(InvalidModeError { value }),
        }
    }
}

/// Convert a mode into a u32.
///
/// This is the serialization side of the D-Bus/eBPF encoding contract.
impl From<Modes> for u32 {
    fn from(value: Modes) -> Self {
        match value {
            Modes::Integrated => 0,
            Modes::Hybrid => 1,
            Modes::Manual => 2,
            Modes::Smart => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modes_try_from_valid_values() {
        assert_eq!(Modes::try_from(0).unwrap(), Modes::Integrated);
        assert_eq!(Modes::try_from(1).unwrap(), Modes::Hybrid);
        assert_eq!(Modes::try_from(2).unwrap(), Modes::Manual);
        assert_eq!(Modes::try_from(3).unwrap(), Modes::Smart);
    }

    #[test]
    fn test_modes_try_from_invalid_value() {
        assert!(Modes::try_from(4).is_err());
        assert!(Modes::try_from(u32::MAX).is_err());
        assert_eq!(Modes::try_from(4).unwrap_err().value(), 4);
    }

    #[test]
    fn test_modes_into_u32_roundtrip() {
        for i in 0..=3u32 {
            let mode = Modes::try_from(i).unwrap();
            let back: u32 = mode.into();
            assert_eq!(back, i);
        }
    }

    #[test]
    fn test_modes_display_formatting() {
        assert_eq!(Modes::Integrated.to_string(), "Integrated");
        assert_eq!(Modes::Hybrid.to_string(), "Hybrid");
        assert_eq!(Modes::Manual.to_string(), "Manual");
        assert_eq!(Modes::Smart.to_string(), "Smart");
    }

    #[test]
    fn test_modes_default_is_hybrid() {
        assert_eq!(Modes::default(), Modes::Hybrid);
    }

    #[test]
    fn test_modes_serde_json_roundtrip() {
        let modes = [
            Modes::Integrated,
            Modes::Hybrid,
            Modes::Manual,
            Modes::Smart,
        ];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: Modes = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, mode);
        }
    }

    #[test]
    fn test_modes_serde_uses_snake_case() {
        let json = serde_json::to_string(&Modes::Integrated).unwrap();
        assert_eq!(json, "\"integrated\"");
        let json = serde_json::to_string(&Modes::Hybrid).unwrap();
        assert_eq!(json, "\"hybrid\"");
    }
}
