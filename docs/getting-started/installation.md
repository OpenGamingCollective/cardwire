# Installation

## Arch

using AUR:

```bash
yay -S cardwire
```

then enable the service and reboot

```bash
sudo systemctl enable cardwired.service
reboot
```

> [!NOTE]
> cardwire-git is also available, this one is built from the dev branch, rather than a fixed release tag

> [!IMPORTANT]  
> i'm also looking for an official maintainer for both AUR, since i do not use Arch.

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

services.cardwire.enable = true;
```

## Fedora

Using the offical copr:

```bash
sudo dnf copr enable luytan/cardwire

sudo dnf install cardwire
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
