# Building and Development

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

## Project Structure

- `crates/cardwire-cli`: User CLI to interact with the daemon
- `crates/cardwire-core`: Low-level GPU manager and IOMMU discovery
- `crates/cardwire-daemon`: System daemon managing state and D-Bus communication
- `crates/cardwire-ebpf`: BPF program and LSM hooks