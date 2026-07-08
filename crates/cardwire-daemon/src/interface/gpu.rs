//! DBUS Interface for single gpu interaction

use std::{
    collections::{BTreeMap, HashMap}, ffi::OsStr, fs::{self, read_dir}, path::{Path, PathBuf}, sync::Arc
};

use crate::{
    core::{
        gpu::{DbusGpuDevice, GpuDevice, GpuVendor}, inode::{card_to_inode, pci_to_inode, render_to_inode, single_pci_to_inode}, pci::PciDevice
    }, file::{CardwireGpuState, CardwireModeState}, interface::Modes
};
use cardwire_ebpf::EbpfBlocker;
use log::{error, info, warn};
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
    pci_list: Arc<RwLock<BTreeMap<String, PciDevice>>>,
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
        match card_to_inode(*self.device.card()) {
            Ok(inode) => blocker.block_inode(inode).into_fdo()?,
            Err(err) => {
                error!("failed to block card{}: {}", *self.device.card(), err);
                return Err(err).into_fdo();
            }
        };
        match render_to_inode(*self.device.render()) {
            Ok(inode) => blocker.block_inode(inode).into_fdo()?,
            Err(err) => {
                error!("failed to block render{}: {}", *self.device.render(), err);
                return Err(err).into_fdo();
            }
        };
        match pci_to_inode(
            self.device.pci.pci_address().to_string(),
            self.device.pci.parent_pci(),
            &pci_list,
        ) {
            Ok(inodes) => {
                for inode in inodes {
                    if let Err(err) = blocker.block_inode(inode) {
                        error!("failed to block inode(pci) {}: {}", inode, err);
                        return Err(err).into_fdo();
                    }
                }
            }
            Err(err) => {
                error!(
                    "failed to block pci {}: {}",
                    self.device.pci.pci_address(),
                    err
                );
                return Err(err).into_fdo();
            }
        };
        // the last one, block nvidia
        //if self.device.gpu_vendor() == GpuVendor::Nvidia
        //    && let Some(minor) = self.device.nvidia_minor()
        //{
        //    blocker
        //        .block_kind(&minor.to_string(), BlockKind::Nvidia)
        //        .into_fdo()?;
        //}
        Ok(())
    }
    /// unblock the gpu
    pub async fn unblock_gpu(&mut self) -> fdo::Result<()> {
        let mut blocker = self.blocker.write().await;
        let pci_list = self.pci_list.read().await;
        // First unblock the card id
        match card_to_inode(*self.device.card()) {
            Ok(inode) => blocker.unblock_inode(inode).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        match render_to_inode(*self.device.render()) {
            Ok(inode) => blocker.unblock_inode(inode).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        match pci_to_inode(
            self.device.pci.pci_address().to_string(),
            self.device.pci.parent_pci(),
            &pci_list,
        ) {
            Ok(inodes) => {
                for inode in inodes {
                    blocker.unblock_inode(inode).into_fdo()?
                }
            }
            Err(err) => return Err(err).into_fdo(),
        };
        // the last one, unblock nvidia
        //if self.device.gpu_vendor() == GpuVendor::Nvidia
        //    && let Some(minor) = self.device.nvidia_minor()
        //{
        //    blocker
        //        .unblock_kind(&minor.to_string(), BlockKind::Nvidia)
        //        .into_fdo()?;
        //}
        Ok(())
    }
    /// check if the gpu is blocked
    pub async fn gpu_blocked(&self) -> fdo::Result<bool> {
        let blocker = self.blocker.read().await;

        let card = match card_to_inode(*self.device.card()) {
            Ok(inode) => blocker.is_inode_blocked(inode).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        let render = match render_to_inode(*self.device.render()) {
            Ok(inode) => blocker.is_inode_blocked(inode).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };
        let pci = match single_pci_to_inode(self.device.pci.pci_address()) {
            Ok(inode) => blocker.is_inode_blocked(inode).into_fdo()?,
            Err(err) => return Err(err).into_fdo(),
        };

        Ok(card && render && pci)
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
                            let comm_read = fs::read_to_string(path.join("comm"));
                            let mut process_name: String = String::new();
                            if let Ok(comm) = comm_read {
                                process_name = comm.trim_ascii_end().to_string()
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
            nvidia: gpu.gpu_vendor() == GpuVendor::Nvidia,
            nvidia_minor: if let Some(minor) = gpu.nvidia_minor() {
                minor.to_string()
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
