# Switcheroo Control

Cardwire implements a compatibility shim for the `net.hadess.SwitcherooControl` D-Bus interface. This allows desktop environments to natively offer "Launch using Discrete Graphics Card" options in their application menus without needing any Cardwire-specific plugins.

## Service

- **Interface:** `net.hadess.SwitcherooControl`
- **Object path:** `/net/hadess/SwitcherooControl`

The shim is served on a second D-Bus connection with `replace_existing_names(true)`, so it takes the name over from an installed upstream switcheroo-control service. Failure to serve it is non-fatal, the daemon logs a warning and keeps running.

---

## Properties

### `HasDualGpu`

Indicates whether the system has exactly two GPUs.

- **Type:** `b` (boolean)
- **Access:** Read

Blocked GPUs are excluded from the count, except in Smart mode where the blocked dGPU is still advertised (see [`GPUs`](#gpus)).

### `NumGPUs`

The number of GPUs detected on the system.

- **Type:** `u` (uint32)
- **Access:** Read

Same rules as `HasDualGpu`: blocked GPUs are excluded, except in Smart mode.

### `GPUs`

A list of all available GPUs and their configurations.

- **Type:** `aa{sv}` (Array of dictionaries mapping strings to variants)
- **Access:** Read
- **Dictionary Keys:**
  - `Name`: `s` - The name of the GPU.
  - `Environment`: `as` - An array of environment variable key-value pairs to set when launching an application on this GPU (e.g., `["CARDWIRE_FORCE_DGPU", "1"]`).
  - `Default`: `b` - Whether this is the default display GPU (usually the iGPU).
  - `Discrete`: `b` - Whether this is a discrete GPU.

Blocked GPUs are excluded from the list, except in Smart mode: the blocked dGPU is still advertised so desktop environments keep offering the "Launch using Discrete Graphics Card" option, with its normal full `Environment`.

The shim is served on a second D-Bus connection. It manually emits `org.freedesktop.DBus.Properties.PropertiesChanged` whenever the GPU list changes (for example after a hotplug refresh or a mode change).

---

## Environment Variables Explained

The `Environment` property provides the exact environment variables the desktop environment should inject into the application when the user selects a specific GPU.

### `CARDWIRE_FORCE_DGPU=1`

This is provided when the user selects a **Discrete GPU** on a 2-GPU system where the discrete GPU is not the default one. On systems with 3 or more GPUs, the routing variable is `CARDWIRE_FORCE_GPU=<gpu_id>` instead.

When Cardwire detects this environment variable during the application's launch in Smart Mode, it does two things:

1. **Unblocks the dGPU**: The eBPF hooks allow the application to access the discrete GPU's device files.
2. **Hides the iGPU**: It actively intercepts and blocks the application from seeing the integrated GPU.

Hiding the iGPU ensures that the application is forced to use the discrete GPU, preventing issues where an application might get confused by seeing two GPUs and accidentally select the weaker one.

### `CARDWIRE_ALLOW`

This is provided when the user selects the **Default GPU**. It is set to `1` when the default GPU is discrete (desktop), and to `0` when the default GPU is the iGPU (laptop).

The analyzer only allows the dGPU when the value is `1`. Any other value falls through to the regular policy checks, it is not an explicit keep-blocked directive.

### Vendor environment variables

Depending on the GPU vendor, the environment also carries the offload variables used by the graphics stacks:

- **NVIDIA**: `__NV_PRIME_RENDER_OFFLOAD=1`, `__GLX_VENDOR_LIBRARY_NAME=nvidia`, `__VK_LAYER_NV_optimus=NVIDIA_only`, `VK_LOADER_DRIVERS_SELECT=*nvidia*,*nouveau*`
- **AMD**: `DRI_PRIME=pci-0000_xx_xx_x`, `VK_LOADER_DRIVERS_SELECT=*radeon*`
- **Intel**: `DRI_PRIME`, `VK_LOADER_DRIVERS_SELECT=*intel*`

> [!CAUTION]
> These ENV were inherited from switcheroo-control old API, if they are obsolete/non-necessary they may get dropped in a future cardwire release
