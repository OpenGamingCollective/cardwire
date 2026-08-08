use crate::{
    gui_config::GuiConfig, helpers::GpuDevice, models::{DaemonSettings, LogEntry, LsofData, Mode, Page, PciDevice}, tray::{TrayAction, TrayHandle}
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    FetchedMode(Result<Mode, String>),
    SetMode(Mode),
    AllDevicesFetched(Result<BTreeMap<usize, GpuDevice>, String>),
    UpdateNvidiaSetting(bool),
    UpdateStateSetting(bool),
    UpdateBatterySetting(bool),
    UpdateBatteryMode(Mode),
    UpdateGuiConfig(GuiConfig),
    TrayReady(TrayHandle),
    TrayAction(TrayAction),
    TrayUnavailable(String),
    TrayShutdownComplete,
    WindowClosed(iced::window::Id),
    UpdateGpuPowerState(usize, String),
    UpdateBlockState(usize, bool),
    FetchedSetting(Result<(DaemonSettings, Option<bool>, Option<Mode>), String>),
    FetchedPciList(BTreeMap<String, PciDevice>),
    PciListToClipboard(),
    ToggleMenu(Option<usize>),
    SetGpuBlock(usize, bool),
    GpuBlockResult(Result<(usize, bool), String>),
    RequestLsof(usize),
    LsofResult(Result<LsofData, String>),
    CloseLsofWindow,
    RefreshGpu,
    RefreshGpuResult(Result<(), String>),
    FetchedLogs(Result<VecDeque<LogEntry>, String>),
    NewLog(LogEntry),
    FetchedAppPolicies(
        Result<std::collections::HashMap<String, crate::models::DbusAppMetadata>, String>,
    ),
    SetAppPolicy(String, i32),
    AppPolicyResult(Result<(String, i32), String>),
    UpdateSmartSearch(String),
    RefreshSmartPolicies,
    OpenUrl(String),
    ClearError,
    ClearInfo,
    None,
}
