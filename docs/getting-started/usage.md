# Usage

## Querying GPUs

To have cardwire list all detected GPUs, use:

```
cardwire list
```

For each detected GPU, the command will return:
- An identifier (`ID`). These are used for manual blocking and unblocking. 
- The GPU's name (`NAME`)
- The GPU's PCI address (`PCI`) 
- The associated render node (`RENDER`)
- The associated device node (`CARD`) 
- Whether the GPU has been identified as the default GPU (`DEFAULT`). Default GPUs will remain available when cardwire is set to integrated.  
- Whether the GPU is currently blocked (`BLOCKED`) 

## Mode switching 

GPU modes can be switched using the `cardwire set` command. 

To have cardwire block all GPUs except the default GPU, use: 

```
cardwire set integrated
``` 

To have cardwire allow access to all GPUs, use 

```
cardwire set hybrid 
``` 

## Manual blocking and unblocking 


> [!IMPORTANT] 
> To prevent system breakage, cardwire will not block default GPUs, even when explicitly instructed to do so.

If more granular control over several GPUs is required, cardwire also allows manually blocking individual GPUs by ID. To do so, it needs to be set to manual mode: 

```
cardwire set manual 
``` 

Once set to manual, GPU states can then be set by ID; to find the correct ID, see [Querying GPUs](#Querying-GPUs). 

To block the GPU with ID `1`: 

```
cardwire gpu 1 --block
```

To unblock: 

``` 
cardwire gpu 1 --unblock
``` 

## Battery Auto Switch Mode

Cardwire can automatically switch GPU modes when the system switches between battery and AC power. When `battery_auto_switch` is enabled, cardwire switches to integrated mode on battery and back to a configurable mode when on AC power.

The mode cardwire switches to on AC power is controlled by `battery_auto_switch_mode`. This can be set to `integrated`, `hybrid`, `manual`, or `smart`.

To get the current battery auto switch mode:

```
cardwire config battery-auto-switch-mode
```

To set the battery auto switch mode:

```
cardwire config battery-auto-switch-mode --set hybrid
```

### Smart Mode

Smart mode blocks the dedicated GPU by default like integrated mode, but uses eBPF to analyze each application at launch and selectively allow GPU access for approved apps. It checks `CARDWIRE_ALLOW` env var (highest priority), Steam games (`SteamAppId=`), gamemode (`libgamemodeauto.so`), Flatpak apps with XDG `PrefersNonDefaultGpu=true`, and explicit GPU env vars (`DRI_PRIME=1`, `__NV_PRIME_RENDER_OFFLOAD=1`).
