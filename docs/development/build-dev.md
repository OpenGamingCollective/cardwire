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
nix build .#checks.x86_64-linux.vm-ci-2gpu
nix build .#checks.x86_64-linux.vm-ci-3gpu
nix build .#checks.x86_64-linux.vm-ci-15gpu

# Run GUI D-Bus tests on a private session bus
nix build .#checks.x86_64-linux.gui-dbus

# Build the vm and enter
nix run .#nixosConfigurations.x86_64-linux.config.system.build.vm
```

### Manual Compilation

If you don't use Nix, ensure you have `clang`, `libbpf (devel)`, `libudev (devel)`, `pkg-config` and `cargo` installed (needed for eBPF compilation during the Rust build), plus `bpf-linker` and a pinned nightly toolchain (see `cardwire-ebpf-userspace/build.rs`). The GUI build additionally needs the Vulkan, EGL, Wayland and X11 development packages.

Formatting requires nightly `rustfmt` (the project uses nightly-only formatting options). Install it and run with:

```bash
rustup toolchain install nightly --component rustfmt
cargo +nightly fmt --all --check
```

Run the regular Rust tests from the repository root:

```sh
cargo test --locked
```

The GUI instance D-Bus tests are ignored by default because they claim the real
`org.opengamingcollective.cardwire.Gui` session bus name. Run them on a private bus
so they cannot interact with a running Cardwire GUI:

```sh
dbus-run-session -- cargo test --locked -p cardwire-gui helpers::dbus::tests:: -- --ignored
```

These tests need `dbus-run-session` and `dbus-daemon` (the `dbus-daemon` package on
Ubuntu, or `dbus` in the Nix development shell).

```bash
# Build the project
make

# Install binaries, systemd service, D-Bus config, desktop file, icons and
# metainfo, and enable the systemd unit (requires sudo)
sudo make install
```

## Project Structure

- `crates/cardwire-cli`: User CLI to interact with the daemon
- `crates/cardwire-daemon`: System daemon managing state and D-Bus communication
- `crates/cardwire-ebpf`: BPF program and LSM hooks (never built directly, built by ebpf-userspace)
- `crates/cardwire-ebpf-userspace`: Loads the BPF program, compiles it at build time
- `crates/cardwire-gui`: The iced GUI and tray
