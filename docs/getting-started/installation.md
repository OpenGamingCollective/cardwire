# Installation

## Arch

using AUR:

```bash
yay -S cardwire
```

then enable and start the service

```bash
sudo systemctl enable cardwired.service

sudo systemctl start cardwired.service
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

## Fedora

Using Terra

```bash
sudo dnf install cardwire

sudo systemctl enable cardwired.service

sudo systemctl start cardwired.service
```

## Other distros

For now, other distros must clone the repo and use `make` to build and install Cardwire.

Build dependencies:

- cargo
- clang
- libbpf

```bash
git clone https://github.com/OpenGamingCollective/cardwire.git

make build
sudo make install
```

> [!CAUTION]
> Makefile wasn't tested, use with caution.

> [!IMPORTANT]  
> For mainstream distros, i will be making an official install methods, like a copr for Fedora and a .deb for Debian based.

## Non-systemd distros

> [!WARNING]
> Cardwire only supports systemd-based distros. If you want to use it on a non-systemd distro, either open a PR with patches for non-systemd or get it working on your setup.

## Display server support

> [!CAUTION]
> X11 is not tested and not supported. Cardwire requires Wayland to function properly.
