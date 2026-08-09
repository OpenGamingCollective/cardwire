# BPF

## Introduction

Cardwire uses Linux eBPF along with Linux Security Modules (LSM) and Syscall tracepoints to intercept and block applications. The eBPF program is written in Rust with aya-ebpf (`#![no_std]`) and loaded by the `cardwire-ebpf-userspace` crate. By intercepting these operations directly in the kernel, Cardwire provides a fast and seamless blocking without needing to unload drivers or modify user applications/files.

## eBPF Hooks

Cardwire utilizes two main types of eBPF hooks:

### 1. LSM Hooks

LSM hooks are used to intercept and block permission checks or file openings on device files (like `/dev/dri/*`). This stops applications from accessing a GPU simply by checking file stats.

- `lsm/file_open`: Intercepts the actual opening of blocked device files.
- `lsm/inode_permission`: Prevents permissions checks on blocked devices.
- `lsm/inode_getattr`: Prevents `stat()` calls on blocked devices.

### 2. Syscall Tracepoints

Tracepoints are used to monitor process lifecycle and manipulate the directory listings applications see.

- `tracepoint/sched/sched_process_exec`: In Smart and Manual modes, this signals the Cardwire daemon that a new process is starting so it can be analyzed. It first cleans the pid maps for the process.
- `tracepoint/sched/sched_process_exit`: Cleans up the process entries in the allowed and forced pid maps directly in the kernel.
- `tp/syscalls/sys_enter_getdents64` and `sys_exit_getdents64`: Intercepts directory listings. This is the core magic behind dynamically hiding device files from applications. Under kernel lockdown this pair degrades to a weakened state (directory hiding is skipped).

## eBPF Maps

The eBPF programs communicate with the Cardwire userspace daemon using several BPF maps:

- **`CW_MODE`**: Stores the current Cardwire mode (0=Integrated, 1=Hybrid, 2=Manual, 3=Smart).
- **`CW_BLOCKED_INO`**: A hash map (16384 entries) containing the inodes of blocked device files (`/dev/dri/cardX`, `/dev/dri/renderDX`, PCI sysfs, hwmon). The value is an `InodeState` struct `{ gpu_id: u32, blocked: u8, _padding: [u8; 3] }`.
- **`CW_EXP_BLK_INO`**: Contains inodes of blocked NVIDIA-specific files when `experimental_nvidia_block` is enabled. The value is the GPU id.
- **`CW_ALLOWED_PID`**: Used in Smart mode. Contains the PIDs of applications that have been analyzed and allowed to use the dGPU. The stored value is always `0`.
- **`CW_FORCED_PID`**: Used in Smart and Manual modes. Maps a PID to the GPU id it is forced to use.
- **`CW_ALLOWED_COMM`**: A whitelist of process names (like `udev` or `pacman`) that bypass blocking entirely.
- **`CW_DAEMON_PID`**: Cardwire's own PID so it doesn't block itself.
- **`CW_SETTINGS`**: Settings flags, key `0` gates the experimental NVIDIA blocking.
- **`CW_DIRENT`**: TID-keyed getdents64 state, used to pair the enter/exit hooks.
- **`CW_EXEC_EVENTS`**, **`CW_REPORT_EVENTS`**: Ring buffers used to send exec and blocked-access events back to userspace.

The decision algorithm (in `is_inode_blocked`) runs on every hook: it looks the inode up in `CW_BLOCKED_INO` (and `CW_EXP_BLK_INO` when the NVIDIA setting is on), then applies the mode-specific logic. In Manual mode a PID found in `CW_FORCED_PID` (itself or its parent) is allowed only on the forced GPU. In Smart mode allowed PIDs can use any GPU, forced PIDs only their GPU, and inodes of GPU 0 (the iGPU) are always allowed. Every block reports an event into `CW_REPORT_EVENTS`.

## Directory Hiding (`getdents64`)

Across all blocking modes (Integrated, Manual, Smart), Cardwire uses the `getdents64` syscall hooks to manipulate the contents of directories (like `/dev/dri/`) on the fly. 

When an application calls `getdents64` to list available GPUs, the eBPF program `patch_dirent_if_found` loops through the directory entries in memory. If it spots an inode belonging to a blocked GPU, it overwrites the previous entry's length field, effectively "jumping over" the blocked device. To the application, the blocked GPU simply does not exist and is omitted from directory listings rather than causing an error.
