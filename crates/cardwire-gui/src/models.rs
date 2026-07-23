use std::fmt::{self, Display};
use strum::{EnumIter, FromRepr, VariantArray};

#[derive(PartialEq, zbus::zvariant::Type, Clone, Copy, Debug, VariantArray, FromRepr, Default)]
#[repr(u32)]
pub enum Mode {
    Integrated = 0,
    Hybrid = 1,
    #[default]
    Manual = 2,
    Smart = 3,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Mode::Integrated => "Integrated",
            Mode::Hybrid => "Hybrid",
            Mode::Manual => "Manual",
            Mode::Smart => "Smart",
        };
        write!(f, "{}", s)
    }
}

impl From<Mode> for u32 {
    fn from(value: Mode) -> Self {
        value as u32
    }
}

#[derive(Debug, Clone, Copy, EnumIter, Default, PartialEq)]
pub enum Page {
    #[default]
    Main,
    SmartMode,
    CardwireSettings,
    AccessLogs,
    About,
}
impl Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Page::Main => write!(f, "Main"),
            Page::SmartMode => write!(f, "Smart Mode"),
            Page::CardwireSettings => write!(f, "Cardwire Settings"),
            Page::AccessLogs => write!(f, "Access Logs"),
            Page::About => write!(f, "About"),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct MainState {
    pub current_mode: Option<Mode>,
}

#[derive(Default, Clone, Debug)]
pub struct SettingState {
    pub nvidia_checked: bool,
    pub state_checked: bool,
    pub battery_checked: bool,
    pub battery_mode: Option<Mode>,
}

#[derive(Clone, Debug)]
pub enum DaemonSettings {
    AutoApplyGpuState,
    ExpNvidiaBlock,
    BattAutoSwitch,
    BattAutoSwitchMode,
}

impl Display for DaemonSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonSettings::AutoApplyGpuState => write!(f, "AutoApplyGpuState"),
            DaemonSettings::ExpNvidiaBlock => write!(f, "ExperimentalNvidiaBlock"),
            DaemonSettings::BattAutoSwitch => write!(f, "BatteryAutoSwitch"),
            DaemonSettings::BattAutoSwitchMode => write!(f, "BatteryAutoSwitchMode"),
        }
    }
}
