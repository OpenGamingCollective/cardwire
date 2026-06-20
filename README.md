# cardwire

[![Packaging status](https://repology.org/badge/vertical-allrepos/cardwire.svg)](https://repology.org/project/cardwire/versions)

[![GitHub License](https://img.shields.io/github/license/OpenGamingCollective/cardwire)](https://github.com/OpenGamingCollective/cardwire/blob/main/LICENSE)

A GPU manager for Linux using eBPF LSM hooks to block GPUs

Creator and Main maintainer: @luytan

# Disclaimer

- This project is in early development. Expect bugs and incomplete functionality

## Getting Started

Head to the [docs](https://opengamingcollective.github.io/cardwire) to see how to install Cardwire on your system

## Usage

The `cardwire` CLI lets you manage GPU states and system modes

### Modes

- **Integrated**: Blocks the discrete GPU
- **Hybrid**: Unblocks the discrete GPU
- **Manual**: Default mode for safety, allows individual GPU blocking/unblocking
- **Smart**: Like integrated mode but uses eBPF to analyze applications at launch and selectively allow GPU access

_Note: Integrated/Hybrid/Smart modes only work on systems with exactly two GPUs_

```bash
# Set system mode
cardwire set integrated / hybrid / manual / smart

# Get current mode status
cardwire get

# List all detected GPUs and their status
cardwire list

# Manually block/unblock a specific GPU by ID
cardwire gpu 1 --block
cardwire gpu 1 --unblock
```

## Configuration

The daemon reads its configuration from `/etc/cardwire/cardwire.toml`.

```toml
# /etc/cardwire/cardwire.toml
auto_apply_gpu_state = true
experimental_nvidia_block = false
battery_auto_switch = false
battery_auto_switch_mode = "hybrid"
```

`experimental_nvidia_block` is an experimental feature that blocks specific NVIDIA files, must be used with caution.

`battery_auto_switch_mode` controls which mode cardwire switches to on AC power when `battery_auto_switch` is enabled. Can be set to `integrated`, `hybrid`, `manual`, or `smart`.

## Community projects:
_for issues related to these projects, please report to their respective repo_


GNOME extension (by Moxuz):
https://extensions.gnome.org/extension/9919/cardwire-gpu-toggle/

Cardwire-tray (by SeawolfTony):
https://github.com/JuanDelPueblo/cardwire-tray


## How it works

Cardwire uses eBPF with LSM hooks to intercept file operations on GPU device nodes, such as `/dev/dri/renderDX`, `/dev/dri/cardX`, sysfs `config` and `nvidiaX`

When a GPU is "blocked," the eBPF program returns `-ENOENT` for any syscall targeting that device. This provides several key benefits:

- **Instant App Startup:** Prevents applications (like Electron apps or GTK apps) from attempting to initialize the GPU, this eliminates the 3–4 second "hang" typically caused by waiting for a sleeping GPU to power up
- **Power Efficiency:** By blocking access at the syscall level, the GPU is never woken from its lowest power state (D3cold), extending battery life on laptops
- **Non-Invasive:** Unlike traditional methods that might require driver unloading, risky unbind or complex Wayland setups, this approach is transparent to the rest of the system and easy to toggle

_Note: X11 is not supported. Cardwire requires Wayland._
- _Also works with games_

## Notes

- I'm still learning Rust, if some parts of the code are bad or unoptimized, feel free to open a PR

## Credits

- Asus-linux Discord for helping me find the ebpf method

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
