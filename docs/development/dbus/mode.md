# Mode

`org.opengamingcollective.cardwire.Mode`

Served at the root object path `/org/opengamingcollective/cardwire`.

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

See the [Smart Mode](../smart.md) page for the smart-mode policy engine.
