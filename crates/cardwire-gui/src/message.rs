use crate::{
    helpers::GpuDevice, models::{DaemonSettings, Mode, Page}
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
    FetchedSetting(Result<(DaemonSettings, Option<bool>, Option<Mode>), String>),
    ClearError,
}
