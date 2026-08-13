# Installation

## Arch/CachyOS/Arch-based

The official Open Gaming Collective Arch packages are available here: [ogc-arch-packaging](https://github.com/OpenGamingCollective/ogc-arch-packaging#Using-the-repo)

Alternatively, using the AUR:

```bash
yay -S cardwire
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```

## Nix

Using the repo's flake:

flake.nix:

```nix
cardwire = {
    url = "github:opengamingcollective/cardwire";
    inputs.nixpkgs.follows = "nixpkgs";
};
```

configuration.nix:

```nix
imports = [ inputs.cardwire.nixosModules.default ];

services.cardwire = {
 enable = true;
 settings = {
     auto_apply_gpu_state = true;
     experimental_nvidia_block = true;
     battery_auto_switch = true;
     battery_auto_switch_mode = "hybrid";
     external_display_auto_switch = false;
 };
};
```

## Fedora/Fedora-based

Cardwire is officially distributed through Terra on Fedora systems

To install Terra, follow the instructions here:
<https://docs.terrapkg.com/usage/installing>

Using Terra

```bash
sudo dnf install cardwire
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```

## Bazzite/Atomic Fedora-based

Cardwire is officially distributed through Terra on Fedora systems

To install Terra, follow the instructions here:
<https://docs.terrapkg.com/usage/installing>

Using Terra

```bash
sudo rpm-ostree install cardwire
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```

> [!NOTE]
> Thanks to the Fyra Labs / Terra team for packaging and maintaining Cardwire on Fedora !!

## AerynOS

Cardwire is available in the official AerynOS repositories.

```bash
sudo moss install cardwire
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```

## Ubuntu / Debian

A `.deb` package is available on the [releases page](https://github.com/OpenGamingCollective/cardwire/releases). You can download and install it using `apt`.

```bash
sudo apt install ./cardwire_*.deb
```

```bash
sudo systemctl enable cardwired --now
```

### Build from source (Alternative)

Install build dependencies:

```bash
sudo apt install libbpf-dev libudev-dev pkg-config libegl1-mesa-dev libvulkan-dev libglvnd-dev libwayland-dev libxkbcommon-dev libx11-dev libxcb1-dev libx11-xcb-dev gcc-multilib
```

The eBPF program also requires `bpf-linker` and a pinned nightly Rust toolchain with the `rust-src` component (see `cardwire-ebpf-userspace/build.rs`). The devshell provided by `nix develop` bundles all of these (excepted bpf-linker which need to be installed using `cargo install bpf-linker`)

Install Rust (if not already installed):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then clone and build:

```bash
git clone https://github.com/OpenGamingCollective/cardwire.git

make build
sudo make install
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```

## Other distros

For now, other distros must clone the repo and use `make` to build and install Cardwire. You will also need to enable BPF LSM manually, see the [Enabling BPF LSM (with GRUB)](#enabling-bpf-lsm-with-grub) section above.

Build dependencies:

- cargo (plus `bpf-linker` and a pinned nightly toolchain for the eBPF program)
- libbpf
- libudev-dev
- pkg-config
- Vulkan, EGL, Wayland and X11 development packages (GUI build)

```bash
git clone https://github.com/OpenGamingCollective/cardwire.git

make build
sudo make install
```

And start the service:

```bash
sudo systemctl enable cardwired --now
```
