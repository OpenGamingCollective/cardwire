mod errors;

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{HashMap, MapError}, programs::Lsm
};
pub struct EbpfBlocker {
    ebpf: Ebpf,
}

impl EbpfBlocker {
    pub fn new() -> CardwireEbpfResult<Self> {
        if !Self::is_bpf_enabled() {
            return Err(CardwireEbpfError::LSMNotEnabled);
        }
        let mut ebpf = match Ebpf::load(aya::include_bytes_aligned!(concat!(
            env!("OUT_DIR"),
            "/bpf.o"
        ))) {
            Ok(ebpf) => ebpf,
            Err(e) => return Err(CardwireEbpfError::EbpfLoadError(e.to_string())),
        };

        let btf = Btf::from_sys_fs().map_err(CardwireEbpfError::aya)?;

        let program_file_open: &mut Lsm = ebpf
            .program_mut("file_open")
            .ok_or_else(|| Self::missing_entity("program", "file_open"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        program_file_open
            .load("file_open", &btf)
            .map_err(CardwireEbpfError::aya)?;
        program_file_open.attach().map_err(CardwireEbpfError::aya)?;

        // For lsm/inode_permission
        let program_inode_permission: &mut Lsm = ebpf
            .program_mut("inode_permission")
            .ok_or_else(|| Self::missing_entity("program", "inode_permission"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        program_inode_permission
            .load("inode_permission", &btf)
            .map_err(CardwireEbpfError::aya)?;
        program_inode_permission
            .attach()
            .map_err(CardwireEbpfError::aya)?;

        // For lsm/inode_getattr
        let program_inode_getattr: &mut Lsm = ebpf
            .program_mut("inode_getattr")
            .ok_or_else(|| Self::missing_entity("program", "inode_getattr"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        program_inode_getattr
            .load("inode_getattr", &btf)
            .map_err(CardwireEbpfError::aya)?;
        program_inode_getattr
            .attach()
            .map_err(CardwireEbpfError::aya)?;

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

    fn missing_entity(kind: &str, name: &str) -> CardwireEbpfError {
        CardwireEbpfError::missing_entity(kind, name)
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

    /*
       This part is for blocking a specific CardID
    */

    pub fn block_card(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_card(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_card_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_CARDID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_CARDID"))?,
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
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_render(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_RENDERID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_render_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_RENDERID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_RENDERID"))?,
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
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        map.insert(id, 1, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_nvidia(&mut self, id: u32) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_NVIDIAID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let _ = map.remove(&id);
        Ok(())
    }

    pub fn is_nvidia_blocked(&self, id: u32) -> CardwireEbpfResult<bool> {
        let map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_NVIDIAID")
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_NVIDIAID"))?,
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
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
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
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
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
                .ok_or_else(|| Self::missing_entity("map", "BLOCKED_PCI"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        let key = Self::pci_key(pci);
        match map.get(&key, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn set_vulkan_block(&mut self, block: bool) -> CardwireEbpfResult<()> {
        let mut map: HashMap<_, u32, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("SETTINGS")
                .ok_or_else(|| Self::missing_entity("map", "SETTINGS"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        if block {
            map.insert(0, 1, 0).map_err(CardwireEbpfError::aya)?;
        } else {
            let _ = map.remove(&0);
        }
        Ok(())
    }
}
