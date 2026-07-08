//! main lib code of cardwire-ebpf
mod errors;

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{HashMap, MapError, RingBuf}, programs::{Lsm, TracePoint}
};

pub enum EbpfSettings {
    ExperimentalNvidia,
}

pub struct EbpfBlocker {
    ebpf: Ebpf,
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

        Ok(Self { ebpf })
    }

    /// whitelist cardwire's pid to prevent self-locking in ebpf
    pub fn whitelist_cardwire_pid(&mut self, pid: u32) -> CardwireEbpfResult<()> {
        let mut inode_map: HashMap<_, u8, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("cardwire_daemon_pid")
                .ok_or_else(|| CardwireEbpfError::missing_map("cardwire_daemon_pid"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(0, pid, 0)
            .map_err(|err| CardwireEbpfError::Aya(err.to_string()))
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

    pub fn block_inode(&mut self, inode: u64) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("cardwire_blocked_inodes")
                .ok_or_else(|| CardwireEbpfError::missing_map("cardwire_blocked_inodes"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(inode, 1, 0)
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }
    pub fn unblock_inode(&mut self, inode: u64) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("cardwire_blocked_inodes")
                .ok_or_else(|| CardwireEbpfError::missing_map("cardwire_blocked_inodes"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match inode_map.get(&inode, 0) {
            // Ok = key found, remove
            Ok(_) => inode_map.remove(&inode).map_err(CardwireEbpfError::aya),
            // key not found, skip
            Err(MapError::KeyNotFound) => Ok(()),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn is_inode_blocked(&self, inode: u64) -> CardwireEbpfResult<bool> {
        // Also insert hardcoded values for now
        let inode_map: HashMap<_, u64, u8> = HashMap::try_from(
            self.ebpf
                .map("cardwire_blocked_inodes")
                .ok_or_else(|| CardwireEbpfError::missing_map("cardwire_blocked_inodes"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match inode_map.get(&inode, 0) {
            Ok(_) => Ok(true),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn set_ebpf_setting(&mut self, setting: EbpfSettings, value: u8) -> CardwireEbpfResult<()> {
        let key: u8 = match setting {
            EbpfSettings::ExperimentalNvidia => 0,
        };
        let mut setting_map: HashMap<_, u8, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("cardwire_settings")
                .ok_or_else(|| CardwireEbpfError::missing_map("cardwire_settings"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        setting_map
            .insert(key, value, 0)
            .map_err(CardwireEbpfError::aya)
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
        let map = self.ebpf.take_map("cardwire_mode").unwrap();
        let map: HashMap<aya::maps::MapData, u8, u8> = HashMap::try_from(map).unwrap();
        Ok(map)
    }
}
