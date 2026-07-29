use std::{
    collections::{BTreeMap, HashMap}, sync::Arc
};

use tokio::sync::RwLock;
use zbus::{
    interface, zvariant::{self, OwnedValue, Value}
};

use crate::interface::GpuInterface;

#[derive(Clone)]
pub struct SwitcherooInterface {
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
}
impl SwitcherooInterface {
    pub fn build(gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>) -> Self {
        Self { gpu_list }
    }
}

#[interface(name = "net.hadess.SwitcherooControl")]
impl SwitcherooInterface {
    #[zbus(property, name = "HasDualGpu")]
    pub async fn has_dual_gpu(&self) -> bool {
        let gpu_list = self.gpu_list.read().await;
        gpu_list.iter().count().eq(&2)
    }

    #[zbus(property, name = "NumGPUs")]
    pub async fn num_gpus(&self) -> u32 {
        let gpu_list = self.gpu_list.read().await;
        gpu_list.iter().count() as u32
    }

    #[zbus(property, name = "GPUs")]
    pub async fn gpus(&self) -> Vec<HashMap<&'static str, OwnedValue>> {
        let mut vec: Vec<HashMap<&str, OwnedValue>> = Vec::new();
        let gpu_list = self.gpu_list.read().await;
        for (_, gpu) in gpu_list.iter() {
            let mut dict = HashMap::new();

            // The name (s)
            let name_str = zvariant::Str::from(gpu.device.name());
            dict.insert("Name", OwnedValue::from(name_str));
            // Env Vars
            let env_vars = if !gpu.device.is_default() {
                // It's the dGPU, force DGPU
                vec!["CARDWIRE_FORCE_DGPU".to_string(), "1".to_string()]
            } else {
                // It's the iGPU, don't allow the dGPU
                vec!["CARDWIRE_ALLOW".to_string(), "0".to_string()]
            };
            let env_val = Value::from(env_vars);
            dict.insert("Environment", OwnedValue::try_from(env_val).unwrap());
            // "Default" (b)
            dict.insert("Default", OwnedValue::from(gpu.device.is_default()));
            // "Discrete" (b)
            dict.insert("Discrete", OwnedValue::from(!gpu.device.is_default()));

            vec.push(dict);
        }

        vec
    }
}
