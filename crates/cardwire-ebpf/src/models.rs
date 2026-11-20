#[repr(C, align(8))]
#[derive(Copy, Clone)]
pub struct InodeState {
    pub gpu_id: u32,
    pub blocked: u8,
    pub _padding: [u8; 3], // 8-byte alignment
}

#[repr(C, align(8))]
#[derive(Copy, Clone)]
pub struct InodeKey {
    pub name: [u8; 64], // dirent name
    pub ino: u64,       // inode number
}

pub struct ReturnCode {}
impl ReturnCode {
    // Succes means we didn't block the process
    pub const SUCCESS: Result<i32, i32> = Ok(0);
    // ENOENT means we blocked the process, it can't see the file
    pub const ENOENT: Result<i32, i32> = Ok(-2);
}

pub struct ScanCode {}
impl ScanCode {
    /// The loop ran to completion
    pub const OK: u32 = 0;
    /// A dirent header could not be read
    pub const READ_FAILED: u32 = 1;
    /// A hidden entry could not be merged into the previous one
    pub const WRITE_FAILED: u32 = 2;
}
