//! main lib code of cardwire-ebpf
mod errors;

use std::{fs, path::Path};

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{Array, HashMap, MapError, RingBuf}, programs::{Lsm, TracePoint}
};
use aya_log::EbpfLogger;
use log::{Log, error, info, warn};
use tokio::io::{Interest, unix::AsyncFd};

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

        /*
           This part can get rejected by the kernel if the lockdown is enabled, we warn but we do not exit carwired, it will just run in a weakened state
           sys_exit_getdents64 re-write userspace memory to hide an entry (file/folder), it can be rejected
        */

        let mut did_sys_exit_getdents64_success = false;

        // to hide files
        let cardwire_sys_exit_getdents64: &mut TracePoint = ebpf
            .program_mut("tracepoint_exit_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_exit_getdents64"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        // Try to load the program, if success attach it, else just warn the user
        match cardwire_sys_exit_getdents64
            .load()
            .map_err(CardwireEbpfError::aya)
        {
            Ok(_) => {
                did_sys_exit_getdents64_success = true;
                cardwire_sys_exit_getdents64
                    .attach("syscalls", "sys_exit_getdents64")
                    .map_err(CardwireEbpfError::aya)?;
            }
            Err(err) => {
                // If we cannot load the program, it usually mean the kernel lockdown is enabled
                let lockdown = is_lockdown_enabled();
                warn!(
                    "Failed to load sys_exit_getdents64. Lockdown status: {}",
                    lockdown
                );
                warn!("{}", err);
                warn!("falling back to a weakened cardwired...");
            }
        };
        // to hide files
        let cardwire_sys_enter_getdents64: &mut TracePoint = ebpf
            .program_mut("tracepoint_enter_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_enter_getdents64"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        if did_sys_exit_getdents64_success {
            match cardwire_sys_enter_getdents64
                .load()
                .map_err(CardwireEbpfError::aya)
            {
                Ok(_) => {
                    cardwire_sys_enter_getdents64
                        .attach("syscalls", "sys_enter_getdents64")
                        .map_err(CardwireEbpfError::aya)?;
                }
                Err(err) => {
                    let lockdown = is_lockdown_enabled();
                    warn!(
                        "Failed to load sys_enter_getdents64. Lockdown status: {}",
                        lockdown
                    );
                    warn!("{}", err);
                    warn!("falling back to a weakened cardwired...");
                }
            };
        }
        // Try to load the program, if success attach it, else just warn the user

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

    pub fn is_inode_blocked(&self, inode: u64, value: u32) -> CardwireEbpfResult<bool> {
        // Also insert hardcoded values for now
        let inode_map: HashMap<_, u64, u32> = HashMap::try_from(
            self.ebpf
                .map("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        match inode_map.get(&inode, 0) {
            // if value (gpu key associed to inode) = our func value
            Ok(map_value) => Ok(value == map_value),
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

    /// Turn a comm string into a 16-byte key with a guaranteed terminating NUL
    pub fn comm_to_key(comm: &str) -> [u8; 16] {
        let mut key = [0u8; 16];
        let bytes = comm.as_bytes();
        let len = bytes.len().min(15);
        key[..len].copy_from_slice(&bytes[..len]);
        key
    }

    pub fn allow_comm(&mut self, comm: &str) -> CardwireEbpfResult<()> {
        let comm = Self::comm_to_key(comm);
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

    /// take the CW_EXEC_EVENTS RingBuf map from the blocker
    pub fn get_exec_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map_str = "CW_EXEC_EVENTS";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let ring_buf: RingBuf<aya::maps::MapData> = match RingBuf::try_from(map) {
            Ok(ringbuf) => ringbuf,
            Err(err) => {
                error!("error while trying to get the exec ring_buf");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(ring_buf)
    }

    /// take the CW_CLOSE_EVENTS RingBuf map from the blocker
    pub fn get_close_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map_str = "CW_CLOSE_EVENTS";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let ring_buf: RingBuf<aya::maps::MapData> = match RingBuf::try_from(map) {
            Ok(ringbuf) => ringbuf,
            Err(err) => {
                error!("error while trying to get the close ring_buf");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(ring_buf)
    }

    /// take the CW_REPORT_EVENTS RingBuf map from the blocker
    pub fn get_report_ring(&mut self) -> CardwireEbpfResult<RingBuf<aya::maps::MapData>> {
        let map_str = "CW_REPORT_EVENTS";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let ring_buf: RingBuf<aya::maps::MapData> = match RingBuf::try_from(map) {
            Ok(ringbuf) => ringbuf,
            Err(err) => {
                error!("error while trying to get the report ring_buf");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(ring_buf)
    }

    /// take the CW_ALLOWED_PID HashMap map from the blocker
    pub fn get_pid_map(&mut self) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map_str = "CW_ALLOWED_PID";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let map: HashMap<aya::maps::MapData, u32, u32> = match HashMap::try_from(map) {
            Ok(map) => map,
            Err(err) => {
                error!("error while trying to get the allowed_pid map");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(map)
    }

    /// take the CW_FORCED_PID HashMap map from the blocker
    pub fn get_forced_pid_map(
        &mut self,
    ) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map_str = "CW_FORCED_PID";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let map: HashMap<aya::maps::MapData, u32, u32> = match HashMap::try_from(map) {
            Ok(map) => map,
            Err(err) => {
                error!("error while trying to get the forced_pid map");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(map)
    }

    /// take the CW_MODE Array map from the blocker
    pub fn get_mode_map(&mut self) -> CardwireEbpfResult<Array<aya::maps::MapData, u8>> {
        let map_str = "CW_MODE";
        let map = match self.ebpf.take_map(map_str) {
            Some(map) => map,
            None => {
                error!("error while trying to take map {}", map_str);
                return Err(CardwireEbpfError::MissingMap {
                    name: map_str.to_string(),
                });
            }
        };
        let array: Array<aya::maps::MapData, u8> = match Array::try_from(map) {
            Ok(array) => array,
            Err(err) => {
                error!("error while trying to get the mode array");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(array)
    }

    pub fn get_ebpf_logger(
        &mut self,
    ) -> Result<AsyncFd<EbpfLogger<&'static dyn Log>>, CardwireEbpfError> {
        let logger = match EbpfLogger::init(&mut self.ebpf) {
            Ok(logger) => logger,
            Err(err) => {
                error!("failed to initialize eBPF logger");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        let async_fd = match AsyncFd::with_interest(logger, Interest::READABLE) {
            Ok(fd) => fd,
            Err(err) => {
                error!("couldn't get the async_fd for ebpf_logger");
                return Err(CardwireEbpfError::aya(err));
            }
        };
        Ok(async_fd)
    }
}

fn is_lockdown_enabled() -> bool {
    let path = Path::new("/sys/kernel/security/lockdown");
    if let Ok(entry) = fs::read_to_string(path)
        && (entry.contains("[integrity]") || entry.contains("[confidentiality]"))
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comm_to_key_short_string() {
        let key = EbpfBlocker::comm_to_key("pacman");
        assert_eq!(&key[..6], b"pacman");
        assert_eq!(&key[6..], &[0u8; 10]);
    }

    #[test]
    fn test_comm_to_key_exact_15_bytes() {
        let name = "123456789012345";
        let key = EbpfBlocker::comm_to_key(name);
        assert_eq!(&key[..15], name.as_bytes());
        assert_eq!(key[15], 0);
    }

    #[test]
    fn test_comm_to_key_truncates_to_15_bytes_reserving_nul() {
        let name = "1234567890123456789";
        let key = EbpfBlocker::comm_to_key(name);
        assert_eq!(&key[..15], b"123456789012345");
        assert_eq!(key[15], 0);
    }
}
