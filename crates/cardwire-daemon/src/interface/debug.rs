use cardwire_core::{gpu::GpuBlocker, pci::PciDevice};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use zbus::{fdo, interface, object_server::SignalEmitter};

use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{ConfigMemory, GpuInterface}
};

#[derive(Clone)]
pub struct DebugInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<ConfigMemory>,
    pub blocker: Arc<RwLock<GpuBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
}
impl DebugInterface {
    pub fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<GpuBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    ) -> anyhow::Result<DebugInterface> {
        Ok(DebugInterface {
            mode_state,
            gpu_state,
            gpu_list,
            config,
            blocker,
            pci_list,
        })
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Debug")]
impl DebugInterface {
    async fn diagnostic_gpu(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let emitter = emitter.into_owned();
        emitter.diagnostic_info("Hello").await;
        Ok(())
    }
    #[zbus(signal)]
    async fn diagnostic_info(emitter: &SignalEmitter<'_>, text: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn diagnostic_ended(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
