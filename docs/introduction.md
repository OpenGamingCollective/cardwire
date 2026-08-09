# Introduction

Cardwire is a GPU manager for Linux systems with multiple GPUs. It allows users to smoothly and safely switch between "integrated", "hybrid" and more GPU modes. It was created as the successor to the deprecated [supergfxctl](https://gitlab.com/asus-linux/supergfxctl) project.

## Why Cardwire?

Traditional GPU managers for Linux (like envycontrol, optimus-manager, supergfxctl) often require [system restarts](https://github.com/bayasdev/envycontrol), [display manager logouts](https://gitlab.com/asus-linux/supergfxctl), or rely on legacy [X11 architectures](https://github.com/Askannz/optimus-manager). Other built-in tools (like switcheroo-control) are great for launching apps but don't actively protect the dedicated GPU from being woken up by misbehaving background applications.

Cardwire solves this by using **eBPF (Extended Berkeley Packet Filter) and LSM (Linux Security Modules)** to dynamically block access to the GPU. This ensures the GPU can enter its deepest sleep state (`D3Cold`, a hardware state that uses almost zero power) without requiring logouts or reboots to change modes.

Furthermore, unlike older managers, **Cardwire never unbinds PCI devices or kernel drivers**. Unbinding drivers on the fly is notoriously unstable and is a frequent cause of crashes on AMD GPUs or system deadlocks on NVIDIA GPUs. Cardwire's eBPF approach is entirely seamless and significantly more stable.

### Comparison

| GPU Manager            | How it works                                                 | Unbinds Drivers?           | Requires Reboot/Logout | Notes                                                                                                                                 |
| :--------------------- | :----------------------------------------------------------- | :------------------------- | :--------------------- | :------------------------------------------------------------------------------------------------------------------------------------ |
| **Cardwire**           | **eBPF LSM hooks** block file/device access dynamically.     | **No** (Seamless)          | **No**                 | Actively prevents rogue apps from waking the dGPU. Emulates switcheroo-control for seamless GNOME/KDE integration.                    |
| **switcheroo-control** | Sets environment variables (e.g. `DRI_PRIME`).               | **No**                     | No                     | The desktop default. Good for launching, but doesn't actively block apps, meaning the dGPU can still be woken up by background tasks. |
| **supergfxctl**        | Modprobe blacklisting, udev rules, stopping display manager. | **Yes** (Prone to crashes) | Logout (often)         | Deprecated. The predecessor to Cardwire, inflexible and often required restarting the graphical session.                              |
| **optimus-manager**    | Generates specific Xorg configurations.                      | **Yes**                    | Logout                 | Built heavily around X11, making it problematic for modern Wayland compositors.                                                       |
| **envycontrol**        | Modprobe blacklisting and udev rules.                        | **Yes**                    | Reboot                 | Very reliable but inflexible, as it requires a full system restart to apply any mode changes.                                         |

## Modes

Cardwire provides several GPU management modes:

- **Integrated mode** -- Uses eBPF LSM hooks to block applications from accessing dedicated GPUs. This saves power by preventing the GPU from waking up and allowing it to enter an energy-efficient sleep state (`D3Cold`).

- **Hybrid mode** -- Removes the blocks, letting the system function normally with both integrated and dedicated GPUs available.

- **Manual mode** -- Allows users to manually block or unblock individual GPUs by ID for granular control. It is only available on desktop systems and never blocks the default GPU.

- **Smart mode** -- Like integrated mode it blocks the dGPU by default, but a userspace analyzer inspects each application at launch and selectively allows GPU access for approved applications. It is only available on laptops.

Switching between modes is fast and does not require reboots or logouts.

> [!CAUTION]
> Cardwire is in an early development stage, expect breaking changes.

## Getting Started

To get started with cardwire, please take a look at the [requirements](getting-started/requirements.md) to make sure your system is supported and configured, then head over to [the installation instructions](getting-started/installation.md).
