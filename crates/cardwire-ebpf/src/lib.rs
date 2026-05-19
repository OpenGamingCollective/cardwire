//! main lib code of cardwire-ebpf
mod errors;

use std::fmt;

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{HashMap, MapError}, programs::Lsm
};
pub struct EbpfBlocker {
    ebpf: Ebpf,
}

#[derive(PartialEq)]
pub enum BlockKind {
    Card,
    Render,
    Pci,
    Nvidia,
}
impl fmt::Display for BlockKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BlockKind::Card => write!(f, "BLOCKED_CARDID"),
            BlockKind::Render => write!(f, "BLOCKED_RENDERID"),
            BlockKind::Pci => write!(f, "BLOCKED_PCI"),
            BlockKind::Nvidia => write!(f, "BLOCKED_NVIDIAID"),
        }
    }
}

impl EbpfBlocker {
    pub fn new() -> CardwireEbpfResult<Self> {
        // quit if bpf is not enabled
        if !Self::is_bpf_enabled() {
            return Err(CardwireEbpfError::LSMNotEnabled);
        }
        // load the program from the .o
        let mut ebpf = match Ebpf::load(aya::include_bytes_aligned!(concat!(
            env!("OUT_DIR"),
            "/bpf.o"
        ))) {
            Ok(ebpf) => ebpf,
            Err(e) => return Err(CardwireEbpfError::EbpfLoadError(e.to_string())),
        };

        let btf = Btf::from_sys_fs().map_err(CardwireEbpfError::aya)?;

        let load_list: [&str; 3] = ["file_open", "inode_permission", "inode_getattr"];
        for entity in load_list {
            let program: &mut Lsm = ebpf
                .program_mut(entity)
                .ok_or_else(|| CardwireEbpfError::missing_lsm("program", entity))?
                .try_into()
                .map_err(CardwireEbpfError::aya)?;
            program.load(entity, &btf).map_err(CardwireEbpfError::aya)?;
            program.attach().map_err(CardwireEbpfError::aya)?;
        }
        Ok(Self { ebpf })
    }

    /// turn a pci string into a u8 array with a fixed 16 size
    fn pci_key(pci: &str) -> [u8; 16] {
        let mut key = [0u8; 16];
        let bytes = pci.as_bytes();
        // leave one byte for terminator
        let len = bytes.len().min(15);
        key[..len].copy_from_slice(&bytes[..len]);
        key[len] = 0;
        key
    }
    /// turn a file string into a u8 array with a fixed 30 size
    fn file_key(file: &str) -> [u8; 30] {
        let mut key = [0u8; 30];
        let bytes = file.as_bytes();
        // leave one byte for terminator
        let len = bytes.len().min(29);
        key[..len].copy_from_slice(&bytes[..len]);
        key[len] = 0;
        key
    }
    /*
       Checks if bpf/lsm is enabled in the kernel
    */
    fn is_bpf_enabled() -> bool {
        match std::fs::read_to_string("/sys/kernel/security/lsm") {
            Ok(lsm) => lsm.contains("bpf"),
            Err(_) => false,
        }
    }

    fn is_format_valid(entity: &str, kind: &BlockKind) -> bool {
        match kind {
            BlockKind::Render => entity.parse::<u32>().is_ok(),
            BlockKind::Card => entity.parse::<u32>().is_ok(),
            BlockKind::Nvidia => entity.parse::<u32>().is_ok(),
            // Only the Pci need a real check
            BlockKind::Pci => entity.starts_with("0000:") && !entity.contains("pcie"),
        }
    }

    pub fn block_kind(&mut self, entity: &str, kind: BlockKind) -> CardwireEbpfResult<()> {
        // validate input format for the bpf map, else return Err
        if !Self::is_format_valid(entity, &kind) {
            return Err(CardwireEbpfError::WrongFormat {
                kind: kind.to_string(),
                input: entity.to_string(),
            });
        }

        let kind_string = kind.to_string();

        match kind {
            BlockKind::Pci => {
                let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
                    self.ebpf
                        .map_mut("BLOCKED_PCI")
                        .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
                )
                .map_err(CardwireEbpfError::aya)?;

                let key = Self::pci_key(entity);
                map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
            }
            BlockKind::Render | BlockKind::Card | BlockKind::Nvidia => {
                let mut map: HashMap<_, u32, u8> = HashMap::try_from(
                    self.ebpf
                        .map_mut(&kind_string)
                        .ok_or_else(|| CardwireEbpfError::missing_lsm("map", &kind_string))?,
                )
                .map_err(CardwireEbpfError::aya)?;

                if let Ok(value) = entity.parse::<u32>() {
                    map.insert(value, 1, 0).map_err(CardwireEbpfError::aya)?;
                }
            }
        }

        Ok(())
    }

    pub fn unblock_kind(&mut self, entity: &str, kind: BlockKind) -> CardwireEbpfResult<()> {
        // validate input format for the bpf map, else return Err
        if !Self::is_format_valid(entity, &kind) {
            return Err(CardwireEbpfError::WrongFormat {
                kind: kind.to_string(),
                input: entity.to_string(),
            });
        }

        let kind_string = kind.to_string();

        match kind {
            BlockKind::Pci => {
                let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
                    self.ebpf
                        .map_mut("BLOCKED_PCI")
                        .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
                )
                .map_err(CardwireEbpfError::aya)?;

                let value = Self::pci_key(entity);
                let _ = map.remove(&value);
            }
            BlockKind::Render | BlockKind::Card | BlockKind::Nvidia => {
                let mut map: HashMap<_, u32, u8> = HashMap::try_from(
                    self.ebpf
                        .map_mut(&kind_string)
                        .ok_or_else(|| CardwireEbpfError::missing_lsm("map", &kind_string))?,
                )
                .map_err(CardwireEbpfError::aya)?;

                if let Ok(value) = entity.parse::<u32>() {
                    let _ = map.remove(&value);
                }
            }
        }

        Ok(())
    }
    /*
       This part is for blocking a specific CardID
    */

    pub fn block_card(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_CARDID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_card(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_CARDID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_card_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_CARDID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_CARDID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }
    /*
       This part is for blocking a specific RenderID
    */

    pub fn block_render(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_RENDERID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_RENDERID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_render(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_RENDERID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_RENDERID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_render_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_RENDERID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_RENDERID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }
    /*
       This part is for blocking a specific NvidiaID
    */

    pub fn block_nvidia(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIAID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_NVIDIAID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_nvidia(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIAID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_NVIDIAID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_nvidia_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_NVIDIAID")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_NVIDIAID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }
    /*
       This part is for blocking a specific PCI
    */
    pub fn block_pci(&mut self, pci: &str) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_PCI")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::pci_key(pci);
        map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_pci(&mut self, pci: &str) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_PCI")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::pci_key(pci);
        let _ = map.remove(&key);
        Ok(())
    }

    pub fn is_pci_blocked(&self, pci: &str) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_PCI")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::pci_key(pci);
        match map.get(&key, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn set_nvidia_block(&mut self, block: bool) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("SETTINGS")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "SETTINGS"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        if block {
            map.insert(0, 1, 0).map_err(CardwireEbpfError::aya)?;
        } else {
            let _ = map.remove(&0);
        }
        Ok(())
    }

    pub fn set_nvidia_file_block(&mut self, file: &str) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, [u8; 30], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIA_FILES")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::file_key(file);
        map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn set_file_block(&mut self, file: &str) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, [u8; 30], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_PCI_FILES")
                .ok_or_else(|| CardwireEbpfError::missing_lsm("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::file_key(file);
        map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }
}
