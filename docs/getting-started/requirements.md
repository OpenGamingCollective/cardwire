# Requirements

Requirements to be able to run Cardwire:

## Kernel

Version 5.7 or later

```bash
uname -r
```

Built with `CONFIG_BPF_LSM` enabled

On e.g. Ubuntu/Fedora:
```bash
grep CONFIG_BPF_LSM /boot/config-$(uname -r)
```

On other distros possibly:
```bash
zcat /proc/config.gz | grep CONFIG_BPF_LSM
```

> returns `CONFIG_BPF_LSM=y` if it's enabled
