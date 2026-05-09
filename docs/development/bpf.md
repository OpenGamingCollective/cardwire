# BPF

## Introduction

Cardwire use the Kernel eBPF + LSM features to block syscall to the dGPU

## List of used LSM

- lsm/file_open
- lsm/inode_permission
- lsm/inode_getattr

## List of used MAPS

**BLOCKED_RENDERID**

- Used for the renderD minor

**BLOCKED_CARDID**

- For the card minor

**BLOCKED_PCI**

- For the PCI address

**BLOCKED_PCI_FILES**

- For the list of blocked PCI files

**BLOCKED_NVIDIA_FILES**

- For the list of blocked NVIDIA files

**SETTINGS**

- For experimental_nvidia_block

## Block list 

### PCI files

Files that get blocked when a gpu's PCI address is blocked:

- config
- current_link_speed
- current_link_width
- max_link_speed
- max_link_width

### NVIDIA files

These files are only blocked when the `experimental_nvidia_block` setting is enabled

- libGLX_nvidia.so.0
- nvidia_icd.json
- nvidia_icd.x86_64.json
- nvidiactl

/dev/nvidia? using the minor

Example:

```bash
/dev/nvidia0
```

Will be blocked using the major `195` and the minor `0`

### DRM

DRM node (card + renderD) are blocked using their major + minor ID

Example:

```bash
/dev/dri/card1
/dev/dri/renderD128
```

Will be blocked using the major `226` and the minor `1` || `128`
