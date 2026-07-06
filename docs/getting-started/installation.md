# Installation

## Arch

using AUR:

```bash
yay -S cardwire
```

Then [enable BPF LSM](#enabling-bpf-lsm) and start the service:

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

The NixOS module configures BPF LSM automatically — no manual kernel parameter changes needed.

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
```

Then [enable BPF LSM](#enabling-bpf-lsm) and start the service:

```bash
sudo systemctl enable cardwired --now
```

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

## Enabling BPF LSM (with GRUB)

Check your kernel's default LSM list:

On Ubuntu/Fedora:
```bash
grep CONFIG_LSM= /boot/config-$(uname -r)
```

On Arch and other distros:
```bash
zcat /proc/config.gz | grep CONFIG_LSM=
```

> Outputs e.g. `CONFIG_LSM="landlock,lockdown,yama,integrity,apparmor"`

Edit `/etc/default/grub` and append `bpf` to `GRUB_CMDLINE_LINUX_DEFAULT`, keeping all existing entries:

```
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash lsm=landlock,lockdown,yama,integrity,apparmor,bpf"
```

> [!IMPORTANT]
> Do not set `lsm=bpf` alone — that drops other active security policies. Always append `bpf` to the existing list from the command above.

Apply and reboot:

| Distro | Command |
|--------|---------|
| Ubuntu | `sudo update-grub` |
| Fedora | `sudo grub2-mkconfig -o /boot/grub2/grub.cfg` |
| Arch   | `sudo grub-mkconfig -o /boot/grub/grub.cfg` |

```bash
sudo reboot
```

## Other distros

For now, other distros must clone the repo and use `make` to build and install Cardwire. You will also need to enable BPF LSM manually — see the [Enabling BPF LSM](#enabling-bpf-lsm) section above.

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
