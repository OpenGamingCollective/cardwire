//! main lib code of cardwire-ebpf
mod errors;

use std::fmt;

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{HashMap, MapError, RingBuf}, programs::{Lsm, TracePoint}
};
pub struct EbpfBlocker {
    ebpf: Ebpf,
}

#[derive(PartialEq)]
pub enum MapKind {
    CardwirePid,
    CurrentMode,
    Settings,
    BlockedInodes,
    AllowedPid,
}
impl fmt::Display for MapKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MapKind::CardwirePid => write!(f, "CARDWIRE_PID"),
            MapKind::CurrentMode => write!(f, "CURRENT_MODE"),
            MapKind::Settings => write!(f, "SETTINGS"),
            MapKind::BlockedInodes => write!(f, "BLOCKED_INODES"),
            MapKind::AllowedPid => write!(f, "ALLOWED_PID"),
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
                .ok_or_else(|| CardwireEbpfError::missing_lsm(entity))?
                .try_into()
                .map_err(CardwireEbpfError::aya)?;
            program.load(entity, &btf).map_err(CardwireEbpfError::aya)?;
            program.attach().map_err(CardwireEbpfError::aya)?;
        }

        let exec_program: &mut TracePoint = ebpf
            .program_mut("trace_exec")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("trace_exec"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        exec_program.load().map_err(CardwireEbpfError::aya)?;
        exec_program
            .attach("sched", "sched_process_exec")
            .map_err(CardwireEbpfError::aya)?;

        let close_program: &mut TracePoint = ebpf
            .program_mut("trace_process_exit")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("trace_process_exit"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        close_program.load().map_err(CardwireEbpfError::aya)?;
        close_program
            .attach("sched", "sched_process_exit")
            .map_err(CardwireEbpfError::aya)?;
        // to hide files
        let cardwire_sys_enter_getdents64: &mut TracePoint = ebpf
            .program_mut("cardwire_sys_enter_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("cardwire_sys_enter_getdents64"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        cardwire_sys_enter_getdents64
            .load()
            .map_err(CardwireEbpfError::aya)?;

        cardwire_sys_enter_getdents64
            .attach("syscalls", "sys_enter_getdents64")
            .map_err(CardwireEbpfError::aya)?;
        // to hide files
        let cardwire_sys_exit_getdents64: &mut TracePoint = ebpf
            .program_mut("cardwire_sys_exit_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("cardwire_sys_exit_getdents64"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        cardwire_sys_exit_getdents64
            .load()
            .map_err(CardwireEbpfError::aya)?;

        cardwire_sys_exit_getdents64
            .attach("syscalls", "sys_exit_getdents64")
            .map_err(CardwireEbpfError::aya)?;

        // get pid of process and push to ebpf
        let pid = std::process::id();
        println!("pushing this pid: {}", pid);
        let mut inode_map: HashMap<_, u8, u32> = HashMap::try_from(
            ebpf.map_mut("CARDWIRE_PID")
                .ok_or_else(|| CardwireEbpfError::missing_map("CARDWIRE_PID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map.insert(0, pid, 0);

        Ok(Self { ebpf })
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
        Block a kind
    */
    //pub fn block_kind(&mut self, entity: &str, kind: BlockKind) -> CardwireEbpfResult<()> {
    //    // validate input format for the bpf map, else return Err
    //    if !Self::is_format_valid(entity, &kind) {
    //        return Err(CardwireEbpfError::WrongFormat {
    //            kind: kind.to_string(),
    //            input: entity.to_string(),
    //        });
    //    }
    //
    //    let kind_string = kind.to_string();
    //
    //    match kind {
    //        BlockKind::Pci => {
    //            //let mut map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
    //            //    self.ebpf
    //            //        .map_mut(&kind_string)
    //            //        .ok_or_else(|| CardwireEbpfError::missing_map(&kind_string))?,
    //            //)
    //            //.map_err(CardwireEbpfError::aya)?;
    //            //
    //            //let key = Self::pci_key(entity);
    //            //map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
    //        }
    //        // set file blocklist
    //        BlockKind::NvidiaFile | BlockKind::File => {
    //            let mut map: HashMap<_, [u8; 30], u8> = HashMap::try_from(
    //                self.ebpf
    //                    .map_mut(&kind_string)
    //                    .ok_or_else(|| CardwireEbpfError::missing_map(&kind_string))?,
    //            )
    //            .map_err(CardwireEbpfError::aya)?;
    //            let key = Self::file_key(entity);
    //            map.insert(key, 1, 0).map_err(CardwireEbpfError::aya)?;
    //        }
    //        BlockKind::NvidiaSetting => {
    //            if entity.parse::<bool>().is_ok() {
    //                let mut map: HashMap<_, u32, u8> = HashMap::try_from(
    //                    self.ebpf
    //                        .map_mut(&kind_string)
    //                        .ok_or_else(|| CardwireEbpfError::missing_map(&kind_string))?,
    //                )
    //                .map_err(CardwireEbpfError::aya)?;
    //                map.insert(0, 1, 0).map_err(CardwireEbpfError::aya)?;
    //            }
    //        }
    //        BlockKind::Render | BlockKind::Card | BlockKind::Nvidia => {
    //            //let mut map: HashMap<_, u32, u8> = HashMap::try_from(
    //            //    self.ebpf
    //            //        .map_mut(&kind_string)
    //            //        .ok_or_else(|| CardwireEbpfError::missing_map(&kind_string))?,
    //            //)
    //            //.map_err(CardwireEbpfError::aya)?;
    //            //
    //            //if let Ok(value) = entity.parse::<u32>() {
    //            //    map.insert(value, 1, 0).map_err(CardwireEbpfError::aya)?;
    //            //}
    //
    //            // Also insert hardcoded values for now
    //            let mut inode_map: HashMap<_, u64, u8> = HashMap::try_from(
    //                self.ebpf
    //                    .map_mut("BLOCKED_INODES")
    //                    .ok_or_else(|| CardwireEbpfError::missing_map(&kind_string))?,
    //            )
    //            .map_err(CardwireEbpfError::aya)?;
    //            // card1 = 431
    //            inode_map
    //                .insert(431, 1, 0)
    //                .map_err(CardwireEbpfError::aya)?;
    //            // renderD128 = 430
    //            inode_map
    //                .insert(430, 1, 0)
    //                .map_err(CardwireEbpfError::aya)?;
    //            // 0000:03:00.0 = 13757
    //            inode_map
    //                .insert(13757, 1, 0)
    //                .map_err(CardwireEbpfError::aya)?;
    //            // 0000:03:00.1 = 13838
    //            inode_map
    //                .insert(13838, 1, 0)
    //                .map_err(CardwireEbpfError::aya)?;
    //        }
    //    }
    //
    //    Ok(())
    //}
    //

    pub fn block_inode(&mut self, inode: u64) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_INODES")
                .ok_or_else(|| CardwireEbpfError::missing_map("BLOCKED_INODES"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        println!("adding {} to map", inode);
        inode_map
            .insert(inode, 1, 0)
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }
    pub fn unblock_inode(&mut self, inode: u64) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("BLOCKED_INODES")
                .ok_or_else(|| CardwireEbpfError::missing_map("BLOCKED_INODES"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map.remove(&inode).map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn is_inode_blocked(&self, inode: u64) -> CardwireEbpfResult<(bool)> {
        // Also insert hardcoded values for now
        let inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map("BLOCKED_INODES")
                .ok_or_else(|| CardwireEbpfError::missing_map("BLOCKED_INODES"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        return match inode_map.get(&inode, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        };
    }

    pub fn get_exec_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map = self.ebpf.take_map("EXEC_EVENTS").unwrap();
        let ring_buf: RingBuf<aya::maps::MapData> = RingBuf::try_from(map).unwrap();
        Ok(ring_buf)
    }
    pub fn get_close_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map = self.ebpf.take_map("CLOSE_EVENTS").unwrap();
        let ring_buf: RingBuf<aya::maps::MapData> = RingBuf::try_from(map).unwrap();
        Ok(ring_buf)
    }
    pub fn get_pid_map(&mut self) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u8>> {
        let map = self.ebpf.take_map("ALLOWED_PID").unwrap();
        let map: HashMap<aya::maps::MapData, u32, u8> = HashMap::try_from(map).unwrap();
        Ok(map)
    }
    pub fn get_mode_map(&mut self) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u8, u8>> {
        let map = self.ebpf.take_map("CURRENT_MODE").unwrap();
        let map: HashMap<aya::maps::MapData, u8, u8> = HashMap::try_from(map).unwrap();
        Ok(map)
    }
}
