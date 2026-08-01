# Usage

## Querying GPUs

To have cardwire list all detected GPUs, use:

```bash
cardwire list
```

For each detected GPU, the command will return:

- An identifier (`ID`). These are used for manual blocking and unblocking.
- The GPU's name (`NAME`)
- The GPU's PCI address (`PCI`)
- The associated render node (`RENDER`)
- The associated device node (`CARD`)
- Whether the GPU has been identified as the default GPU (`DEFAULT`). Default GPUs will remain available when cardwire is set to integrated.
- Whether the GPU is currently blocked (`BLOCKED`)

Example:

```bash
$ cardwire list
ID  NAME                                         PCI           RENDER      CARD   DEFAULT  BLOCKED
--  -------------------------------------------  ------------  ----------  -----  -------  -------
0   Rembrandt [Radeon 680M]                      0000:07:00.0  renderD129  card2  (*)      false
1   Navi 23 [Radeon RX 6650 XT / 6700S / 6800S]  0000:03:00.0  renderD128  card1  ( )      true
```

## Mode switching

GPU modes can be switched using the `cardwire set` command.

### Integrated

To have cardwire block the dGPU, use:

```bash
cardwire set integrated
```

> [!NOTE]
> The block only applies to new launched apps. Apps that are already running will keep using the GPU until you restart them. Restarting can help.

> [!TIP]
> The dedicated GPU can still power down even if your desktop has it open, as long as nothing is actively using it. To double-check, run: `cardwire gpu 1 --lsof`

### Hybrid

To have cardwire allow access to all GPUs, use

```bash
cardwire set hybrid
```

### Smart Mode

Smart mode blocks the dedicated GPU by default like integrated mode, but uses a real-time analyzer to scan each application at launch and selectively allow GPU access for approved apps.

Cardwire natively integrates with desktop environments (GNOME, KDE) via a **Switcheroo DBus shim**. This means you can simply right-click an application in your app launcher and select **"Launch using Discrete Graphics Card"**, and Cardwire will automatically unblock the GPU for that application.

> [!TIP]
> When an application is launched via the Switcheroo UI or with `CARDWIRE_FORCE_DGPU=1`, Cardwire will hide the integrated GPU (iGPU) from the app. The app will _only_ be able to see and use the dedicated GPU, guaranteeing it runs on the correct hardware.

When launching apps in Smart mode, cardwire checks for the following to allow the dGPU:

- `CARDWIRE_ALLOW=1` env var (highest priority, unblocks the GPU but doesn't force the app to use it)
- `CARDWIRE_FORCE_DGPU=1` env var (unblocks the GPU, forces the app to use it, and completely hides the iGPU)
- Steam games (`SteamAppId=`)
- Flatpak apps with XDG `PrefersNonDefaultGpu=true` (Only on system that does not implement switcheroo/cardwire)
- Explicit GPU env vars (`DRI_PRIME=1`, `__NV_PRIME_RENDER_OFFLOAD=1`)

```bash
cardwire set smart
```

> [!Note]
> This feature is a work in progress. The detection methods will be improved in future updates.

### Manual

> [!IMPORTANT]
> To prevent system breakage, cardwire will not block the default GPU, even when explicitly instructed to do so.

If more granular control over several GPUs is required, cardwire also allows manually blocking individual GPUs by ID. To do so, it needs to be set to manual mode:

```bash
cardwire set manual
```

Once set to manual, GPU states can then be set by ID; to find the correct ID, see [Querying GPUs](#Querying-GPUs).

To block the GPU with ID `1`:

```bash
cardwire gpu 1 --block
```

To unblock:

```bash
cardwire gpu 1 --unblock
```

## Configuration

### Experimental Nvidia Block

> [!NOTE]
> This setting is experimental because it tells cardwire to block specific Nvidia files, such as /dev/nvidiactl, that can be shared across multiple Nvidia GPUs. For this reason, it only works reliably on systems with exactly two GPUs: one integrated GPU and one dedicated Nvidia GPU.

> [!TIP]
> Even though it is experimental, enabling this setting is recommended. It helps prevent unwanted GPU wakeups from Vulkan apps (GTK on gnome) and from tools that use /dev/nvidiactl, such as nvtop

To get if experimental Nvidia block is enabled:

```bash
cardwire config experimental-nvidia-block
```

To enable/disable it:

```bash
cardwire config experimental-nvidia-block true
```

### Battery Auto Switch Mode

Cardwire can automatically switch GPU modes when the system switches between battery and AC power. When `battery_auto_switch` is enabled, cardwire switches to integrated mode on battery and back to a configurable mode when on AC power.

To get if battery auto switch is enabled:

```bash
cardwire config battery-auto-switch
```

To enable/disable it:

```bash
cardwire config battery-auto-switch true
```

The mode cardwire switches to on AC power is controlled by `battery_auto_switch_mode`. This can be set to `integrated`, `hybrid`, `manual`, or `smart`.

To get the current battery auto switch mode:

```bash
cardwire config battery-auto-switch-mode
```

To set the battery auto switch mode:

```bash
cardwire config battery-auto-switch-mode hybrid
```

### External Display Auto Switch

Cardwire can switch to hybrid when an external display is connected to a port that is wired
directly to the dedicated GPU.

This feature is disabled by default. Enable or disable it with:

```bash
cardwire config external-display-auto-switch true
cardwire config external-display-auto-switch false
```

The mode restored after disconnect defaults to integrated and can be set to any Cardwire mode:

```bash
cardwire config external-display-auto-switch-mode
cardwire config external-display-auto-switch-mode smart
```

### Auto Apply Gpu State

When you switch back to manual mode, this setting automatically restores the GPU states you had set before. These saved states are stored in `/var/lib/cardwire/gpu_state.json`

To view the current saved states, run:

```bash
cat /var/lib/cardwire/gpu_state.json
```

Example output:

```json
{
  "0000:03:00.0": {
    "block": false
  },
  "0000:07:00.0": {
    "block": false
  }
}
```

In this example, both GPUs are set to allow access (block: false) when manual mode is restored

To get the current setting:

```bash
cardwire config auto-apply-gpu-state
```

To set the setting:

```bash
cardwire config auto-apply-gpu-state true
```
