# Introduction

Cardwire is a GPU manager for Linux systems with multiple GPUs. It allows users to smoothly and safely switch between "integrated" and "hybrid" GPU modes. It was created as the successor to the deprecated [supergfxctl](https://gitlab.com/asus-linux/supergfxctl) project.

## Modes

Cardwire provides several GPU management modes:

- **Integrated mode** — Uses eBPF LSM hooks to block applications from accessing dedicated GPUs. This saves power by preventing the GPU from waking up and allowing it to enter an energy-efficient sleep state (`D3Cold`).

- **Hybrid mode** — Removes the blocks, letting the system function normally with both integrated and dedicated GPUs available.

- **Manual mode** — Allows users to manually block or unblock individual GPUs by ID for granular control.

- **Smart mode** — Like integrated mode it blocks the dGPU by default, but uses eBPF to analyze each application at launch and selectively allow GPU access for approved applications.

Switching between modes is fast and does not require reboots or logouts.

> [!CAUTION]
> Cardwire is in an early development stage, expect breaking changes and instability.

## Getting Started

To get started with cardwire, please take a look at the [requirements](getting-started/requirements.md) to make sure your system is supported, then head over to [the installation instructions](getting-started/installation.md).
