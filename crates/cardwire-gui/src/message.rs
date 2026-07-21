use crate::{
    helpers::GpuDevice, models::{Mode, Page}
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    FetchedMode(Result<Mode, String>),
    SetMode(Mode),
    AllDevicesFetched(Result<BTreeMap<usize, GpuDevice>, String>),
    ClearError,
}
