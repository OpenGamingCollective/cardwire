# DBUS

## Service

- **Bus Name:** `com.github.opengamingcollective.cardwire`

---

## Object Path
`/com/github/opengamingcollective/cardwire`

### Manager
`com.github.opengamingcollective.cardwire.Manager`

**Methods:**

- **`RefreshGpu`**
  Refresh the internal GPU list from the system (Not implemented yet)
  - **Inputs:** None
  - **Outputs:** None

- **`Status`**
  Simple dbus method to check if the daemon is alive
  - **Inputs:** None
  - **Outputs:** None

### Mode
`com.github.opengamingcollective.cardwire.Mode`

**Properties:**

- **`Mode`**
  Controls the Cardwire's Mode
  - **Type:** `u`
  - **Access:** Read/Write
  - **Emits:** `PropertiesChanged` on change
  - **Values:**
    - `0` Integrated: Block the dGPU. Requires exactly 2 GPUs
    - `1` Hybrid: Unblock the dGPU. Requires exactly 2 GPUs
    - `2` Manual: Allow per-GPU blocking via individual GPU objects. Applies saved GPU state on mode change if `auto_apply_gpu_state` is enabled

### Config
`com.github.opengamingcollective.cardwire.Config`

**Properties:**

- **`AutoApplyGpuState`**
  Automatically applies the saved block/unblock states to GPUs
  - **Type:** `b`
  - **Access:** Read/Write

- **`BatteryAutoSwitch`**
  Controls whether the daemon automatically switches modes when switching to battery power
  - **Type:** `b`
  - **Access:** Read/Write

- **`ExperimentalNvidiaBlock`**
  Toggles the experimental blocking for NVIDIA GPU, only works if the system has exactly 1 Nvidia GPU
  - **Type:** `b`
  - **Access:** Read/Write

**Methods:**

- **`SaveToFile`**
  Save the current daemon configuration (properties above) to the `cardwire.toml` config file
  - **Inputs:** None
  - **Outputs:** None

### Debug
`com.github.opengamingcollective.cardwire.Debug`

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
`/com/github/opengamingcollective/cardwire/Gpu/{id}`

Represents a single GPU device, where `{id}` is the numeric identifier of the GPU (0 is always the default one). These objects can be dynamically discovered by calling `GetManagedObjects` on the standard `org.freedesktop.DBus.ObjectManager` interface located at the root path (`/com/github/opengamingcollective/cardwire`)

**Properties:**

- **`Block`**
  Set or get the block state for this specific GPU. Only writable when `Mode` is set to `Manual`. The default gpu cannot be blocked.
  - **Type:** `b`
  - **Access:** Read/Write

**Methods:**

- **`GetDevice`**
  Get the detailed informations of this GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `(ssuubbs)` -- A struct containing:
      - `name`: `s` - GPU name
      - `pci`: `s` - PCI address
      - `render`: `u` - DRM render node minor number
      - `card`: `u` - DRM card node minor number
      - `default`: `b` - Whether this is the default display GPU
      - `nvidia`: `b` - Whether the GPU is an NVIDIA device
      - `nvidia_minor`: `s` - NVIDIA driver minor number (empty string if not applicable)

- **`PowerState`**
  Get the current power state of the GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `s` -- The power state (e.g., "D0", "D3cold")

- **`Lsof`**
  Read file descriptors to find which applications have currently opened the GPU
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{sas}` -- A dictionary mapping file paths (like `/dev/dri/card0`) to an array of process names

**Signals:**

- **`PowerStateChanged`**
  Emitted when the power state of the GPU changes
  - **Parameters:** `s` (string) -- The new power state
