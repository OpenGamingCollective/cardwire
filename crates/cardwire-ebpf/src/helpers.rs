use aya_ebpf::helpers::{
    bpf_get_current_comm, bpf_get_current_pid_tgid, bpf_probe_read_kernel, generated::bpf_get_current_task
};

use crate::{
    CardwiredSetting, DAEMON_INDEX, HYBRID, INTEGRATED, MANUAL, MODE_INDEX, SMART, maps::{
        CW_ALLOWED_COMM, CW_ALLOWED_PID, CW_BLOCKED_INO, CW_DAEMON_PID, CW_EXP_BLK_INO, CW_FORCED_PID, CW_MODE, CW_REPORT_EVENTS, CW_SETTINGS, ReportEvent
    }
};

use crate::vmlinux::task_struct;

/// Verify if the inode is inside CW_BLOCKED_INO or not
#[inline(always)]
pub unsafe fn is_inode_blocked(inode: u64) -> bool {
    let mut blocked: bool = false;
    let mut ino_gpu_id: u32 = 0;

    'inode_check: {
        // Check if the inode is in the blocked list
        if let Some(v) = unsafe { CW_BLOCKED_INO.get(inode) } {
            blocked = true;
            ino_gpu_id = *v;
            break 'inode_check;
        }
        // We didn't match any inode, try with nvidia inodes
        if unsafe { is_nvidia_setting_enabled() }
            && let Some(v) = unsafe { CW_EXP_BLK_INO.get(inode) }
        {
            blocked = true;
            ino_gpu_id = *v;
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

        // If everything ok, read the pid
        let pid: u32 = (bpf_get_current_pid_tgid() >> 32) as u32;

        let comm = bpf_get_current_comm().unwrap_or([0u8; 16]);

        if *mode == INTEGRATED {
            // if integrated, just report the event and block
            report_event(pid, ino_gpu_id, comm);
            return true;
        }

        if *mode == MANUAL {
            let ppid = get_task_ppid().unwrap_or(u32::MAX);

            // Check if the PID or PPID is in the forced map
            let forced_gpu_id =
                unsafe { CW_FORCED_PID.get(pid).or_else(|| CW_FORCED_PID.get(ppid)) };

            if let Some(pid_gpu_id) = forced_gpu_id {
                // If forced GPU ID matches the inode's GPU ID, allow access
                match *pid_gpu_id == ino_gpu_id {
                    true => break 'end,
                    false => {
                        report_event(pid, ino_gpu_id, comm);
                        return true;
                    }
                }
            }

            // Default manual mode behavior: block access to blocked inodes
            report_event(pid, ino_gpu_id, comm);
            return true;
        }

        // 0 = iGPU
        // 1 = dGPU
        if *mode == SMART {
            let ppid = get_task_ppid().unwrap_or(u32::MAX);

            // We need to check if the map contains the pid
            // In smart mode, we do not check if the ino_gpu_id matches, it was only made for dual
            // gpu(hybrid) laptops

            // First we try with the pid
            if unsafe { CW_ALLOWED_PID.get(pid).is_some() }
                || unsafe { CW_ALLOWED_PID.get(ppid).is_some() }
            {
                // We got a match, pid is allowed !
                break 'end;
            }

            // If we are here, the pid AND the ppid are not in the allowed map, check the FORCED map
            let forced_gpu_id =
                unsafe { CW_FORCED_PID.get(pid).or_else(|| CW_FORCED_PID.get(ppid)) };

            if let Some(pid_gpu_id) = forced_gpu_id {
                // We match the ino_gpu_id with the pid_gpu_id
                // If they match, that means the ino is owned by the said GPU id, and we want to
                // force the process to use said GPU id
                match *pid_gpu_id == ino_gpu_id {
                    // The process should be allowed to see the inode
                    true => break 'end,
                    // Process should only be allowed to see the said GPU id
                    false => {
                        // Report the event to the daemon
                        report_event(pid, ino_gpu_id, comm);
                        return true;
                    }
                }
            }

            // Check if inode gpu id matches 0, the iGPU.
            // iGPU should always be 0
            if ino_gpu_id == 0 {
                // allow the iGPU
                break 'end;
            }

            // Report the event to the daemon
            report_event(pid, ino_gpu_id, comm);

            // End of smart mode check, block if it didnt get allowed earlier
            return true;
        }
    }

    false
}

#[inline(always)]
fn report_event(pid: u32, gpu_id: u32, comm: [u8; 16]) {
    if let Some(mut ring_buf) = CW_REPORT_EVENTS.reserve(0) {
        let event: ReportEvent = ReportEvent { pid, gpu_id, comm };
        // write to the map
        ring_buf.write(event);
        // submit
        ring_buf.submit(0);
    };
}

#[inline(always)]
fn get_task_ppid() -> Option<u32> {
    let task: *const task_struct = unsafe { bpf_get_current_task() as *const task_struct };
    if task.is_null() {
        return None;
    }

    let real_parent =
        match unsafe { bpf_probe_read_kernel(core::ptr::addr_of!((*task).real_parent)) } {
            Ok(parent) => parent,
            Err(_) => {
                return None;
            }
        };

    if real_parent.is_null() {
        return None;
    }

    match unsafe { bpf_probe_read_kernel(core::ptr::addr_of!((*real_parent).tgid)) } {
        Ok(ppid) => Some(ppid as u32),
        Err(_) => None,
    }
}

/// Verify if the proc is whitelisted, returns false if not
#[inline(always)]
pub fn is_comm_whitelisted() -> bool {
    if let Ok(comm) = bpf_get_current_comm()
        && unsafe { CW_ALLOWED_COMM.get(comm).is_some() }
    {
        return true;
    }
    false
}

/// Verify if the proc is cardwired, returns None if the map fails
#[inline(always)]
pub fn is_cardwired() -> Option<bool> {
    let proc_pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    CW_DAEMON_PID.get(DAEMON_INDEX).map(|pid| proc_pid == *pid)
}

/// Verify if the current device mode is hybrid, returns None if the map fails
#[inline(always)]
pub unsafe fn is_hybrid() -> Option<bool> {
    CW_MODE.get(MODE_INDEX).map(|mode| *mode == HYBRID)
}

/// Verify if the current device mode is smart, returns None if the map fails
#[inline(always)]
pub unsafe fn is_smart() -> Option<bool> {
    CW_MODE.get(MODE_INDEX).map(|mode| *mode == SMART)
}

/// Verify if the current device mode is manual, returns None if the map fails
#[inline(always)]
pub unsafe fn is_manual() -> Option<bool> {
    CW_MODE.get(MODE_INDEX).map(|mode| *mode == MANUAL)
}

#[inline(always)]
pub unsafe fn is_nvidia_setting_enabled() -> bool {
    match unsafe { CW_SETTINGS.get(CardwiredSetting::EXP_NVIDIA) } {
        Some(setting) => *setting,
        None => false,
    }
}
