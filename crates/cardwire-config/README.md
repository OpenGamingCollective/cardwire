# Cardwire Config

The crate that handle all files written by Cardwire

## Includes:

- `/etc/cardwire.toml`
- `/var/lib/cardwire/mode.json`
- `/var/lib/cardwire/gpu_state.json`
- `/var/lib/cardwire/cardwire.db`

## TODO:

- Cardwire user config (GUI + daemon)

### /etc/cardwire.toml


```toml
[global_settings]
restore_gpu_states = true
switch_on_display = false
  
[global_settings.battery_switch]
enabled = false
ac_mode = "hybrid"
bat_mode = "integrated"

[experimental_features]
advanced_nvidia_blocking = true
fake_drm_uevent = false

[ebpf]
enabled = true

[switcheroo_settings]
enabled = true
cardwire_envs = true
switcheroo_envs = true

[internal_whitelist]
packages_managers = true
vfio = true
systemd = true
nvidia_powerd = true
```