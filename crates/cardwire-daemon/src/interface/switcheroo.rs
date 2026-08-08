use std::{
    collections::{BTreeMap, HashMap}, sync::{Arc, OnceLock}
};

use log::warn;
use tokio::sync::RwLock;
use zbus::{
    interface, object_server::SignalEmitter, zvariant::{self, OwnedValue, Value}
};

use crate::{core::gpu::GpuVendor, interface::GpuInterface};

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

pub fn compute_switcheroo_env(
    gpu_count: usize,
    is_default: bool,
    is_discrete: bool,
    gpu_id: u32,
    vendor: GpuVendor,
    pci_address: &str,
) -> Vec<String> {
    // Return early for the default display GPU
    if is_default {
        return if is_discrete {
            // Primary dGPU (desktop): allow direct access to primary discrete GPU
            vec!["CARDWIRE_ALLOW".to_string(), "1".to_string()]
        } else {
            // Primary iGPU (laptop): restrict default rendering to integrated GPU
            vec!["CARDWIRE_ALLOW".to_string(), "0".to_string()]
        };
    }

    let mut env = Vec::new();

    // Cardwire-specific routing variable
    if gpu_count == 2 && is_discrete {
        // Dual-GPU hybrid setup: force offloaded process onto dGPU
        env.push("CARDWIRE_FORCE_DGPU".to_string());
        env.push("1".to_string());
    } else {
        // Multi-GPU (3+) or secondary iGPU: route using explicit GPU ID
        env.push("CARDWIRE_FORCE_GPU".to_string());
        env.push(gpu_id.to_string());
    }

    // Standard switcheroo-control / Mesa / NVIDIA offload variables
    // DRI_PRIME=pci-<addr> selects the render node by PCI address
    let dri_prime_val = format!("pci-{}", pci_address.replace([':', '.'], "_"));
    match vendor {
        GpuVendor::Nvidia => {
            env.push("__NV_PRIME_RENDER_OFFLOAD".to_string());
            env.push("1".to_string());
            env.push("__GLX_VENDOR_LIBRARY_NAME".to_string());
            env.push("nvidia".to_string());
            env.push("__VK_LAYER_NV_optimus".to_string());
            env.push("NVIDIA_only".to_string());
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*nvidia*,*nouveau*".to_string());
        }
        GpuVendor::Amd => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*radeon*".to_string());
        }
        GpuVendor::Intel => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*intel*".to_string());
        }
        GpuVendor::Other => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
        }
    }

    env
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_switcheroo_env_laptop_hybrid_nvidia() {
        // Laptop: iGPU default, Nvidia dGPU secondary
        let igpu_env = compute_switcheroo_env(2, true, false, 0, GpuVendor::Intel, "0000:00:02.0");
        assert_eq!(igpu_env, vec!["CARDWIRE_ALLOW", "0"]);

        let dgpu_env = compute_switcheroo_env(2, false, true, 1, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(
            dgpu_env,
            vec![
                "CARDWIRE_FORCE_DGPU",
                "1",
                "__NV_PRIME_RENDER_OFFLOAD",
                "1",
                "__GLX_VENDOR_LIBRARY_NAME",
                "nvidia",
                "__VK_LAYER_NV_optimus",
                "NVIDIA_only",
                "VK_LOADER_DRIVERS_SELECT",
                "*nvidia*,*nouveau*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_laptop_hybrid_amd() {
        // Laptop: iGPU default, AMD dGPU secondary
        let dgpu_env = compute_switcheroo_env(2, false, true, 1, GpuVendor::Amd, "0000:03:00.0");
        assert_eq!(
            dgpu_env,
            vec![
                "CARDWIRE_FORCE_DGPU",
                "1",
                "DRI_PRIME",
                "pci-0000_03_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_desktop_dgpu_primary() {
        // Desktop: dGPU default, iGPU secondary
        let dgpu_env = compute_switcheroo_env(2, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(dgpu_env, vec!["CARDWIRE_ALLOW", "1"]);

        let igpu_env = compute_switcheroo_env(2, false, false, 1, GpuVendor::Amd, "0000:0d:00.0");
        assert_eq!(
            igpu_env,
            vec![
                "CARDWIRE_FORCE_GPU",
                "1",
                "DRI_PRIME",
                "pci-0000_0d_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_single_gpu() {
        let single_env =
            compute_switcheroo_env(1, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(single_env, vec!["CARDWIRE_ALLOW", "1"]);

        let single_igpu =
            compute_switcheroo_env(1, true, false, 0, GpuVendor::Intel, "0000:00:02.0");
        assert_eq!(single_igpu, vec!["CARDWIRE_ALLOW", "0"]);
    }

    #[test]
    fn test_compute_switcheroo_env_multi_gpu() {
        let default_env =
            compute_switcheroo_env(3, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(default_env, vec!["CARDWIRE_ALLOW", "1"]);

        let secondary_amd =
            compute_switcheroo_env(3, false, true, 2, GpuVendor::Amd, "0000:04:00.0");
        assert_eq!(
            secondary_amd,
            vec![
                "CARDWIRE_FORCE_GPU",
                "2",
                "DRI_PRIME",
                "pci-0000_04_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }
}
