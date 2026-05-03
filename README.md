# cardwire

[![Packaging status](https://repology.org/badge/vertical-allrepos/cardwire.svg)](https://repology.org/project/cardwire/versions)

[![GitHub License](https://img.shields.io/github/license/OpenGamingCollective/cardwire)](https://github.com/OpenGamingCollective/cardwire/blob/main/LICENSE)

A GPU manager for Linux using eBPF LSM hooks to block GPUs

Creator and Main maintainer: @luytan

# Disclaimer

- This project is in early development. Expect bugs and incomplete functionality
> [!NOTE]
> Regarding the use of AI in this project: AI was used early on to prototype and fix some errors. The code has since been mostly rewritten by hand. The only AI-generated code left is in `pci_devices.rs`, specifically the `load_pci_name_db` function, which will be rewritten soon

## Getting Started

Head to the [docs](https://opengamingcollective.github.io/cardwire) to see how to install Cardwire on your system

## Usage

The `cardwire` CLI lets you manage GPU states and system modes

### Modes

- **Integrated**: Blocks the discrete GPU
- **Hybrid**: Unblocks the discrete GPU
- **Manual**: Default mode for safety, allows individual GPU blocking/unblocking

_Note: Integrated/Hybrid modes only work on host with two GPUs_

_Note 2: Manual mode is not implemented_

```bash
# Set system mode
cardwire set integrated / hybrid / manual

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
block_nvidia_vulkan = false
battery_auto_switch = false
```

`block_nvidia_vulkan` is an experimental feature that blocks the nvidia's vulkan icd, must be used with caution

## Building and Development

### Using Nix

```bash
# Enter development shell
nix develop

# Build the project
nix build

# Run formatting checks
nix build .#checks.x86_64-linux.pre-commit-check

# Run integration tests in VM
nix build .#checks.x86_64-linux.vm-test

# Build the vm and enter
nix run .#nixosConfigurations.x86_64-linux.config.system.build.vm
```

### Manual Compilation

If you don't use Nix, ensure you have `clang`, `libbpf (devel)`, hwdata and `cargo` installed (needed for eBPF compilation during the Rust build)

```bash
# Build the project
make

# Install binaries, systemd service, and D-Bus config (requires sudo)
sudo make install
```

## How it works

Cardwire uses eBPF with LSM hooks to intercept file operations on GPU device nodes, such as `/dev/dri/renderDX`, `/dev/dri/cardX`, sysfs `config` and `nvidiaX`

When a GPU is "blocked," the eBPF program returns `-ENOENT` for any syscall targeting that device. This provides several key benefits:

- **Instant App Startup:** Prevents applications (like Electron apps or GTK apps) from attempting to initialize the GPU, this eliminates the 3–4 second "hang" typically caused by waiting for a sleeping GPU to power up
- **Power Efficiency:** By blocking access at the syscall level, the GPU is never woken from its lowest power state (D3cold), extending battery life on laptops
- **Non-Invasive:** Unlike traditional methods that might require driver unloading, risky unbind or complex X11/Wayland setups, this approach is transparent to the rest of the system and easy to toggle
- _Also works with games_

## Project Structure

- `crates/cardwire-cli`: User CLI to interact with the daemon
- `crates/cardwire-core`: Low-level GPU manager and IOMMU discovery
- `crates/cardwire-daemon`: System daemon managing state and D-Bus communication
- `crates/cardwire-ebpf`: BPF program and LSM hooks

## Notes

- I'm still learning Rust, if some parts of the code are bad or unoptimized, feel free to open a PR

## Credits

- Asus-linux Discord for helping me find the ebpf method
- Caelestia shell for the flake.nix, i used it as a reference

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
