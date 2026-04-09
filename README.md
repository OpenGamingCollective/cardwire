# cardwire

a GPU manager for Linux using eBPF LSM hooks to block/hide GPUs
# WARNING!!
- This project is in early development. Expect bugs and incomplete functionality
- It doesnt work with NVIDIA GPUs, since i do not own one, feel free to PR this
- Makefile was AI-generated, i only use the flake.nix, be careful
## Prerequisites

Before using `cardwire`, ensure your system meets these requirements:

- **Linux Kernel**: version 5.7 or later
- **eBPF LSM Support**: Your kernel must have `CONFIG_BPF_LSM=y`

### How to check:
```bash
# Check kernel version (should be >= 5.7)
uname -r

# Check if BPF LSM is enabled in your kernel
zgrep CONFIG_BPF_LSM /proc/config.gz || grep CONFIG_BPF_LSM /boot/config-$(uname -r)

# Check if 'bpf' is in the active LSM list
cat /sys/kernel/security/lsm
```

## Usage

The `cardwire` CLI allows you to manage GPU states and system modes

### Modes
- **Manual**: Default mode for safety, allows individual GPU blocking/unblocking
- **Integrated**: Automatically blocks the discrete GPU 
- **Hybrid**: Enables the dGPU for use (unblocked)

**Note :** Integrated/Hybrid modes only work on host with two GPUs
**Note 2 :** Manual mode is not implemented
```bash
# Set system mode
cardwire set integrated/hybrid/manual

# Get current mode status
cardwire get

# List all detected GPUs and their status
cardwire list

# Manually block/unblock a specific GPU by ID
cardwire gpu 1 block on
cardwire gpu 1 block off

# Get detailed info for a specific GPU, will probably be deprecated
cardwire gpu 1 info
```

## Configuration

The daemon reads its configuration from `/var/lib/cardwire/cardwire.toml`. If the file is missing, it defaults to `Manual` mode. (Config parsing not tested yet)

```toml
# /var/lib/cardwire/cardwire.toml
mode = "Manual"
```

## Building and Development

### Using Nix

```bash
# Enter development shell
nix develop

# Build the project
nix build

# Build the vm and enter
nix run .#nixosConfigurations.x86_64-linux.config.system.build.vm
```

### Manual Compilation
If you don't use Nix, ensure you have `clang`, `libbpf` and `cargo` installed (needed for eBPF compilation during the Rust build)

```bash
# Build the project
make

# Install binaries, systemd service, and D-Bus config (requires sudo)
sudo make install
```

## How it works

Cardwire uses eBPF with LSM hooks to intercept file operations on GPU device nodes, such as `/dev/dri/renderDX` and `/dev/dri/cardX`, and sysfs `config` file

When a GPU is "blocked," the eBPF program returns `-ENOENT` for any `open` syscall targeting that device. This provides several key benefits:

*   **Instant App Startup:** Prevents applications (like Electron apps) from attempting to initialize the GPU, this eliminates the 3–4 second "hang" typically caused by waiting for a sleeping GPU to power up
*   **Power Efficiency:** By blocking access at the syscall level, the GPU is never woken from its lowest power state (D3cold), extending battery life for laptops
*   **Non-Invasive:** Unlike traditional methods that might require driver unloading, risky unbind or complex X11/Wayland configurations, this approach is transparent to the rest of the system and easily toggled
*   Also works with games

## Project Structure

- `crates/cardwire-core`: Low-level GPU manager and IOMMU discovery
- `crates/cardwire-daemon`: System daemon managing state and D-Bus communication
- `crates/cardwire-cli`: User CLI to interact with the daemon
- `crates/cardwire-ebpf`: BPF program and LSM hooks

## Notes
- I'm not a senior dev,, if you think the code is objectively bad, feel free to make a PR
- Credits to asus-linux discord for helping me find this ebpf method

## References used
- https://docs.ebpf.io/
- CaelestiaShell for the flake.nix part

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
