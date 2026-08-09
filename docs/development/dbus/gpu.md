# Gpu

`org.opengamingcollective.cardwire.Gpu`

Object path `/org/opengamingcollective/cardwire/Gpu/{id}`

Represents a single GPU device, where `{id}` is the numeric identifier of the GPU (0 is always the default one). These objects can be dynamically discovered by calling `GetManagedObjects` on the standard `org.freedesktop.DBus.ObjectManager` interface located at the root path (`/org/opengamingcollective/cardwire`). New objects appear on `InterfacesAdded` and disappear on `InterfacesRemoved` when GPUs are hotplugged.

**Properties:**

- **`Block`**
  Set or get the block state for this specific GPU. Only writable when `Mode` is set to `Manual`. The default gpu cannot be blocked.
  - **Type:** `b`
  - **Access:** Read/Write

- **`Env`**
  The GPU launch environment, as flat key/value pairs. This is the same environment the [Switcheroo shim](switcheroo.md) exposes, and it is what `cardwire launch` applies to child processes.
  - **Type:** `as`
  - **Access:** Read-only

- **`Launchable`**
  Whether this GPU can be targeted by an offload launch in the current mode: `true` when the GPU is available and not blocked, or blocked in `Smart` mode (where the smart policy can grant per-process access). On desktops and multi-GPU systems (`Manual`/`Hybrid` modes) blocked GPUs are never launchable. The daemon stays the single source of truth.
  - **Type:** `b`
  - **Access:** Read-only

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
