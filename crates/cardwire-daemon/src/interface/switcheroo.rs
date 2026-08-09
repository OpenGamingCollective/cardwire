use std::{
    collections::{BTreeMap, HashMap}, sync::{Arc, OnceLock}
};

use log::warn;
use tokio::sync::RwLock;
use zbus::{
    interface, object_server::SignalEmitter, zvariant::{self, OwnedValue, Value}
};

use crate::{
    core::env::{compute_switcheroo_env, is_gpu_launchable}, file::CardwireModeState, interface::GpuInterface, types::Modes
};

#[derive(Clone)]
pub struct SwitcherooInterface {
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    pub signal_emitter: Arc<OnceLock<SignalEmitter<'static>>>,
    mode_state: Arc<RwLock<CardwireModeState>>,
}
impl SwitcherooInterface {
    pub fn build(
        gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
        mode_state: Arc<RwLock<CardwireModeState>>,
    ) -> Self {
        Self {
            gpu_list,
            signal_emitter: Arc::new(OnceLock::new()),
            mode_state,
        }
    }

    /// Emit a PropertiesChanged signal for the three read-only properties, mirroring
    /// upstream switcheroo-control's change notification on GPU list updates
    pub async fn emit_gpu_list_changed(&self) {
        let Some(emitter) = &self.signal_emitter.get() else {
            return;
        };

        let (has_dual_gpu, num_gpus, gpus) = {
            let gpu_list = self.gpu_list.read().await;
            let mode = self.mode_state.read().await.mode();
            (
                Self::has_dual_gpu_locked(&gpu_list, mode).await,
                Self::num_gpus_locked(&gpu_list, mode).await,
                Self::gpus_locked(&gpu_list, mode).await,
            )
        };

        let mut changed: HashMap<&str, OwnedValue> = HashMap::new();
        changed.insert("HasDualGpu", OwnedValue::from(has_dual_gpu));
        changed.insert("NumGPUs", OwnedValue::from(num_gpus));
        let gpus_value = match OwnedValue::try_from(Value::from(gpus)) {
            Ok(value) => value,
            Err(err) => {
                warn!("could not build switcheroo GPUs payload: {err}");
                return;
            }
        };
        changed.insert("GPUs", gpus_value);

        let body = ("net.hadess.SwitcherooControl", changed, Vec::<&str>::new());
        if let Err(err) = emitter
            .emit(
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &body,
            )
            .await
        {
            warn!("failed to emit switcheroo PropertiesChanged: {err}");
        }
    }

    /// true if exactly two GPUs are visible, see has_dual_gpu(). A GPU is visible when it is
    /// available, or blocked in Smart mode
    async fn has_dual_gpu_locked(
        gpu_list: &BTreeMap<usize, Arc<GpuInterface>>,
        mode: Modes,
    ) -> bool {
        Self::num_gpus_locked(gpu_list, mode).await == 2
    }

    /// number of visible GPUs, excluding blocked ones outside of Smart mode, see num_gpus()
    async fn num_gpus_locked(gpu_list: &BTreeMap<usize, Arc<GpuInterface>>, mode: Modes) -> u32 {
        let mut count = 0;
        for gpu in gpu_list.values() {
            if is_gpu_launchable(
                gpu.device.is_available(),
                gpu.gpu_blocked().await.unwrap_or(true),
                mode,
            ) {
                count += 1;
            }
        }
        count
    }

    /// Build the GPUs property payload from a gpu list, excluding blocked GPUs outside of Smart
    /// mode, see gpus(). Blocked GPUs in Smart mode are advertised with the full environment
    async fn gpus_locked(
        gpu_list: &BTreeMap<usize, Arc<GpuInterface>>,
        mode: Modes,
    ) -> Vec<HashMap<&'static str, OwnedValue>> {
        let mut vec: Vec<HashMap<&str, OwnedValue>> = Vec::new();
        let mut visible_gpus: Vec<&Arc<GpuInterface>> = Vec::new();
        for gpu in gpu_list.values() {
            if is_gpu_launchable(
                gpu.device.is_available(),
                gpu.gpu_blocked().await.unwrap_or(true),
                mode,
            ) {
                visible_gpus.push(gpu);
            }
        }
        let gpu_count = visible_gpus.len();

        for gpu in visible_gpus {
            let mut dict = HashMap::new();

            // The name (s)
            let name_str = zvariant::Str::from(gpu.device.name());
            dict.insert("Name", OwnedValue::from(name_str));
            // Env Vars
            let env_vars = compute_switcheroo_env(
                gpu_count,
                gpu.device.is_default(),
                gpu.device.is_discrete(),
                gpu.id,
                gpu.device.gpu_vendor(),
                gpu.device.pci().pci_address(),
            );

            let env_val = Value::from(env_vars);
            match OwnedValue::try_from(env_val) {
                Ok(value) => {
                    dict.insert("Environment", value);
                }
                Err(err) => {
                    warn!(
                        "could not convert switcheroo environment for {}: {err}",
                        gpu.device.name()
                    );
                    continue;
                }
            }
            // "Default" (b)
            dict.insert("Default", OwnedValue::from(gpu.device.is_default()));
            // "Discrete" (b)
            dict.insert("Discrete", OwnedValue::from(gpu.device.is_discrete()));

            vec.push(dict);
        }
        vec
    }
}

#[interface(name = "net.hadess.SwitcherooControl")]
impl SwitcherooInterface {
    #[zbus(property, name = "HasDualGpu")]
    pub async fn has_dual_gpu(&self) -> bool {
        let gpu_list = self.gpu_list.read().await;
        let mode = self.mode_state.read().await.mode();
        Self::has_dual_gpu_locked(&gpu_list, mode).await
    }

    #[zbus(property, name = "NumGPUs")]
    pub async fn num_gpus(&self) -> u32 {
        let gpu_list = self.gpu_list.read().await;
        let mode = self.mode_state.read().await.mode();
        Self::num_gpus_locked(&gpu_list, mode).await
    }

    #[zbus(property, name = "GPUs")]
    pub async fn gpus(&self) -> Vec<HashMap<&'static str, OwnedValue>> {
        let gpu_list = self.gpu_list.read().await;
        let mode = self.mode_state.read().await.mode();
        Self::gpus_locked(&gpu_list, mode).await
    }
}
