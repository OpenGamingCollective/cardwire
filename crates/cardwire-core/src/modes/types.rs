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

/// Try to convert a u32 into a mode
impl TryFrom<u32> for Modes {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            3 => Ok(Self::Smart),
            _ => Err("unknown"),
        }
    }
}

/// Convert a mode into a u32
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
