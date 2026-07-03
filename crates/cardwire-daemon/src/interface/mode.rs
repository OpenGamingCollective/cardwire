//! Define the mode dbus
use crate::{
    file::{CardwireGpuState, CardwireModeState}, interface::{GpuInterface, config::ConfigMemory}
};
use anyhow::Result;
use aya::maps::HashMap as AyaHashMap;
use cardwire_ebpf::EbpfBlocker;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use zbus::{fdo, interface};
#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Modes {
    Integrated,
    Hybrid,
    #[default]
    Manual,
    Smart,
}

impl fmt::Display for Modes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Modes::Integrated => write!(f, "Integrated"),
            Modes::Hybrid => write!(f, "Hybrid"),
            Modes::Manual => write!(f, "Manual"),
            Modes::Smart => write!(f, "Smart"),
        }
    }
}

/// convert a u32 into a mode
impl TryFrom<u32> for Modes {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            3 => Ok(Self::Smart),
            _ => Err("unknown mode"),
        }
    }
}

/// Convert a mode into a u32 and reverse
impl From<Modes> for u32 {
    fn from(value: Modes) -> Self {
        match value {
            Modes::Integrated => 0,
            Modes::Hybrid => 1,
            Modes::Manual => 2,
            Modes::Smart => 3,
        }
    }
}

// to change a mode, we need the config, the mode_state, the gpu_list
#[derive(Clone)]
pub struct ModeInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<ConfigMemory>,
    mode_map: Arc<Mutex<AyaHashMap<aya::maps::MapData, u8, u8>>>,
}

impl ModeInterface {
    pub async fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<ConfigMemory>,
        blocker: Arc<RwLock<EbpfBlocker>>,
    ) -> Result<ModeInterface> {
        let mut blocker = blocker.write().await;
        let mode_map: aya::maps::HashMap<aya::maps::MapData, u8, u8> = blocker.get_mode_map()?;
        let mode_map = Arc::new(Mutex::new(mode_map));
        Ok(ModeInterface {
            mode_state,
            gpu_state,
            gpu_list,
            config,
            mode_map,
        })
    }
    async fn insert_to_map(&self, mode: Modes) -> fdo::Result<()> {
        let mut mode_map = self.mode_map.lock().await;
        let mode: u32 = Modes::into(mode);
        mode_map
            .insert(0, mode as u8, 0)
            .map_err(|err| fdo::Error::Failed(err.to_string()))
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Mode")]
impl ModeInterface {
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::try_from(mode).map_err(|err| fdo::Error::InvalidArgs(err.to_string()))?;
        let mut current_mode = self.mode_state.write().await;
        let mut gpu_list = self.gpu_list.write().await;
        match mode {
            // Integrated/Hybrid only works on laptop with two gpus, will refuse if the computer has
            // more than 2 gpus
            Modes::Integrated | Modes::Hybrid | Modes::Smart => {
                if gpu_list.len() != 2 {
                    error!(
                        "Couldn't set mode to {}, the mode require exactly 2 GPUs",
                        mode
                    );
                    return Err(fdo::Error::NotSupported(format!(
                        "Couldn't set mode to {}, the mode require exactly 2 GPUs",
                        mode
                    )));
                }
                // Loop to find the non default gpu and block it,
                for gpu in gpu_list.values_mut() {
                    if !gpu.device.is_default() {
                        if mode == Modes::Integrated || mode == Modes::Smart {
                            gpu.block_gpu().await?;
                        } else {
                            gpu.unblock_gpu().await?;
                        }
                    };
                }
            }
            // If the auto apply is false, return all gpus to unblocked
            // Else apply the gpu_state but still unblock other gpus
            Modes::Manual => {
                //let gpu_state = self.state.gpu_state.read().await;
                let config = self
                    .config
                    .auto_apply_gpu_state
                    .load(std::sync::atomic::Ordering::Relaxed);
                let gpu_state = self.gpu_state.read().await;
                for (_, gpu) in gpu_list.iter_mut() {
                    if gpu_state.gpu_block_state(gpu.device.pci().pci_address()) && config {
                        if gpu.device.is_default() {
                            // For safety, warn and unblock if default
                            warn!(
                                "auto_apply_gpu_state tried to block gpu: {}, which is the default gpu, unblocking for safety...",
                                gpu.device.name()
                            );
                            gpu.unblock_gpu().await?;
                        } else {
                            info!("blocking: {} ", gpu.device.pci().pci_address());
                            gpu.block_gpu().await?;
                        }
                    } else {
                        gpu.unblock_gpu().await?;
                    }
                }
            }
        }
        self.insert_to_map(mode).await?;
        if let Err(e) = current_mode.save_state(mode).await {
            warn!("mode couldn't be saved to config: {e}");
        }
        info!("Switched to {}", mode);
        Ok(())
    }
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        let current_mode = self.mode_state.read().await;
        Ok(Modes::into(current_mode.mode()))
    }
}
