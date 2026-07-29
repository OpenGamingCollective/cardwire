# BPF

## Introduction

Cardwire uses Linux eBPF along with Linux Security Modules (LSM) and Syscall tracepoints to intercept and block applications. By intercepting these operations directly in the kernel, Cardwire provides a fast and seamless blocking without needing to unload drivers or modify user applications/files.

## eBPF Hooks

Cardwire utilizes two main types of eBPF hooks:

### 1. LSM Hooks
LSM hooks are used to intercept and block permission checks or file openings on device files (like `/dev/dri/*`). This stops applications from accessing a GPU simply by checking file stats.

- `lsm/file_open`: Intercepts the actual opening of blocked device files.
- `lsm/inode_permission`: Prevents permissions checks on blocked devices.
- `lsm/inode_getattr`: Prevents `stat()` calls on blocked devices.

### 2. Syscall Tracepoints
Tracepoints are used to monitor process lifecycle and manipulate the directory listings applications see.

- `tracepoint/sched/sched_process_exec`: In Smart mode, this signals the Cardwire daemon that a new process is starting so it can be analyzed.
- `tracepoint/sched/sched_process_exit`: Signals when a process dies, cleaning up its entries in the allowed process maps.
- `tp/syscalls/sys_enter_getdents64` and `sys_exit_getdents64`: Intercepts directory listings. This is the core magic behind dynamically hiding device files from applications.

## eBPF Maps

The eBPF programs communicate with the Cardwire userspace daemon using several BPF maps:

- **`cw_mode`**: Stores the current Cardwire mode (0=Integrated, 1=Hybrid, 2=Manual, 3=Smart).
- **`cw_blocked_ino`**: A hash map containing the inodes of blocked DRM devices (`/dev/dri/cardX`, `/dev/dri/renderDX`). The value indicates the GPU ID (0 for iGPU, 1 for dGPU).
- **`cw_exp_blk_ino`**: Contains inodes of blocked NVIDIA-specific files when `experimental_nvidia_block` is enabled.
- **`cw_allowed_pid`**: Used in Smart mode. Contains the PIDs of applications that have been analyzed and allowed to use the dGPU. The stored value (`__u8`) is used to identify if PID is meant for iGPU(0) or dGPU(1)
- **`cw_allowed_comm`**: A whitelist of process names (like `udev` or `pacman`) that bypass blocking entirely.
- **`cw_daemon_pid`**: Cardwire's own PID so it doesn't block itself.
- **`cw_exec_events`**, **`cw_close_events`**, **`cw_report_events`**: Ring buffers used to send process and block events back to userspace.

## Directory Hiding (`getdents64`)

Across all blocking modes (Integrated, Manual, Smart), Cardwire uses the `getdents64` syscall hooks to manipulate the contents of directories (like `/dev/dri/`) on the fly. 

When an application calls `getdents64` to list available GPUs, the eBPF program `patch_dirent_if_found` loops through the directory entries in memory. If it spots an inode belonging to a blocked GPU, it overwrites the previous entry's length field, effectively "jumping over" the blocked device. To the application, the blocked GPU simply does not exist.
