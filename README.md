# cardwire

[![Packaging status](https://repology.org/badge/vertical-allrepos/cardwire.svg)](https://repology.org/project/cardwire/versions)

[![GitHub License](https://img.shields.io/github/license/OpenGamingCollective/cardwire)](https://github.com/OpenGamingCollective/cardwire/blob/main/LICENSE)

A GPU manager for Linux using eBPF LSM hooks to block GPUs

![Cardwire GUI screenshot](./assets/com.github.opengamingcollective.cardwire.screenshot.png)

Creator and Main maintainer: @luytan

# Disclaimer

- This project is in early development. Expect bugs and incomplete functionality

## Getting Started

Head to the [docs](https://opengamingcollective.github.io/cardwire) to see how to install and use Cardwire on your system

## How it works

Cardwire uses eBPF with LSM hooks to intercept file operations on GPU device nodes, such as `/dev/dri/renderDX`, `/dev/dri/cardX`, sysfs `config`, `nvidiaX` and other GPU-related files.

When a GPU is "blocked," the eBPF program returns `-ENOENT` for any syscall targeting that device, effectively hiding it from apps. This provides several key benefits:

- **Instant App Startup:** Prevents applications (like Electron apps or GTK apps) from attempting to initialize the GPU, this eliminates the 3–4 second "hang" typically caused by waiting for a sleeping GPU to power up
- **Power Efficiency:** By blocking access at the syscall level, the GPU is never woken from its lowest power state (D3cold), extending battery life on laptops
- **Non-Invasive:** Unlike traditional methods that might require driver unloading, risky unbind or complex Wayland setups, this approach is transparent to the rest of the system and easy to toggle

_Note: X11 is not supported. Cardwire requires Wayland._

## Community projects:

_for issues related to these projects, please report to their respective repo_

GNOME extension (by Moxuz):
https://extensions.gnome.org/extension/9919/cardwire-gpu-toggle/

## Discord

Need help or have a question about Cardwire? Join us on the OGC Discord:
https://discord.gg/4K2pZ6abQm

## Credits

- Huge thanks to the asus-linux community for their guidance and for suggesting the eBPF approach.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
