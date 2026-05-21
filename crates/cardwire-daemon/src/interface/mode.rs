//! Define the mode dbus
use crate::{
    file::{CardwireConfig, CardwireModeState}, interface::GpuInterface
};
use anyhow::Result;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, sync::Arc};
use tokio::sync::RwLock;
use zbus::{fdo, fdo::Error, interface};

#[derive(Deserialize, Serialize, PartialEq, zbus::zvariant::Type, Clone, Copy, Default)]
pub enum Modes {
    Integrated,
    Hybrid,
    #[default]
    Manual,
}

impl fmt::Display for Modes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Modes::Integrated => write!(f, "Integrated"),
            Modes::Hybrid => write!(f, "Hybrid"),
            Modes::Manual => write!(f, "Manual"),
        }
    }
}

impl Modes {
    pub fn parse(input: &u32) -> zbus::fdo::Result<Modes> {
        match input {
            0 => Ok(Self::Integrated),
            1 => Ok(Self::Hybrid),
            2 => Ok(Self::Manual),
            unknown => Err(Error::InvalidArgs(format!(
                "unknown mode: {unknown} \n expected integrated|hybrid|manual"
            ))),
        }
    }
}

// to change a mode, we need the config, the mode_state, the gpu_list
#[derive(Clone)]
pub struct ModeInterface {
    pub mode_state: Arc<RwLock<CardwireModeState>>,
    pub gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
    pub config: Arc<RwLock<CardwireConfig>>,
}

impl ModeInterface {
    pub fn build(
        mode_state: Arc<RwLock<CardwireModeState>>,
        gpu_list: Arc<RwLock<BTreeMap<usize, GpuInterface>>>,
        config: Arc<RwLock<CardwireConfig>>,
    ) -> Result<ModeInterface> {
        Ok(ModeInterface {
            mode_state,
            gpu_list,
            config,
        })
    }
}

#[interface(
    name = "com.github.opengamingcollective.Mode",
    proxy(
        default_service = "com.github.opengamingcollective.cardwire",
        default_path = "/com/github/opengamingcollective/cardwire"
    )
)]
impl ModeInterface {
    /*
        Set the mode
    */
    #[zbus(property)]
    pub(crate) async fn set_mode(&self, mode: u32) -> fdo::Result<()> {
        // Valide inputs and turn into a Modes
        let mode = Modes::parse(&mode)?;
        let mut current_mode = self.mode_state.write().await;
        let mut gpu_list = self.gpu_list.write().await;
        match mode {
            // Integrated/Hybrid only works on laptop with two gpus, will refuse if the computer has
            // more than 2 gpus
            Modes::Integrated | Modes::Hybrid => {
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
                        if mode == Modes::Integrated {
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
                for (_, gpu) in gpu_list.iter_mut() {
                    gpu.unblock_gpu().await?;
                }
            }
        }
        if let Err(e) = current_mode.save_state(mode).await {
            warn!("mode couldn't be saved to config: {e}");
        }
        info!("Switched to {}", mode);
        Ok(())
    }
    #[zbus(property)]
    pub(crate) async fn mode(&self) -> fdo::Result<u32> {
        let current_mode = self.mode_state.read().await;
        match current_mode.mode() {
            Modes::Integrated => Ok(0),
            Modes::Hybrid => Ok(1),
            Modes::Manual => Ok(2),
        }
    }
}
