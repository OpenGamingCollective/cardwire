# DBUS

## Service

- **Bus Name:** `org.opengamingcollective.cardwire`

> [!NOTE]
> Cardwire also implements the SwitcherooControl interface for desktop environment integration. See [switcheroo.md](switcheroo.md) for details.

---

## Object Path

`/org/opengamingcollective/cardwire`

### Manager

`org.opengamingcollective.cardwire.Manager`

**Methods:**

- **`Status`**
  Simple dbus method to check if the daemon is alive
  - **Inputs:** None
  - **Outputs:** None

> [!NOTE]
> GPU list refresh is available on the [Debug](#debug) interface as `RefreshGpu`.

### Mode

`org.opengamingcollective.cardwire.Mode`

**Methods:**

- **`AvailableModes`**
  List the modes the current system supports
  - **Inputs:** None
  - **Outputs:**
    - (out): `au` -- Laptop systems: `[0, 1, 3]`, Desktop/Manual systems: `[1, 2]`

**Properties:**

- **`Mode`**
  Controls the Cardwire's Mode
  - **Type:** `u`
  - **Access:** Read/Write
  - **Emits:** `PropertiesChanged` on change
  - **Values:**
    - `0` Integrated: Block the dGPU. Laptop only, requires exactly 2 GPUs
    - `1` Hybrid: Unblock all GPUs. Available on any system, this is the default
    - `2` Manual: Allow per-GPU blocking via individual GPU objects. Applies saved GPU state on mode change if `auto_apply_gpu_state` is enabled
    - `3` Smart: Block the dGPU by default but dynamically allow access per-application using eBPF. Laptop only, requires exactly 2 GPUs

### Config

`org.opengamingcollective.cardwire.Config`

**Properties:**

- **`AutoApplyGpuState`**
  Automatically applies the saved block/unblock states to GPUs
  - **Type:** `b`
  - **Access:** Read/Write

- **`BatteryAutoSwitch`**
  Controls whether the daemon automatically switches modes when switching to battery power
  - **Type:** `b`
  - **Access:** Read/Write

- **`BatteryAutoSwitchMode`**
  Controls which mode the daemon automatically switches
  - **Type:** `u`
  - **Access:** Read/Write

- **`ExperimentalNvidiaBlock`**
  Toggles the experimental blocking for NVIDIA GPU, only works if the system has exactly 1 Nvidia GPU
  - **Type:** `b`
  - **Access:** Read/Write

- **`ExternalDisplayAutoSwitch`**
  Temporarily switches Integrated and Smart modes to Hybrid when an external display is connected
  to a dGPU-owned DRM connector. Hybrid and Manual modes are unchanged. The requested mode is
  restored after disconnect.
  - **Type:** `b`
  - **Access:** Read/Write

### Debug

`org.opengamingcollective.cardwire.Debug`

**Methods:**

- **`GetPciDevices`**
  Get a dictionary of all detected PCI devices.
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{s(sssssssss)}` -- A dictionary mapping PCI addresses to a struct containing:
      - `iommu_group`: `s` - IOMMU group number (empty string if none)
      - `vendor_id`: `s` - PCI vendor ID (empty string if unknown)
      - `device_id`: `s` - PCI device ID (empty string if unknown)
      - `vendor_name`: `s` - Vendor name (empty string if unknown)
      - `device_name`: `s` - Device name (empty string if unknown)
      - `driver`: `s` - Kernel driver in use (empty string if unknown)
      - `class`: `s` - PCI class (empty string if unknown)
      - `parent_pci`: `s` - Parent PCI address (empty string if unknown)
      - `child_pci`: `s` - Child PCI address (empty string if unknown)

### Gpu

`/org/opengamingcollective/cardwire/Gpu/{id}`

Represents a single GPU device, where `{id}` is the numeric identifier of the GPU (0 is always the default one). These objects can be dynamically discovered by calling `GetManagedObjects` on the standard `org.freedesktop.DBus.ObjectManager` interface located at the root path (`/org/opengamingcollective/cardwire`)

**Properties:**

- **`Block`**
  Set or get the block state for this specific GPU. Only writable when `Mode` is set to `Manual`. The default gpu cannot be blocked.
  - **Type:** `b`
  - **Access:** Read/Write

- **`Env`**
  The GPU launch environment, as flat key/value pairs (e.g., `["CARDWIRE_FORCE_DGPU", "1", "__NV_PRIME_RENDER_OFFLOAD", "1"]`). This is the same environment the Switcheroo shim exposes, and it is what `cardwire launch` applies to child processes.
  - **Type:** `as`
  - **Access:** Read

- **`Launchable`**
  Whether this GPU can be targeted by an offload launch in the current mode: `true` when the GPU is available and not blocked, or blocked in `Smart` mode (where the smart policy can grant per-process access). On desktops and multi-GPU systems (`Manual`/`Hybrid` modes) blocked GPUs are never launchable. The daemon stays the single source of truth
  - **Type:** `b`
  - **Access:** Read

**Methods:**

- **`GetDevice`**
  Get the detailed informations of this GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `(ssuubbbbssbs)` -- A struct containing:
      - `name`: `s` - GPU name
      - `pci`: `s` - PCI address
      - `render`: `u` - DRM render node minor number
      - `card`: `u` - DRM card node minor number
      - `default`: `b` - Whether this is the default display GPU
      - `discrete`: `b` - Whether the GPU is a discrete GPU
      - `virtual_gpu`: `b` - Whether the GPU is a virtual device (virtio/qemu)
      - `available`: `b` - Whether the GPU is usable by cardwire (eg non-available if GPU bound to vfio)
      - `vendor`: `s` - GPU vendor name
      - `driver`: `s` - Kernel driver in use ("none" if not applicable)
      - `nvidia`: `b` - Whether the GPU is an NVIDIA device
      - `nvidia_minor`: `s` - NVIDIA driver minor number ("none" if not applicable)

- **`PowerState`**
  Get the current power state of the GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `s` -- The raw power state file content (e.g., "D0", "D3cold")

- **`Lsof`**
  Read file descriptors to find which applications have currently opened the GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{sas}` -- A dictionary mapping file paths (like `/dev/dri/card0`) to an array of process names

**Signals:**

- **`PowerStateChanged`**
  Emitted when the power state of the GPU changes
  - **Parameters:** `s` (string) -- The new power state as a parsed enum value (e.g., "D0", "D3Cold", "D3Hot", "Unknown")

### Logger

`org.opengamingcollective.cardwire.Logger`

**Methods:**

- **`ProcessBlocked`**
  Get the recent blocked GPU access attempts
  - **Inputs:** None
  - **Outputs:**
    - (out): `a(tusus)` -- An array of `LogEntry` structs, where timestamp is in seconds:
      - `timestamp`: `t` - Unix timestamp in seconds
      - `pid`: `u` - Process id
      - `comm`: `s` - Process name
      - `gpu_id`: `u` - The blocked GPU id
      - `wayland_app_id`: `s` - The Wayland app id (empty string if unknown)

**Signals:**

- **`ProcessBlockedChanged`**
  Emitted when a new blocked GPU access attempt is logged
  - **Parameters:** `(tusus)` -- A single `LogEntry` struct

### SmartPolicy

`org.opengamingcollective.cardwire.SmartPolicy`

**Methods:**

- **`RequestProcessAccess`**
  Request a policy override for a running process
  - **Inputs:**
    - `pid`: `u` - The process id
    - `policy`: `s` - One of `"Default"`, `"Allow_dGPU"`, `"Force_dGPU"`, `"Force_GPU"`
    - `value`: `u` - GPU id (used by `Force_GPU`)
  - **Outputs:** None

- **`GetProcessStatus`**
  Get the current policy verdict for a process
  - **Inputs:**
    - `pid`: `u` - The process id
  - **Outputs:**
    - (out): `(sau)` -- `("Allowed" | "Forced" | "")` and the optional forced GPU id. An empty string means the process is unclassified

- **`GetAppPolicies`**
  Get all known applications and their policies
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{s(sasasu)}` -- A dictionary mapping app ids to `DbusAppMetadata` structs

- **`SetAppPolicy`**
  Set the policy for a known application
  - **Inputs:**
    - `app_id`: `s` - The application id
    - `policy`: `i` - `0` Blocked, `1` Allowed
  - **Outputs:** None

**Signals:**

- **`NewAppAdded`**
  Emitted when a new application is discovered
  - **Parameters:** `(s(sasasu))` -- The app id and its `DbusAppMetadata`
