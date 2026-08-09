# Requirements

To run Cardwire, your system needs to meet a few core requirements. **Good news: if you are using a modern Linux distribution, you likely already meet all of these out of the box!**

## 1. Supported Distributions (Kernel & System)

Cardwire requires:
- **Linux Kernel 5.8 or later** (with `CONFIG_BPF_LSM` enabled).
- **systemd** as the init system.

> [!TIP]
> The following distributions are known to work out of the box with zero manual configuration required:
> - **OGC Distros (Officially Supported)**: Bazzite, Ultramarine, Nobara, PikaOS, ChimeraOS, winesapOS
> - **NixOS (Officially Supported)**
> - Arch Linux / CachyOS
> - Fedora (and Atomic variants)
> - Debian

If you are using one of these distributions, you can safely skip the advanced verification below and head straight to the [Installation Guide](installation.md).

> [!WARNING]
> Non-systemd distros are currently not supported. If you want to use Cardwire on a non-systemd distro, either open a PR with patches or configure the required services on your setup.

## 2. Display Server

> [!CAUTION]
> Cardwire only supports **Wayland**. X11 is unsupported.

---

## Advanced: Manual Kernel Verification

If you are not using a distribution listed above, or if you are compiling your own kernel, you will need to manually verify that eBPF LSM is enabled.

### 1. Verify `CONFIG_BPF_LSM` is enabled

On e.g. Ubuntu/Fedora:

```bash
grep CONFIG_BPF_LSM /boot/config-$(uname -r)
```

On other distros possibly:

```bash
zcat /proc/config.gz | grep CONFIG_BPF_LSM
```

> Returns `CONFIG_BPF_LSM=y` if it's enabled.

### 2. Verify BPF is in the boot cmdline

Check your current boot parameters:

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

> Outputs e.g. `lsm=landlock,yama,apparmor,bpf` or `CONFIG_LSM="landlock,lockdown,yama,integrity,apparmor,bpf"`.
> If it contains 'bpf', bpf is already enabled and usable in your system!

### 3. Verify BPF LSM is active at runtime

The cardwire daemon refuses to start without this. Check the list of active LSMs:

```bash
cat /sys/kernel/security/lsm
```

> Must contain `bpf`. If it does, eBPF LSM is ready even if the boot cmdline looks different.

### Enabling BPF LSM (with GRUB)

If `bpf` is not in your boot cmdline, edit `/etc/default/grub` and append `bpf` to `GRUB_CMDLINE_LINUX_DEFAULT`, keeping all existing entries:

```bash
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash lsm=landlock,lockdown,yama,integrity,apparmor,bpf"
```

> [!IMPORTANT]
> Do not set `lsm=bpf` alone, that drops other active security policies. Always append `bpf` to the existing list from the command above.

Apply and reboot:

| Distro | Command                                       |
| ------ | --------------------------------------------- |
| Ubuntu | `sudo update-grub`                            |
| Fedora | `sudo grub2-mkconfig -o /boot/grub2/grub.cfg` |
| Arch   | `sudo grub-mkconfig -o /boot/grub/grub.cfg`   |

```bash
sudo reboot
```
