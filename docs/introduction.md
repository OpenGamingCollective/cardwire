# Introduction

Cardwire is a GPU manager for Linux systems with multiple GPUs. It allows users to smoothly and safely switch between "integrated", "hybrid" and more GPU modes. It was created as the successor to the deprecated [supergfxctl](https://gitlab.com/asus-linux/supergfxctl) project.

## Why Cardwire?

Traditional GPU managers for Linux (like envycontrol, optimus-manager, supergfxctl) often require system restarts or display manager logouts. And other built-in tools (like switcheroo-control) are great for launching apps but don't actively protect the dedicated GPU from being woken up, and cannot force an APP to run on a specific GPU.

Cardwire solves this by using **eBPF and LSM** to dynamically block access to the GPU (more info about those [here](https://youtu.be/eVsMkXDE_5I)). This ensures the GPU is blocked at a userspace level.

Furthermore, unlike older managers, **Cardwire never unbinds PCI devices or unload kernel drivers**. Unbinding drivers on the fly is notoriously unstable and is a frequent cause of crashes on AMD GPUs or system deadlocks on NVIDIA GPUs. Cardwire's eBPF approach is entirely seamless and significantly more stable.

## Modes

Cardwire provides several GPU management modes:

- **Integrated mode** - Block applications from accessing dedicated GPU, leaving only the iGPU available

- **Hybrid mode** -- Removes the blocks, letting the system function normally with both integrated and dedicated GPUs available.

- **Manual mode** -- Allows users to manually block or unblock individual GPUs by ID for granular control. It is only available on desktop systems and never blocks the default GPU.

- **Smart mode** -- Like integrated mode it blocks the dGPU by default, but a userspace analyzer inspects each application at launch and selectively allows GPU access for approved applications. It is only available on laptops.

> [!WARNING]
> Integrated and Smart mode are only available for laptops, desktop/multi-gpus have access to Hybrid & Manual

Switching between modes is fast and does not require reboots or logouts.

> [!CAUTION]
> Cardwire is in an early development stage, expect breaking changes.

## Getting Started

To get started with cardwire, please take a look at the [requirements](getting-started/requirements.md) to make sure your system is supported and configured, then head over to [the installation instructions](getting-started/installation.md).
