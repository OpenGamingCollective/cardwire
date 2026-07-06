# Requirements

Requirements to be able to run Cardwire:

## Kernel

### 1. Version 5.7 or later

```bash
uname -r
```

### 2. Built with `CONFIG_BPF_LSM` enabled

On e.g. Ubuntu/Fedora:

```bash
grep CONFIG_BPF_LSM /boot/config-$(uname -r)
```

On other distros possibly:

```bash
zcat /proc/config.gz | grep CONFIG_BPF_LSM
```

> returns `CONFIG_BPF_LSM=y` if it's enabled

### 3. Bpf present in the boot cmdline

On most distros

```bash
cat /proc/cmdline | tr ' ' '\n'|grep lsm
```

Alternative methods:

```bash
grep CONFIG_LSM= /boot/config-$(uname -r)
```

or

```bash
zcat /proc/config.gz | grep CONFIG_LSM=
```

> Outputs e.g. `lsm=landlock,yama,apparmor,bpf` or `CONFIG_LSM="landlock,lockdown,yama,integrity,apparmor,bpf"`

> If it contains 'bpf', bpf is already enabled and usable in your system, go to [installation](getting-started/installation.md)

## Enabling BPF LSM (with GRUB)

> [!CAUTION]
> bpf should already be enabled by default on these distros: Arch, CachyOS, Bazzite, Fedora, NixOS, Debian

Edit `/etc/default/grub` and append `bpf` to `GRUB_CMDLINE_LINUX_DEFAULT`, keeping all existing entries:

```
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash lsm=landlock,lockdown,yama,integrity,apparmor,bpf"
```

> [!IMPORTANT]
> Do not set `lsm=bpf` alone — that drops other active security policies. Always append `bpf` to the existing list from the command above.

Apply and reboot:

| Distro | Command                                       |
| ------ | --------------------------------------------- |
| Ubuntu | `sudo update-grub`                            |
| Fedora | `sudo grub2-mkconfig -o /boot/grub2/grub.cfg` |
| Arch   | `sudo grub-mkconfig -o /boot/grub/grub.cfg`   |

```bash
sudo reboot
```
