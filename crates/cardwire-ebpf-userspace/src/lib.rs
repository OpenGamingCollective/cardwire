//! main lib code of cardwire-ebpf
mod errors;

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{Array, HashMap, MapError, RingBuf}, programs::{Lsm, TracePoint}
};
use log::info;

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
        let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
            env!("OUT_DIR"),
            "/cardwire-ebpf"
        )))
        .map_err(|err| CardwireEbpfError::EbpfLoadError(err.to_string()))?;

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
            .program_mut("tracepoint_sched_process_exec")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_sched_process_exec"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        exec_program.load().map_err(CardwireEbpfError::aya)?;
        exec_program
            .attach("sched", "sched_process_exec")
            .map_err(CardwireEbpfError::aya)?;

        let close_program: &mut TracePoint = ebpf
            .program_mut("tracepoint_sched_process_exit")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_sched_process_exit"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;
        close_program.load().map_err(CardwireEbpfError::aya)?;
        close_program
            .attach("sched", "sched_process_exit")
            .map_err(CardwireEbpfError::aya)?;
        // to hide files
        let cardwire_sys_enter_getdents64: &mut TracePoint = ebpf
            .program_mut("tracepoint_enter_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_enter_getdents64"))?
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
            .program_mut("tracepoint_exit_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("try_tracepoint_exit_getdents64"))?
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
        let mut array_map: Array<_, u32> = Array::try_from(
            self.ebpf
                .map_mut("CW_DAEMON_PID")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_DAEMON_PID"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        info!("inserting: {} into map", pid);
        array_map.set(0, pid, 0).map_err(CardwireEbpfError::aya)?;
        Ok(())
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

    /// Block an inode, value is the associated GPU id
    pub fn block_inode(&mut self, inode: u64, value: u32) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(inode, value, 0)
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }
    pub fn unblock_inode(&mut self, inode: u64) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
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
        let inode_map: HashMap<_, u64, u32> = HashMap::try_from(
            self.ebpf
                .map("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match inode_map.get(&inode, 0) {
            // 1 = dGPU, 0 = iGPU, if the inode is in the map it means the dGPU is meant to be
            // blocked so true
            Ok(value) => Ok(value == 1),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn block_exp_inode(&mut self, inode: u64, value: u32) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, u64, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_EXP_BLK_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_EXP_BLK_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(inode, value, 0)
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn set_ebpf_setting(&mut self, setting: EbpfSettings, value: u8) -> CardwireEbpfResult<()> {
        let key: u8 = match setting {
            EbpfSettings::ExperimentalNvidia => 0,
        };
        let mut setting_map: HashMap<_, u8, u8> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_SETTINGS")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_SETTINGS"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        setting_map
            .insert(key, value, 0)
            .map_err(CardwireEbpfError::aya)
    }

    pub fn allow_comm(&mut self, comm: &str) -> CardwireEbpfResult<()> {
        // turn the comm str into a char[16]
        let comm = {
            let mut key = [0u8; 16];
            let bytes = comm.as_bytes();
            // leave one byte for terminator
            let len = bytes.len().min(15);
            key[..len].copy_from_slice(&bytes[..len]);
            key[len] = 0;
            key
        };
        let mut allowed_comm_map: HashMap<_, [u8; 16], u8> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_ALLOWED_COMM")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_ALLOWED_COMM"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        allowed_comm_map
            .insert(comm, 0, 0)
            .map_err(CardwireEbpfError::aya)
    }

    pub fn get_exec_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map = self.ebpf.take_map("CW_EXEC_EVENTS").unwrap();
        let ring_buf: RingBuf<aya::maps::MapData> = RingBuf::try_from(map).unwrap();
        Ok(ring_buf)
    }
    pub fn get_close_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map = self.ebpf.take_map("CW_CLOSE_EVENTS").unwrap();
        let ring_buf: RingBuf<aya::maps::MapData> = RingBuf::try_from(map).unwrap();
        Ok(ring_buf)
    }
    pub fn get_report_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map = self.ebpf.take_map("CW_REPORT_EVENTS").unwrap();
        let ring_buf: RingBuf<aya::maps::MapData> = RingBuf::try_from(map).unwrap();
        Ok(ring_buf)
    }
    pub fn get_pid_map(&mut self) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map = self.ebpf.take_map("CW_ALLOWED_PID").unwrap();
        let map: HashMap<aya::maps::MapData, u32, u32> = HashMap::try_from(map).unwrap();
        Ok(map)
    }
    pub fn get_forced_pid_map(
        &mut self,
    ) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map = self.ebpf.take_map("CW_FORCED_PID").unwrap();
        let map: HashMap<aya::maps::MapData, u32, u32> = HashMap::try_from(map).unwrap();
        Ok(map)
    }
    pub fn get_mode_map(&mut self) -> CardwireEbpfResult<Array<aya::maps::MapData, u8>> {
        let map = self.ebpf.take_map("CW_MODE").unwrap();
        let map: Array<aya::maps::MapData, u8> = Array::try_from(map).unwrap();
        Ok(map)
    }
}
