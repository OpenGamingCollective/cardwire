# Config

`org.opengamingcollective.cardwire.Config`

Served at the root object path `/org/opengamingcollective/cardwire`.

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