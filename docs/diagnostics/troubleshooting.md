# Troubleshooting

## Name is not activable

Is the daemon running?

```bash
systemctl status cardwired.service
```

> If it's not running, enable the daemon with `systemctl enable cardwired.service` and reboot your device.

## dGPU is detected as the default gpu

### On ROG laptop

is the asus MUX enabled?

```bash
asusctl armoury list
```

then find

```bash
gpu_mux_mode:
  current: [(0),1]
```

> 0 means that the MUX is disabled, the dGPU **IS** the default GPU in this case

To enable it:

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

## nvidia-powerd failure after switching modes

When switching to integrated mode on NVIDIA hardware, you may see errors or failures related to the `nvidia-powerd` service. This is a known quirk caused by the GPU entering `D3Cold` (a deep sleep state) which prevents `nvidia-powerd` from communicating with it.

Since v0.12.0, cardwired restarts `nvidia-powerd` automatically after every mode change (only when the service is enabled), so this is usually fixed without any action. If the problem persists, restart it manually:

```bash
sudo systemctl restart nvidia-powerd.service
```

> [!NOTE]
> This was fixed in cardwire 0.12.1, cardwired now stop and start nvidia-powerd on mode switch instead of a naive restart
