use aya_ebpf::{
    btf_maps::RingBuf, macros::{btf_map, map}, maps::{Array, HashMap}
};

/*
    A single entry array used to store cardwired pid
*/
#[map]
pub static CW_DAEMON_PID: Array<u32> = Array::<u32>::with_max_entries(1, 0);

/*
    A single entry array used to store the current cardwired mode
    List of possible values:
    0 - Integrated
    1 - Hybrid
    2 - Manual
    3 - Smart
*/
#[map]
pub static CW_MODE: Array<u8> = Array::<u8>::with_max_entries(1, 0);

/*
    Hashmap containing cardwired exp_nvidia setting
    0 - Exp_nvidia_setting
*/
#[map]
pub static CW_SETTINGS: HashMap<u8, bool> = HashMap::<u8, bool>::with_max_entries(255, 0);

/*
   Map used to store blocked inodes sent from userspace
   Key = Inode
   Value = associated GPU
*/
#[map]
pub static CW_BLOCKED_INO: HashMap<u64, u32> = HashMap::<u64, u32>::with_max_entries(4096, 0);

/*
   Map used to store blocked inodes from exp_nvidia
   Key = Inode
   Value = 0, not used because exp files can be shared by multiple devices (nvidiactl)
*/
#[map]
pub static CW_EXP_BLK_INO: HashMap<u64, u32> = HashMap::<u64, u32>::with_max_entries(4096, 0);

/*
    Map used to store a list of allowed pid
    Key = PID
    Value = always 0
*/
#[map]
pub static CW_ALLOWED_PID: HashMap<u32, u32> = HashMap::<u32, u32>::with_max_entries(16384, 0);

/*
    Map used to store a list of forced pid (CARDWIRE_FORCE_GPU)
    The value is used to identify a GPU, the process will only be able to see blocked_ino that matchs
    If it doesnt match, the process wont be able to see the said ino
    Key = PID
    Value = GPU id
*/
#[map]
pub static CW_FORCED_PID: HashMap<u32, u32> = HashMap::<u32, u32>::with_max_entries(16384, 0);

/*
    Map used to store a list of whitelist comm, some comm needs to have access to the GPUs, preventing that access can cause crash or instability
    Eg. udev on pci rescan, pacman on nvidia driver update
*/
#[map]
pub static CW_ALLOWED_COMM: HashMap<[u8; 16], u8> =
    HashMap::<[u8; 16], u8>::with_max_entries(1024, 0);

#[map]
pub static CW_DIRENT: HashMap<u32, u64> = HashMap::<u32, u64>::with_max_entries(1024, 0);

#[repr(align(8))]
pub struct ExecEvent {
    pid: u64,
}

#[btf_map]
pub static CW_EXEC_EVENTS: RingBuf<ExecEvent, 4096> = RingBuf::new();

#[repr(align(8))]
pub struct CloseEvent {
    pid: u64,
}

#[btf_map]
pub static CW_CLOSE_EVENTS: RingBuf<CloseEvent, 4096> = RingBuf::new();

#[repr(align(8))]
pub struct ReportEvent {
    pid: u64,
}

#[btf_map]
pub static CW_REPORT_EVENTS: RingBuf<ReportEvent, 4096> = RingBuf::new();
