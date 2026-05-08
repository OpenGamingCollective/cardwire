# Sleep

# How to diagnose a dGPU that won't sleep

Your NVIDIA dGPU won't sleep? Here's how to find and fix the issue.

## Check your NVIDIA GPU power information

Before this, please set cardwire to unblock your dGPU.

Replace the PCI with yours.

```bash
cat /proc/driver/nvidia/gpus/0000:01:00.0/power

Runtime D3 status:          Enabled (fine-grained)
Video Memory:               Off

GPU Hardware Support:
 Video Memory Self Refresh: Supported
 Video Memory Off:          Supported

S0ix Power Management:
 Platform Support:          Supported
 Status:                    Enabled

Notebook Dynamic Boost:     Not Supported
```

The most important section should be `Runtime D3 status`.

If Runtime D3 status is disabled, your GPU will never sleep.

To enable it, follow this method (only tested on Arch; please adapt it for other distros):

>[!CAUTION]
> If you lack the knowledge, or you fear you will break your system, you can always make a post on the Discord to get assistance.

Go to [https://gitlab.com/asus-linux/nvidia-laptop-power-cfg](https://gitlab.com/asus-linux/nvidia-laptop-power-cfg).

We will need two files:
* nvidia.rules
* nvidia.conf

You will need to copy them to their respective directory:
For nvidia.conf:
```bash
/etc/modprobe.d/nvidia.conf
```
For nvidia.rules:
```bash
/usr/lib/udev/rules.d/80-nvidia-pm.rules
```

Once it's done, execute:
```bash
sudo mkinitcpio -P
```
and restart your computer.

### RTX 2000 Series

If it's not working and you own an RTX 2000 GPU, it's a known issue.
You must use driver 580 and add `NVreg_EnableGpuFirmware=0` to `/etc/modprobe.d/nvidia.conf`.

## Check the PCI control value