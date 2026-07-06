# Installation

## Arch/CachyOS/Arch-based

using AUR:

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

## Ubuntu

Install build dependencies:

```bash
sudo apt install clang libbpf-dev linux-headers-$(uname -r)
```

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

Then [enable BPF LSM](#enabling-bpf-lsm) and start the service:

```bash
sudo systemctl enable cardwired --now
```

## Other distros

For now, other distros must clone the repo and use `make` to build and install Cardwire. You will also need to enable BPF LSM manually — see the [Enabling BPF LSM](#enabling-bpf-lsm) section above.

Build dependencies:

- cargo
- clang
- libbpf
- libudev-dev

```bash
git clone https://github.com/OpenGamingCollective/cardwire.git

make build
sudo make install
```

> [!NOTE]
> A .deb package for Debian based system is planned.

