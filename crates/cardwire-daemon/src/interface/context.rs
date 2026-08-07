//! Shared daemon state passed to interface constructors.
use crate::{
    core::pci::PciDevice, file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, GpuInterface}
};
use cardwire_ebpf_userspace::EbpfBlocker;
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::RwLock, task};

/// Group of shared `Arc`s used by every D-Bus interface and the daemon's background tasks.
#[derive(Clone)]
pub struct DaemonContext {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<EbpfBlocker>>,
    pub power_tasks: Arc<RwLock<BTreeMap<usize, task::JoinHandle<anyhow::Result<()>>>>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
}
