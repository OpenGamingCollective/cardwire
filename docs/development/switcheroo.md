# Switcheroo Shim

Cardwire implements a compatibility shim for the `net.hadess.SwitcherooControl` D-Bus interface. This allows desktop environments (like GNOME(gio-launch-desktop) and KDE) to natively offer "Launch using Discrete Graphics Card" options in their application menus without needing any Cardwire-specific plugins.

_(Having our own integration would've been better tbh)_

## Service

- **Interface:** `net.hadess.SwitcherooControl`

---

## Properties

### `HasDualGpu`
Indicates whether the system has exactly two GPUs.
- **Type:** `b` (boolean)
- **Access:** Read

### `NumGPUs`
The number of GPUs detected on the system.
- **Type:** `u` (uint32)
- **Access:** Read

### `GPUs`
A list of all available GPUs and their configurations.
- **Type:** `aa{sv}` (Array of dictionaries mapping strings to variants)
- **Access:** Read
- **Dictionary Keys:**
  - `Name`: `s` - The name of the GPU.
  - `Environment`: `as` - An array of environment variable key-value pairs to set when launching an application on this GPU (e.g., `["CARDWIRE_FORCE_DGPU", "1"]`).
  - `Default`: `b` - Whether this is the default display GPU (usually the iGPU).
  - `Discrete`: `b` - Whether this is a discrete GPU.

---

## Environment Variables Explained

The `Environment` property provides the exact environment variables the desktop environment should inject into the application when the user selects a specific GPU.

### `CARDWIRE_FORCE_DGPU=1`
This is provided when the user selects the **Discrete GPU**. 

When Cardwire detects this environment variable during the application's launch in Smart Mode, it does two things:
1. **Unblocks the dGPU**: The eBPF hooks allow the application to access the discrete GPU's device files.
2. **Hides the iGPU**: It actively intercepts and blocks the application from seeing the integrated GPU. 

Hiding the iGPU ensures that the application is forced to use the discrete GPU, preventing issues where an application might get confused by seeing two GPUs and accidentally select the weaker one.

### `CARDWIRE_ALLOW=0`
This is provided when the user selects the **Default/Integrated GPU**. 

It explicitly tells Cardwire's Smart Mode to keep the dGPU blocked for this application, ensuring it runs solely on the integrated graphics to save power.