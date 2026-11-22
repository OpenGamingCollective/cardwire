# Building and Development

Cardwire is a Rust project, build is made to work on a Linux system.

I personally build and develop on a NixOS installation.

### Development

### Toolbox

I have started writing a toolbox in [Nushell](https://www.nushell.sh/)

The script can be run using `just toolbox`

### Without Nix

The project has a `rust-toolchain.toml`, this is the version it is using.

Any commits that don't work in this version will be rejected.

Cardwire uses [Aya](https://aya-rs.dev/) to build its eBPF program in Rust.

Rust ebpf targets are tier 3, which mean we have to compile the `core` crate ourself :

- Install the `rustc` from your distribution
- Install `bpf-linker`, either from your distrubution if possible, or using `cargo binstall`
- Build with `RUSTC_BOOTSTRAP=1`

More informations inside the [Aya official guide](https://aya-rs.dev/book/start/development.html)

> [!NOTE]
> This repository CI does not use nightly rust, everything runs on a stable rustc
> from [fenix](https://github.com/nix-community/fenix)

### Using Nix

```bash
# Enter development shell
nix develop

# Build the project
nix build .#

# Run formatting checks
nix build .#checks.x86_64-linux.pre-commit-check

# Run integration tests in VM
nix build .#checks.x86_64-linux.vm-ci-2gpu
nix build .#checks.x86_64-linux.vm-ci-3gpu
nix build .#checks.x86_64-linux.vm-ci-15gpu

# Build the vm and enter
nix run .#nixosConfigurations.x86_64-linux.config.system.build.vm
```
