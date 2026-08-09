//! DBUS Interface for single gpu interaction

use std::{
    collections::{BTreeMap, HashMap}, fs, sync::{Arc, OnceLock}
};

use crate::{
    core::{
        gpu::{DbusGpuDevice, GpuDevice, is_gpu_active, send_drm_uevent}, inode::{card_to_inode, get_inodes, nvidia_to_inode, render_to_inode, single_pci_to_inode}, pci::PciDevice, procfs
    }, file::{CardwireGpuState, CardwireModeState}, interface::Modes
};
use cardwire_ebpf_userspace::EbpfBlocker;
use log::{info, warn};
use tokio::sync::RwLock;
use zbus::{fdo, interface, object_server::SignalEmitter};

pub trait FdoResultExt<T> {
    fn into_fdo(self) -> fdo::Result<T>;
}

impl<T, E: std::fmt::Display> FdoResultExt<T> for Result<T, E> {
    fn into_fdo(self) -> fdo::Result<T> {
        self.map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

// Represent a single gpu
#[derive(Clone)]
pub struct GpuInterface {
    pub id: u32,
    pub device: Arc<GpuDevice>,
    pub env: Arc<OnceLock<Vec<String>>>,
    blocker: Arc<RwLock<EbpfBlocker>>,
    pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    mode_state: Arc<RwLock<CardwireModeState>>,
    pub signal_emitter: Arc<OnceLock<SignalEmitter<'static>>>,
}

impl GpuInterface {
    pub fn build(
        id: u32,
        device: GpuDevice,
        env: Vec<String>,
        blocker: Arc<RwLock<EbpfBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        mode_state: Arc<RwLock<CardwireModeState>>,
    ) -> anyhow::Result<GpuInterface> {
        Ok(Self {
            id,
            device: Arc::new(device),
            env: Arc::new(OnceLock::from(env)),
            blocker,
            pci_list,
            gpu_state,
            mode_state,
            signal_emitter: Arc::new(OnceLock::new()),
        })
    }
}

impl GpuInterface {
    /// block the gpu, value = gpu key
    pub async fn block_gpu(&self, value: u32) -> fdo::Result<()> {
        let (render, card, pci_address, pci_parent, nvidia_minor, pci_list) = {
            let pci_list_guard = self.pci_list.read().await;

            (
                *self.device.render(),
                *self.device.card(),
                self.device.pci().pci_address().to_owned(),
                self.device.pci().parent_pci().to_owned(),
                *self.device.nvidia_minor(),
                pci_list_guard.clone(),
            )
        };

        let inodes = tokio::task::spawn_blocking(move || {
            get_inodes(
                render,
                card,
                &pci_address,
                &pci_parent,
                &pci_list,
                nvidia_minor,
            )
        })
        .await
        .into_fdo()?
        .into_fdo()?;

        let mut blocker = self.blocker.write().await;

        for inode in inodes {
            blocker.block_inode(inode, value).into_fdo()?;
        }

        Ok(())
    }

    /// unblock the gpu
    pub async fn unblock_gpu(&self) -> fdo::Result<()> {
        let (render, card, pci_address, pci_parent, nvidia_minor, pci_list) = {
            let pci_list_guard = self.pci_list.read().await;

            (
                *self.device.render(),
                *self.device.card(),
                self.device.pci().pci_address().to_owned(),
                self.device.pci().parent_pci().to_owned(),
                *self.device.nvidia_minor(),
                pci_list_guard.clone(),
            )
        };

        // Read the inodes required to unblock the GPU, return if err
        let inodes = tokio::task::spawn_blocking(move || {
            get_inodes(
                render,
                card,
                &pci_address,
                &pci_parent,
                &pci_list,
                nvidia_minor,
            )
        })
        .await
        .into_fdo()?
        .into_fdo()?;
        let mut blocker = self.blocker.write().await;

        for inode in inodes.iter() {
            blocker.unblock_inode(*inode).into_fdo()?;
        }
        Ok(())
    }
    /// check if the gpu is blocked
    pub async fn gpu_blocked(&self) -> fdo::Result<bool> {
        let blocker = self.blocker.read().await;

        let gpu_id = self.id;

        let card = match card_to_inode(*self.device.card()) {
            Ok(inode) => blocker.is_inode_blocked(inode, gpu_id).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        let render = match render_to_inode(*self.device.render()) {
            Ok(inode) => blocker.is_inode_blocked(inode, gpu_id).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        let pci = match single_pci_to_inode(self.device.pci.pci_address()) {
            Ok(inode) => blocker.is_inode_blocked(inode, gpu_id).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        let nvidia = match self.device.nvidia_minor() {
            // GPU is nvidia
            Some(minor) => {
                if let Ok(inode) = nvidia_to_inode(*minor) {
                    blocker.is_inode_blocked(inode, gpu_id).into_fdo()?
                } else {
                    false
                }
            }
            // GPU isnt nvidia, ignore but keep true
            None => true,
        };

        Ok(card && render && pci && nvidia)
    }
}

#[interface(name = "org.opengamingcollective.cardwire.Gpu")]
impl GpuInterface {
    #[zbus(property)]
    pub async fn set_block(&mut self, block: bool) -> fdo::Result<()> {
        let mode = self.mode_state.read().await;
        if mode.mode() != Modes::Manual {
            return Err(fdo::Error::AccessDenied(
                "Per GPU block is only available on manual mode".to_string(),
            ));
        }
        drop(mode);
        if !self.device.is_available() {
            return Err(fdo::Error::AccessDenied(format!(
                "GPU {} is not available and cannot be blocked",
                self.device.name()
            )));
        }
        if block {
            // Don't block if default
            if self.device.is_default() {
                return Err(fdo::Error::AccessDenied(format!(
                    "GPU {} is the default device and cannot be blocked",
                    self.device.name()
                )));
            }
            match is_gpu_active(*self.device.card()).await {
                Some(true) => {
                    return Err(fdo::Error::AccessDenied(format!(
                        "GPU {} is active (monitor IN) and cannot be blocked",
                        self.device.name()
                    )));
                }
                None => {
                    return Err(fdo::Error::Failed(format!(
                        "could not probe display state of GPU {}; refusing to block",
                        self.device.name()
                    )));
                }
                Some(false) => {}
            }
            // Now block
            self.block_gpu(self.id).await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            {
                let mut gpu_state = self.gpu_state.write().await;
                if let Err(e) = gpu_state.save_state(&self.device, true).await {
                    warn!("could not save gpu_state to file: {e}");
                };
            }
            if let Err(err) = send_drm_uevent(*self.device.card()).await {
                warn!(
                    "failed to send drm uevent for {}: {err}",
                    self.device.name()
                );
            };

            Ok(())
        } else {
            // unblock
            self.unblock_gpu().await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            {
                let mut gpu_state = self.gpu_state.write().await;
                if let Err(e) = gpu_state.save_state(&self.device, false).await {
                    warn!("could not save gpu_state to file: {e}");
                };
            }
            if let Err(err) = send_drm_uevent(*self.device.card()).await {
                warn!(
                    "failed to send drm uevent for {}: {err}",
                    self.device.name()
                );
            };
            Ok(())
        }
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        let mode = self.mode_state.read().await.mode();
        if mode == Modes::Smart && (self.device.is_default() && !self.device.is_discrete()) {
            return Ok(false);
        }
        self.gpu_blocked().await
    }

    /// perform a lsof on the GPU nodes
    pub async fn lsof(&self) -> fdo::Result<HashMap<String, Vec<String>>> {
        let card_path = format!("/dev/dri/card{}", self.device.card());
        let render_path = format!("/dev/dri/renderD{}", self.device.render());
        let mut proc_map: HashMap<String, Vec<String>> = HashMap::new();

        async fn read_lsof(path: String) -> fdo::Result<Vec<String>> {
            tokio::task::spawn_blocking(move || procfs::lsof_read(&path))
                .await
                .map_err(|e| fdo::Error::Failed(e.to_string()))?
                .map_err(|e| fdo::Error::Failed(e.to_string()))
        }

        let (card, render) =
            tokio::try_join!(read_lsof(card_path.clone()), read_lsof(render_path.clone()),)?;
        proc_map.insert(card_path, card);
        proc_map.insert(render_path, render);

        if let Some(minor) = self.device.nvidia_minor() {
            let nvidia_path = format!("/dev/nvidia{}", minor);
            let nvidiactl_path = "/dev/nvidiactl".to_string();

            let (nvidia, nvidiactl) = tokio::try_join!(
                read_lsof(nvidia_path.clone()),
                read_lsof(nvidiactl_path.clone()),
            )?;
            proc_map.insert(nvidia_path, nvidia);
            proc_map.insert(nvidiactl_path, nvidiactl);
        }

        Ok(proc_map)
    }
    pub async fn get_device(&self) -> fdo::Result<DbusGpuDevice> {
        Ok(DbusGpuDevice::from(&*self.device))
    }

    pub async fn power_state(&self) -> fdo::Result<String> {
        let power_path = format!(
            "/sys/bus/pci/devices/{}/power_state",
            self.device.pci.pci_address()
        );
        fs::read_to_string(&power_path).map_err(|e| {
            fdo::Error::IOError(format!(
                "error while trying to read {} power_state: {}",
                self.device.name(),
                e
            ))
        })
    }

    #[zbus(signal)]
    pub async fn power_state_changed(emitter: &SignalEmitter<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(property)]
    pub async fn env(&self) -> Vec<String> {
        match self.env.get() {
            Some(v) => v.clone(),
            None => Vec::new(),
        }
    }
}
