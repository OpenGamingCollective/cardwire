mod errors;

use crate::errors::CardwireBPFError;
use aya::maps::{HashMap, MapError};
use aya::programs::Lsm;
use aya::{Btf, Ebpf};
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufReader, Error as IoError, ErrorKind, Read};

pub struct EbpfBlocker {
    ebpf: Ebpf,
}

impl EbpfBlocker {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::is_bpf_enabled()?;
        let mut ebpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
            env!("OUT_DIR"),
            "/bpf.o"
        )))?;

        let btf = Btf::from_sys_fs()?;

        let program: &mut Lsm = ebpf
            .program_mut("file_open")
            .ok_or_else(|| Self::missing_entity("program", "file_open"))?
            .try_into()?;
        program.load("file_open", &btf)?;
        program.attach()?;

        Ok(Self { ebpf })
    }

    fn pci_key(pci: &str) -> [u8; 16] {
        let mut key = [0u8; 16];
        let bytes = pci.as_bytes();
        let len = bytes.len().min(15);
        key[..len].copy_from_slice(&bytes[..len]);
        key[len] = 0;
        key
    }

    fn missing_entity(kind: &str, name: &str) -> IoError {
        IoError::new(ErrorKind::NotFound, format!("{} not found: {}", kind, name))
    }

    /*
       Checks if bpf/lsm is enabled in the kernel
    */
    fn is_bpf_enabled() -> Result<(), CardwireBPFError> {
        // Method 1
        if let Ok(lsm) = std::fs::read_to_string("/sys/kernel/security/lsm") {
            match lsm.contains("bpf") {
                true => return Ok(()),
                false => return Err(CardwireBPFError::LSMNotEnabled),
            };
        };

        // Method 2 if the first one didnt work
        let file = match File::open("/proc/config.gz") {
            Ok(f) => f,
            Err(_) => return Err(CardwireBPFError::LSMNotEnabled),
        };

        let file = BufReader::new(file);
        let mut gz = GzDecoder::new(file);
        let mut config = String::new();

        gz.read_to_string(&mut config)?;

        match config.contains("CONFIG_BPF_LSM=y") {
            true => return Ok(()),
            false => return Err(CardwireBPFError::LSMNotEnabled),
        }
    }

    /*
       This part is for blocking a specific CardID
    */

    pub fn block_card(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
        )?;
        map.insert(id, 1, 0)?;
        Ok(())
    }

    pub fn unblock_card(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
        )?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_card_blocked(&self, id: u32) -> Result<bool, Box<dyn std::error::Error>> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
        )?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
    /*
       This part is for blocking a specific RenderID
    */

    pub fn block_render(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_RENDERID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
        )?;
        map.insert(id, 1, 0)?;
        Ok(())
    }

    pub fn unblock_render(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_RENDERID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
        )?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_render_blocked(&self, id: u32) -> Result<bool, Box<dyn std::error::Error>> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_RENDERID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
        )?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
    /*
       This part is for blocking a specific NvidiaID
    */

    pub fn block_nvidia(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIAID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
        )?;
        map.insert(id, 1, 0)?;
        Ok(())
    }

    pub fn unblock_nvidia(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIAID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
        )?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_nvidia_blocked(&self, id: u32) -> Result<bool, Box<dyn std::error::Error>> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_NVIDIAID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
        )?;
        match map.get(&id, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
    /*
       This part is for blocking a specific PCI
    */
    pub fn block_pci(&mut self, pci: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_PCI")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
        )?;
        let key = Self::pci_key(pci);
        map.insert(key, 1, 0)?;
        Ok(())
    }

    pub fn unblock_pci(&mut self, pci: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_PCI")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
        )?;
        let key = Self::pci_key(pci);
        let _ = map.remove(&key);
        Ok(())
    }

    pub fn is_pci_blocked(&self, pci: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_PCI")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
        )?;
        let key = Self::pci_key(pci);
        match map.get(&key, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
}
