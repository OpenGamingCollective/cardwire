use std::{
    collections::{HashMap, VecDeque}, fmt::{self, Display}, time::SystemTime
};
use strum::{EnumIter, FromRepr, VariantArray};

#[derive(
    PartialEq,
    Eq,
    zbus::zvariant::Type,
    Clone,
    Copy,
    Debug,
    VariantArray,
    FromRepr,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[repr(u32)]
#[serde(rename_all = "snake_case")]
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
    Pci,
    SmartMode,
    Logs,
    CardwireSettings,
    Advanced,
    About,
}
impl Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Page::Main => write!(f, "Main"),
            Page::Pci => write!(f, "PCI"),
            Page::SmartMode => write!(f, "Smart Mode"),
            Page::CardwireSettings => write!(f, "Cardwire Settings"),
            Page::Logs => write!(f, "Logs"),
            Page::Advanced => write!(f, "Advanced"),
            Page::About => write!(f, "About"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MainState {
    pub current_mode: Option<Mode>,
    pub available_modes: Vec<Mode>,
    pub open_gpu_menu: Option<usize>,
    pub lsof_window: Option<LsofData>,
}

impl Default for MainState {
    fn default() -> Self {
        Self {
            current_mode: None,
            available_modes: Mode::VARIANTS.to_vec(),
            open_gpu_menu: None,
            lsof_window: None,
        }
    }
}

#[derive(serde::Deserialize, zbus::zvariant::Type, Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub pid: u32,
    pub comm: String,
    pub gpu_id: u32,
    pub wayland_app_id: String,
}

// Maximum number of log entries kept in the GUI
const MAX_GUI_LOG_ENTRIES: usize = 500;

#[derive(Default, Clone, Debug)]
pub struct LogState {
    pub logs: VecDeque<LogEntry>,
}

impl LogState {
    pub fn replace(&mut self, logs: VecDeque<LogEntry>) {
        let mut logs = logs;
        while logs.len() > MAX_GUI_LOG_ENTRIES {
            logs.pop_front();
        }
        self.logs = logs;
    }

    pub fn push(&mut self, log: LogEntry) {
        self.logs.push_back(log);
        while self.logs.len() > MAX_GUI_LOG_ENTRIES {
            self.logs.pop_front();
        }
    }
}

#[derive(Clone, Debug)]
pub struct LsofData {
    pub gpu_id: usize,
    pub processes: HashMap<String, Vec<String>>,
}

#[derive(Default, Clone, Debug)]
pub struct SettingState {
    pub nvidia_checked: bool,
    pub state_checked: bool,
    pub battery_checked: bool,
    pub battery_mode: Option<Mode>,
    pub external_display_checked: bool,
    pub gui_config: crate::gui_config::GuiConfig,
}

#[derive(Clone, Debug)]
pub enum DaemonSettings {
    AutoApplyGpuState,
    ExpNvidiaBlock,
    BattAutoSwitch,
    BattAutoSwitchMode,
    ExternalDisplayAutoSwitch,
}

impl Display for DaemonSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonSettings::AutoApplyGpuState => write!(f, "AutoApplyGpuState"),
            DaemonSettings::ExpNvidiaBlock => write!(f, "ExperimentalNvidiaBlock"),
            DaemonSettings::BattAutoSwitch => write!(f, "BatteryAutoSwitch"),
            DaemonSettings::BattAutoSwitchMode => write!(f, "BatteryAutoSwitchMode"),
            DaemonSettings::ExternalDisplayAutoSwitch => {
                write!(f, "ExternalDisplayAutoSwitch")
            }
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, zbus::zvariant::Type)]
pub struct PciDevice {
    pub iommu_group: String,
    pub vendor_id: String,
    pub device_id: String,
    pub vendor_name: String,
    pub device_name: String,
    pub driver: String,
    pub class: String,
    pub parent_pci: String,
    pub child_pci: String,
}

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug, Clone)]
pub struct DbusAppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
    pub gpu_policy: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedApp {
    pub app_id: String,
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
    pub icon_path: Option<std::path::PathBuf>,
    pub gpu_policy: u32,
}

#[derive(Default, Clone, Debug)]
pub struct SmartState {
    pub app_policies: std::collections::BTreeMap<String, ResolvedApp>,
    pub search_query: String,
    pub loading: bool,
}
