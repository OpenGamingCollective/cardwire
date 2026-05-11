# Introduction

Cardwire is GPU manager for Linux systems with Hybrid GPU configurations which allows users to smoothly and safely switch between "integrated" and "hybrid" GPU modes. It was created as a successor to the deprecated [supergfxctl](https://gitlab.com/asus-linux/supergfxctl) project.

In "Integrated" mode, cardwire uses eBPF LSM hooks to block applications from accessing dedicated GPUs. This saves power by preventing the GPU from being woken up and allowing it to enter an extremely energy-efficient sleep state (`D3Cold`). In "Hybrid" mode, these blocks are removed and the system functions as it usually would. As a safety option and for users who require more granular control, cardwire additionally allows users to manually block and unblock individual GPUs by ID. Switching is fast and does not require reboots or logouts to take effect. 

> [!CAUTION]
> Cardwire is in an early development stage, expect breaking changes and instability.

## Getting Started

To get started with cardwire, please take a look at the [requirements](getting-started/requirements.md) to make sure your system is supported, and then head over to [the installation instructions](getting-started/installation.md). 
