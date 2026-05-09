# DBUS

## Service

- **Bus Name:** `com.github.opengamingcollective.cardwire`
- **Object Path:** `/com/github/opengamingcollective/cardwire`
- **Interface:** `com.github.opengamingcollective.cardwire`

## Methods

### SetGpuBlock

Set the block state for a specific GPU. Only available when `Mode` is set to `Manual`

The default GPU cannot be blocked

**Inputs:**

- gpu_id (in): `u` -- The GPU identifier (`id` field)
- block (in): `b` -- `true` to block, `false` to unblock

**Outputs:** None

### ListDevices

List all detected GPU devices

**Inputs:** None

**Outputs:**

- (out): `a{t(ussuubbbs)}`

**GPU Struct `(ussuubbbs)` fields:**

- `id`: `u` -- GPU identifier
- `name`: `s` -- GPU name
- `pci`: `s` -- PCI address (e.g. `0000:01:00.0`)
- `render`: `u` -- DRM render node minor number
- `card`: `u` -- DRM card node minor number
- `default`: `b` -- Whether this is the default display GPU
- `blocked`: `b` -- Whether the GPU is currently blocked by the daemon
- `nvidia`: `b` -- Whether the GPU is an NVIDIA device
- `nvidia_minor`: `s` -- NVIDIA driver minor number (empty string if not applicable)

### ListDevicesPci

List all detected PCI devices

**Inputs:** None

**Outputs:**

- (out): `a{s(ssssssss)}`

**PCI Struct `(ssssssss)` fields:**

- `pci_address`: `s` -- PCI address (e.g. `0000:01:00.0`)
- `iommu_group`: `s` -- IOMMU group number (empty string if none)
- `vendor_id`: `s` -- PCI vendor ID (empty string if unknown)
- `device_id`: `s` -- PCI device ID (empty string if unknown)
- `vendor_name`: `s` -- Vendor name (empty string if unknown)
- `device_name`: `s` -- Device name (empty string if unknown)
- `driver`: `s` -- Kernel driver in use (empty string if unknown)
- `class`: `s` -- PCI class (empty string if unknown)
- `parent_pci` : `s` -- Parent PCI (empty string if unknown)
- `child_pci` : `s` -- Parent PCI (empty string if unknown)

## Properties

### Mode

Controls the global GPU blocking mode

- **Type:** `u` (uint32)
- **Access:** Read/Write
- **Emits:** `PropertiesChanged` on change

**Values:**

- `0` -- Integrated: Block the dGPU. Requires exactly 2 GPUs
- `1` -- Hybrid: Unblock the dGPU. Requires exactly 2 GPUs
- `2` -- Manual: Allow per-GPU blocking via `SetGpuBlock`, applies saved GPU state if `auto_apply` is enabled
