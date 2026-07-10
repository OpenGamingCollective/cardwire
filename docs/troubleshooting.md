# Troubleshooting

## Name is not activable/cli not working

Is the daemon running?

```bash
systemctl status cardwired.service
```

> If it's not running, enable the daemon with `systemctl enable cardwired.service` and reboot your device.

## dGPU is detected as the default gpu

### On ROG laptop

Is the ASUS MUX enabled?

```bash
asusctl armoury list
```

then find

```bash
gpu_mux_mode:
  current: [(0),1]
```

> 0 means that the MUX is enabled, the dGPU **IS** the default GPU in this case

To disable it:

```bash
asusctl armoury set gpu_mux_mode 1
```

> A reboot is required for the change to take effect.

### Non ROG Laptop

This shouldn't happen, please create an issue with the output of

```bash
ls /sys/class/drm
```

and

```bash
cat /sys/class/drm/*/status
```
