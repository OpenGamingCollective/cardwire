# Requirements

Requirements to be able to run Cardwire:

## Kernel

Version 5.7 or later

```bash
uname -r
```

Built with `CONFIG_BPF_LSM` enabled
```bash
zcat /proc/config.gz | grep CONFIG_BPF_LSM
```
> returns `CONFIG_BPF_LSM=y` if it's enabled

Enabled in the boot cmdline
```bash
cat /proc/cmdline
```
> Must contains `lsm=bpf` (If empty/doesnt contain bpf, please read Caution)

> [!CAUTION]
> Most distros already enable bpf, only change the cmdline if cardwired doesnt launch
