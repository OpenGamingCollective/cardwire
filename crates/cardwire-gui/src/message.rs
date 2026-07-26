use crate::{
    helpers::GpuDevice, models::{DaemonSettings, LsofData, Mode, Page, PciDevice}
};
use std::collections::BTreeMap;

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
    OpenUrl(String),
    ClearError,
    ClearInfo,
    #[expect(dead_code)]
    // Placeholder used when building a new feature
    None,
}
