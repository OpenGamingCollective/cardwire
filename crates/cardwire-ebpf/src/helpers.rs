use aya_ebpf::helpers::bpf_get_current_pid_tgid;

use crate::{
    CardwiredSetting, DAEMON_INDEX, HYBRID, INTEGRATED, MANUAL, MODE_INDEX, SMART, maps::{CW_BLOCKED_INO, CW_DAEMON_PID, CW_EXP_BLK_INO, CW_MODE, CW_SETTINGS}
};

/// Verify if the inode is inside CW_BLOCKED_INO or not
#[inline(always)]
pub unsafe fn is_inode_blocked(inode: u64) -> bool {
    let mut blocked: bool = false;

    'inode_check: {
        // Check if the inode is in the blocked list
        if unsafe { CW_BLOCKED_INO.get(inode).is_some() } {
            blocked = true;
            break 'inode_check;
        }
        // We didn't match any inode, try with nvidia inodes
        if unsafe { is_nvidia_setting_enabled() && CW_EXP_BLK_INO.get(inode).is_some() } {
            blocked = true;
            break 'inode_check;
        }
    }

    'end: {
        if !blocked {
            // exit and return success
            break 'end;
        }

        // Get the current mode used
        let mode = match CW_MODE.get(MODE_INDEX) {
            Some(mode) => mode,
            // If we can't get the mode, just exit the block and return success
            None => break 'end,
        };
        if *mode == INTEGRATED || *mode == MANUAL {
            // if integrated/manual, just block
            return true;
        }
        if *mode == SMART {
            // TODO: in a future PR
        }
    }

    false
}

/// Verify if the proc is cardwired, returns None if the map fails
#[inline(always)]
pub fn is_cardwired() -> Option<bool> {
    let proc_pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    match CW_DAEMON_PID.get(DAEMON_INDEX) {
        Some(pid) => Some(proc_pid == *pid),
        None => None,
    }
}

/// Verify if the current device mode is hybrid, returns None if the map fails
#[inline(always)]
pub unsafe fn is_hybrid() -> Option<bool> {
    match CW_MODE.get(MODE_INDEX) {
        Some(mode) => Some(*mode == HYBRID),
        None => None,
    }
}

/// Verify if the current device mode is smart, returns None if the map fails
#[inline(always)]
pub unsafe fn is_smart() -> Option<bool> {
    match CW_MODE.get(MODE_INDEX) {
        Some(mode) => Some(*mode == SMART),
        None => None,
    }
}

#[inline(always)]
pub unsafe fn is_nvidia_setting_enabled() -> bool {
    match unsafe { CW_SETTINGS.get(CardwiredSetting::EXP_NVIDIA) } {
        Some(setting) => *setting,
        None => false,
    }
}
