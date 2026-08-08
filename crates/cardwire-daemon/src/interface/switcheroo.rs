use std::{
    collections::{BTreeMap, HashMap}, sync::{Arc, OnceLock}
};

use log::warn;
use tokio::sync::RwLock;
use zbus::{
    interface, object_server::SignalEmitter, zvariant::{self, OwnedValue, Value}
};

use crate::{core::env::compute_switcheroo_env, interface::GpuInterface};

#[derive(Clone)]
pub struct SwitcherooInterface {
    pub gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>,
    pub signal_emitter: Arc<OnceLock<SignalEmitter<'static>>>,
}
impl SwitcherooInterface {
    pub fn build(gpu_list: Arc<RwLock<BTreeMap<usize, Arc<GpuInterface>>>>) -> Self {
        Self {
            gpu_list,
            signal_emitter: Arc::new(OnceLock::new()),
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
            (
                Self::has_dual_gpu_locked(&gpu_list),
                Self::num_gpus_locked(&gpu_list),
                Self::gpus_locked(&gpu_list),
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

    /// true if exactly two GPUs are available, see has_dual_gpu()
    fn has_dual_gpu_locked(gpu_list: &BTreeMap<usize, Arc<GpuInterface>>) -> bool {
        Self::num_gpus_locked(gpu_list) == 2
    }

    /// number of available GPUs, see num_gpus()
    fn num_gpus_locked(gpu_list: &BTreeMap<usize, Arc<GpuInterface>>) -> u32 {
        gpu_list
            .values()
            .filter(|gpu| gpu.device.is_available())
            .count() as u32
    }

    /// Build the GPUs property payload from a gpu list, see gpus()
    fn gpus_locked(
        gpu_list: &BTreeMap<usize, Arc<GpuInterface>>,
    ) -> Vec<HashMap<&'static str, OwnedValue>> {
        let mut vec: Vec<HashMap<&str, OwnedValue>> = Vec::new();
        let available_gpus: Vec<&Arc<GpuInterface>> = gpu_list
            .values()
            .filter(|gpu| gpu.device.is_available())
            .collect();
        let gpu_count = available_gpus.len();

        for gpu in available_gpus {
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
        Self::has_dual_gpu_locked(&gpu_list)
    }

    #[zbus(property, name = "NumGPUs")]
    pub async fn num_gpus(&self) -> u32 {
        let gpu_list = self.gpu_list.read().await;
        Self::num_gpus_locked(&gpu_list)
    }

    #[zbus(property, name = "GPUs")]
    pub async fn gpus(&self) -> Vec<HashMap<&'static str, OwnedValue>> {
        let gpu_list = self.gpu_list.read().await;
        Self::gpus_locked(&gpu_list)
    }
}
