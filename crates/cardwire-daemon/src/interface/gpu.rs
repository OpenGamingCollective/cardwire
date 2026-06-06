//! DBUS Interface for single gpu interaction

use std::{
    collections::{BTreeMap, HashMap}, ffi::OsStr, fs::{self, read_dir}, path::{Path, PathBuf}, sync::Arc
};

use crate::{
    core::{
        gpu::{DbusGpuDevice, GpuDevice}, pci::PciDevice
    }, file::{CardwireGpuState, CardwireModeState}, interface::Modes
};
use cardwire_ebpf::{BlockKind, EbpfBlocker};
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
    pub device: GpuDevice,
    blocker: Arc<RwLock<EbpfBlocker>>,
    pub pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
    gpu_state: Arc<RwLock<CardwireGpuState>>,
    mode_state: Arc<RwLock<CardwireModeState>>,
}

impl GpuInterface {
    pub fn build(
        device: GpuDevice,
        blocker: Arc<RwLock<EbpfBlocker>>,
        pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
        gpu_state: Arc<RwLock<CardwireGpuState>>,
        mode_state: Arc<RwLock<CardwireModeState>>,
    ) -> anyhow::Result<GpuInterface> {
        Ok(Self {
            device,
            blocker,
            pci_list,
            gpu_state,
            mode_state,
        })
    }
}

impl GpuInterface {
    /// block the gpu
    pub async fn block_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;
        // First block the card id
        blocker
            .block_kind(&self.device.card().to_string(), BlockKind::Card)
            .into_fdo()?;
        // block the render id
        blocker
            .block_kind(&self.device.render().to_string(), BlockKind::Render)
            .into_fdo()?;
        // block the pci
        blocker
            .block_kind(self.device.pci.pci_address(), BlockKind::Pci)
            .into_fdo()?;
        // Also block the audio card
        if self.device.pci.pci_address().ends_with(".0") {
            let gpu_audio_adress = self.device.pci.pci_address().replace(".0", ".1");
            blocker
                .block_kind(&gpu_audio_adress, BlockKind::Pci)
                .into_fdo()?;
        }
        // Check if gpu has a parent pci
        // first pci to block
        let mut current_parent = self.device.pci.parent_pci().clone();

        while let Some(parent_pci) = current_parent {
            if let Some(pci_device) = pci_list.get(&parent_pci) {
                blocker
                    .block_kind(pci_device.pci_address(), BlockKind::Pci)
                    .into_fdo()?;
                current_parent = pci_device.parent_pci().clone();
            } else {
                warn!("expected parent pci {} not found in pci_list", parent_pci);
                break;
            }
        }
        // the last one, block nvidia
        if self.device.nvidia() {
            blocker
                .block_kind(
                    &self.device.nvidia_minor().unwrap().to_string(),
                    BlockKind::Nvidia,
                )
                .into_fdo()?;
        }
        Ok(())
    }
    /// unblock the gpu
    pub async fn unblock_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;
        // First unblock the card id
        blocker
            .unblock_kind(&self.device.card().to_string(), BlockKind::Card)
            .into_fdo()?;
        // unblock the render id
        blocker
            .unblock_kind(&self.device.render().to_string(), BlockKind::Render)
            .into_fdo()?;
        // unblock the pci
        blocker
            .unblock_kind(self.device.pci.pci_address(), BlockKind::Pci)
            .into_fdo()?;
        // Also unblock the audio card
        if self.device.pci.pci_address().ends_with(".0") {
            let gpu_audio_adress = self.device.pci.pci_address().replace(".0", ".1");
            blocker
                .unblock_kind(&gpu_audio_adress, BlockKind::Pci)
                .into_fdo()?;
        }
        // Check if gpu has a parent pci
        // first pci to unblock
        let mut current_parent = self.device.pci.parent_pci().clone();

        while let Some(parent_pci) = current_parent {
            if let Some(pci_device) = pci_list.get(&parent_pci) {
                blocker
                    .unblock_kind(pci_device.pci_address(), BlockKind::Pci)
                    .into_fdo()?;
                current_parent = pci_device.parent_pci().clone();
            } else {
                warn!("expected parent pci {} not found in pci_list", parent_pci);
                break;
            }
        }
        // the last one, unblock nvidia
        if self.device.nvidia() {
            blocker
                .unblock_kind(
                    &self.device.nvidia_minor().unwrap().to_string(),
                    BlockKind::Nvidia,
                )
                .into_fdo()?;
        }
        Ok(())
    }
    /// check if the gpu is blocked
    pub async fn gpu_blocked(&self) -> fdo::Result<bool> {
        let blocker = self.blocker.read().await;

        Ok(blocker
            .is_kind_blocked(self.device.pci.pci_address(), BlockKind::Pci)
            .into_fdo()?
            && blocker
                .is_kind_blocked(&self.device.card().to_string(), BlockKind::Card)
                .into_fdo()?
            && blocker
                .is_kind_blocked(&self.device.render().to_string(), BlockKind::Render)
                .into_fdo()?
            && if let Some(minor) = self.device.nvidia_minor() {
                blocker
                    .is_kind_blocked(&minor.to_string(), BlockKind::Nvidia)
                    .into_fdo()?
            } else {
                true
            })
    }
    /// read fd link to find which apps opened the gpu
    async fn lsof_read(&self, s: &str) -> fdo::Result<Vec<String>> {
        let proc_path = Path::new("/proc");
        let mut proc_found: Vec<String> = Vec::new();
        // If proc path doesn't exist, exit
        if !proc_path.exists() || !proc_path.is_dir() {
            return Err(fdo::Error::Failed("couldn't find /proc path".to_string()));
        }
        // read /proc
        for entry in read_dir(proc_path)
            .map_err(|e| fdo::Error::IOError(e.to_string()))?
            .flatten()
        {
            // Check if folder name is a numerical, if not skip
            if let Ok(string) = entry.file_name().into_string()
                && string.parse::<u32>().is_err()
            {
                continue;
            }
            let path = entry.path();
            // now read eg: /proc/1
            if path.is_dir() {
                // now get fd directory
                let fd_dir: PathBuf = read_dir(&path)
                    .map_err(|e| fdo::Error::IOError(e.to_string()))?
                    .filter(|r| r.is_ok())
                    .map(|r| r.unwrap().path())
                    .filter(|r| r.file_name() == Some(OsStr::new("fd")))
                    .collect();
                for entry in read_dir(fd_dir)
                    .map_err(|e| fdo::Error::IOError(e.to_string()))?
                    .flatten()
                {
                    if let Ok(link) = entry.path().read_link()
                        && let Some(file) = link.to_str()
                    {
                        let file = file.to_string();
                        if file.contains(s) {
                            // Found the file, now get process name
                            let status_read = fs::read_to_string(path.join("status"));
                            let mut process_name: String = String::new();
                            if let Ok(status) = status_read {
                                process_name =
                                    status.lines().filter(|l| l.contains("Name:")).collect();
                                process_name = process_name
                                    .split(":")
                                    .last()
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                            }
                            proc_found.push(process_name);
                        }
                    }
                }
            }
        }
        Ok(proc_found)
    }
}

#[interface(name = "com.github.opengamingcollective.cardwire.Gpu")]
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
        if block {
            // Don't block if default
            if self.device.is_default() {
                return Err(fdo::Error::AccessDenied(format!(
                    "GPU {} is the default device and cannot be blocked",
                    self.device.name()
                )));
            }
            // Now block
            self.block_gpu().await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            let mut gpu_state = self.gpu_state.write().await;
            if let Err(e) = gpu_state.save_state(&self.device, true).await {
                warn!("could not save gpu_state to file: {e}");
            };
            Ok(())
        } else {
            // unblock
            self.unblock_gpu().await?;
            info!("Set GPU {} block={}", self.device.name(), block);
            // save new state to file
            let mut gpu_state = self.gpu_state.write().await;
            if let Err(e) = gpu_state.save_state(&self.device, false).await {
                warn!("could not save gpu_state to file: {e}");
            };
            Ok(())
        }
    }

    #[zbus(property)]
    pub async fn block(&self) -> fdo::Result<bool> {
        self.gpu_blocked().await
    }
    pub async fn lsof(&self) -> fdo::Result<HashMap<String, Vec<String>>> {
        let card_path = format!("/dev/dri/card{}", self.device.card());
        let render_path = format!("/dev/dri/renderD{}", self.device.render());
        let mut proc_map: HashMap<String, Vec<String>> = HashMap::new();

        let (card, render) =
            tokio::try_join!(self.lsof_read(&card_path), self.lsof_read(&render_path))?;
        proc_map.insert(card_path, card);
        proc_map.insert(render_path, render);

        if let Some(minor) = self.device.nvidia_minor() {
            let nvidia_path = format!("/dev/nvidia{}", minor);
            let nvidiactl_path = "/dev/nvidiactl".to_string();
            let (nvidia, nvidiactl) = tokio::try_join!(
                self.lsof_read(&nvidia_path),
                self.lsof_read(&nvidiactl_path)
            )?;
            proc_map.insert(nvidia_path, nvidia);
            proc_map.insert(nvidiactl_path, nvidiactl);
        }

        Ok(proc_map)
    }
    pub async fn get_device(&self) -> fdo::Result<DbusGpuDevice> {
        let gpu = &self.device;
        Ok(DbusGpuDevice {
            pci: gpu.pci.pci_address().to_string(),
            render: *gpu.render(),
            name: gpu.name().to_string(),
            card: *gpu.card(),
            default: gpu.default().unwrap_or(false),
            nvidia: gpu.nvidia(),
            nvidia_minor: if gpu.nvidia_minor().is_some() {
                gpu.nvidia_minor().unwrap().to_string()
            } else {
                "".to_string()
            },
        })
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
}
