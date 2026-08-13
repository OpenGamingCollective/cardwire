//! main lib code of cardwire-ebpf
mod errors;

use std::{fs, path::Path, sync::Arc};

pub use crate::errors::{CardwireEbpfError, CardwireEbpfResult};
use aya::{
    Btf, Ebpf, maps::{Array, HashMap, MapError, RingBuf}, programs::{FEntry, Lsm, TracePoint}
};
use aya_log::EbpfLogger;
use log::{Log, error, info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd}, sync::RwLock
};

pub enum EbpfSettings {
    ExperimentalNvidia,
}

pub struct EbpfBlocker {
    ebpf: Ebpf,
    pub pid_map: Arc<RwLock<HashMap<aya::maps::MapData, u32, u32>>>,
    pub forced_map: Arc<RwLock<HashMap<aya::maps::MapData, u32, u32>>>,
    pushed_exp_inodes: Vec<InodeKey>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct InodeState {
    pub gpu_id: u32,
    pub blocked: u8,
    pub _padding: [u8; 3], // 8-byte alignment
}
unsafe impl aya::Pod for InodeState {}

/// Layout must stay identical to the eBPF side's InodeKey, the kernel hashes
/// the raw key bytes so any drift turns every lookup into a silent miss
#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InodeKey {
    pub dev: u64,
    pub ino: u64,
}
unsafe impl aya::Pod for InodeKey {}

impl InodeKey {
    /// Build a key from the `st_dev`/`st_ino` of a stat() result
    pub fn new(st_dev: u64, ino: u64) -> Self {
        Self {
            dev: kernel_dev(st_dev),
            ino,
        }
    }
}

/// Width of the minor field in the kernel's dev_t, MKDEV shifts the major by
/// this much
const MINOR_BITS: u32 = 20;

/// Repack a glibc `st_dev` into the kernel's dev_t, the eBPF side keys on
/// `(*sb).s_dev` which is already in that form
///
/// MKDEV gives each number one contiguous field. glibc instead cuts both in
/// half and interleaves them: minor bits 0-7 sit at bits 0-7, major bits 0-11
/// at 8-19, the rest of minor at 20+, the rest of major at 44+. Each line
/// below rejoins one number's two halves, and the wide mask discards the other
/// number's bits that the shift dragged into range.
fn kernel_dev(st_dev: u64) -> u64 {
    let major = ((st_dev >> 8) & 0x0000_0fff) | ((st_dev >> 32) & 0xffff_f000);
    let minor = (st_dev & 0x0000_00ff) | ((st_dev >> 12) & 0xffff_ff00);

    (major << MINOR_BITS) | minor
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

        let lsm_load_list: [&str; 3] = ["file_open", "inode_permission", "inode_getattr"];
        for entity in lsm_load_list {
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

        // iterate_dir runs between the two getdents64 tracepoints and supplies
        // the device id the dirents lack
        //
        // Unlike the getdents64 exit hook below, this one writes no userspace
        // memory, so lockdown is not what stops it. It can still fail to load on
        // kernels without bpf trampoline support, or when the build renamed the
        // symbol we attach by name, so degrade instead of refusing to start
        let mut did_iterate_dir_success = false;

        let iterate_dir_program: &mut FEntry = ebpf
            .program_mut("fentry_iterate_dir")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("fentry_iterate_dir"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        match iterate_dir_program
            .load("iterate_dir", &btf)
            .map_err(CardwireEbpfError::aya)
            .and_then(|_| iterate_dir_program.attach().map_err(CardwireEbpfError::aya))
        {
            Ok(_) => {
                did_iterate_dir_success = true;
            }
            Err(err) => {
                warn!(
                    "Failed to load or attach iterate_dir (fentry unsupported, or symbol not attachable): {}",
                    err
                );
                warn!("no device id for dirents, directory listings will not be filtered");
            }
        };

        /*
           This part can get rejected by the kernel if the lockdown is enabled, we warn but we do not exit carwired, it will just run in a weakened state
           sys_exit_getdents64 re-write userspace memory to hide an entry (file/folder), it can be rejected
           Only load sys_enter_getdents64 (syscall that will populate the CW_DIRENT MAP) if sys_exit_getdents64 doesnt fail, else the map will overflow
        */

        let mut did_sys_exit_getdents64_success = false;

        let cardwire_sys_exit_getdents64: &mut TracePoint = ebpf
            .program_mut("tracepoint_exit_getdents64")
            .ok_or_else(|| CardwireEbpfError::missing_lsm("tracepoint_exit_getdents64"))?
            .try_into()
            .map_err(CardwireEbpfError::aya)?;

        // Try to load the program into the kernel, if success attach it, else just warn the user
        // Without the device id from iterate_dir the exit hook cannot build a (dev, ino) key, so it
        // would fail open on every entry: skip it entirely
        if did_iterate_dir_success {
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
        }

        // Now we try to load sys_enter_getdents64

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

        let pid_map = Self::get_pid_map(&mut ebpf)?;
        let forced_map = Self::get_forced_pid_map(&mut ebpf)?;

        let pid_map = Arc::new(RwLock::new(pid_map));
        let forced_map = Arc::new(RwLock::new(forced_map));

        Ok(Self {
            ebpf,
            pid_map,
            forced_map,
            pushed_exp_inodes: Vec::new(),
        })
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

    /// Block a file, value is the associated GPU id
    pub fn block_inode(&mut self, key: InodeKey, gpu_id: u32) -> CardwireEbpfResult<()> {
        let mut inode_map: HashMap<_, InodeKey, InodeState> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(
                key,
                InodeState {
                    gpu_id,
                    blocked: 1,
                    _padding: [0; 3],
                },
                0,
            )
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn unblock_inode(&mut self, key: InodeKey, gpu_id: u32) -> CardwireEbpfResult<()> {
        let mut inode_map: HashMap<_, InodeKey, InodeState> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        // Keep the inode in the map for tracking, but set blocked to 0
        inode_map
            .insert(
                key,
                InodeState {
                    gpu_id,
                    blocked: 0,
                    _padding: [0; 3],
                },
                0,
            )
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    /// Drop a file from the map entirely, a missing key is not an error
    pub fn remove_inode(&mut self, key: InodeKey) -> CardwireEbpfResult<()> {
        let mut inode_map: HashMap<_, InodeKey, InodeState> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;

        match inode_map.remove(&key) {
            Ok(()) | Err(MapError::KeyNotFound) => Ok(()),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn is_inode_blocked(&self, key: InodeKey, gpu_id: u32) -> CardwireEbpfResult<bool> {
        let inode_map: HashMap<_, InodeKey, InodeState> = HashMap::try_from(
            self.ebpf
                .map("CW_BLOCKED_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_BLOCKED_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;

        match inode_map.get(&key, 0) {
            Ok(state) => Ok(state.gpu_id == gpu_id && state.blocked == 1),
            Err(MapError::KeyNotFound) => Ok(false),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    pub fn block_exp_inode(&mut self, key: InodeKey, value: u32) -> CardwireEbpfResult<()> {
        // Also insert hardcoded values for now
        let mut inode_map: HashMap<_, InodeKey, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_EXP_BLK_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_EXP_BLK_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;
        inode_map
            .insert(key, value, 0)
            .map_err(CardwireEbpfError::aya)?;
        Ok(())
    }

    pub fn remove_exp_inode(&mut self, key: InodeKey) -> CardwireEbpfResult<()> {
        let mut inode_map: HashMap<_, InodeKey, u32> = HashMap::try_from(
            self.ebpf
                .map_mut("CW_EXP_BLK_INO")
                .ok_or_else(|| CardwireEbpfError::missing_map("CW_EXP_BLK_INO"))?,
        )
        .map_err(CardwireEbpfError::aya)?;

        match inode_map.remove(&key) {
            Ok(()) | Err(MapError::KeyNotFound) => Ok(()),
            Err(err) => Err(CardwireEbpfError::aya(err)),
        }
    }

    /// `pushed_exp_inodes` mirrors what we put in `CW_EXP_BLK_INO`, so it is only
    /// ever updated once the kernel agrees. Dropping a key from it before the
    /// removal succeeds would leave an entry nothing can name afterwards, and it
    /// would stay blocked until the daemon restarts
    pub fn clear_exp_inodes(&mut self) -> CardwireEbpfResult<()> {
        while let Some(key) = self.pushed_exp_inodes.last().copied() {
            self.remove_exp_inode(key)?;
            self.pushed_exp_inodes.pop();
        }
        Ok(())
    }

    pub fn sync_exp_inodes(&mut self, keys: Vec<InodeKey>, gpu_id: u32) -> CardwireEbpfResult<()> {
        let stale: Vec<InodeKey> = self
            .pushed_exp_inodes
            .iter()
            .copied()
            .filter(|key| !keys.contains(key))
            .collect();

        for key in stale {
            self.remove_exp_inode(key)?;
            self.pushed_exp_inodes.retain(|tracked| *tracked != key);
        }

        for key in keys {
            self.block_exp_inode(key, gpu_id)?;
            if !self.pushed_exp_inodes.contains(&key) {
                self.pushed_exp_inodes.push(key);
            }
        }

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
    pub fn get_pid_map(
        ebpf: &mut Ebpf,
    ) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map_str = "CW_ALLOWED_PID";
        let map = match ebpf.take_map(map_str) {
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
        ebpf: &mut Ebpf,
    ) -> CardwireEbpfResult<HashMap<aya::maps::MapData, u32, u32>> {
        let map_str = "CW_FORCED_PID";
        let map = match ebpf.take_map(map_str) {
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

    /// MKDEV, as the kernel builds s_dev
    fn mkdev(major: u64, minor: u64) -> u64 {
        (major << MINOR_BITS) | minor
    }

    #[test]
    fn anonymous_devices_are_unchanged_by_the_conversion() {
        // tmpfs, sysfs and procfs sit on major 0, where both encodings agree
        for minor in [7u64, 25, 28, 50] {
            assert_eq!(kernel_dev(minor), mkdev(0, minor));
        }
    }

    #[test]
    fn real_block_devices_are_re_encoded() {
        // an nvme partition: glibc packs 259:4 as 66308, the kernel as MKDEV(259, 4)
        assert_eq!(kernel_dev(66308), mkdev(259, 4));
        assert_ne!(kernel_dev(66308), 66308);

        // sd-style major 8
        assert_eq!(kernel_dev(2049), mkdev(8, 1));
    }

    #[test]
    fn conversion_round_trips_every_major_minor_split() {
        // exercise values that land in the high half of each split field
        for (major, minor) in [
            (0u64, 0u64),
            (8, 1),
            (259, 4),
            (4095, 255),
            (4096, 256),
            (0xffff, 0xfffff),
        ] {
            let st_dev = ((major & 0xfff) << 8)
                | ((major & !0xfff) << 32)
                | (minor & 0xff)
                | ((minor & !0xff) << 12);

            assert_eq!(
                kernel_dev(st_dev),
                mkdev(major, minor),
                "major {major} minor {minor}"
            );
        }
    }

    #[test]
    fn same_inode_on_different_filesystems_is_not_the_same_key() {
        let gpu = InodeKey::new(7, 259);
        let unrelated = InodeKey::new(66308, 259);

        assert_ne!(gpu, unrelated);
        assert_eq!(gpu.ino, unrelated.ino);
    }

    #[test]
    fn conversion_matches_the_running_kernel() {
        use std::{collections::BTreeMap, fs, os::unix::fs::MetadataExt};

        let Ok(mountinfo) = fs::read_to_string("/proc/self/mountinfo") else {
            return; // not available in every build sandbox
        };

        // stat() on an autofs mount point triggers the automount, so the device id
        // we read back is the mounted filesystem's rather than the one mountinfo
        // listed. Network filesystems can hang the stat outright, there is no
        // timeout to lean on
        const SKIPPED_TYPES: &[&str] = &[
            "autofs",
            "nfs",
            "nfs4",
            "cifs",
            "smb3",
            "fuse",
            "fuse.sshfs",
            "afs",
            "ceph",
        ];

        // Later entries shadow earlier ones when two filesystems share a path
        let mut mounts: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        for line in mountinfo.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let ([major, minor], Some(path)) = (
                match fields.get(2).and_then(|f| f.split_once(':')) {
                    Some((major, minor)) => match (major.parse(), minor.parse()) {
                        (Ok(major), Ok(minor)) => [major, minor],
                        _ => continue,
                    },
                    None => continue,
                },
                fields.get(4),
            ) else {
                continue;
            };

            // The optional fields end at a lone "-", the filesystem type follows it
            let fs_type = fields
                .iter()
                .position(|field| *field == "-")
                .and_then(|separator| fields.get(separator + 1));
            match fs_type {
                Some(fs_type) if SKIPPED_TYPES.contains(fs_type) => continue,
                Some(_) => {}
                // A line we cannot classify is not worth stat'ing blindly
                None => continue,
            }

            mounts.insert((*path).to_owned(), (major, minor));
        }

        let mut checked = 0;
        for (path, (major, minor)) in mounts {
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            assert_eq!(
                kernel_dev(meta.dev()),
                mkdev(major, minor),
                "device id mismatch for {path}"
            );
            checked += 1;
        }

        assert!(checked > 0, "no mount point could be stat'd");
    }
}
