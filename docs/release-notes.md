# Release Notes

## v0.12.0 (2026-08-09)

Cardwire is a Linux GPU manager that blocks GPU access at the syscall level with eBPF LSM hooks. This is the largest release so far. It ships a redesigned GUI, a fully working smart mode, a rewritten eBPF core and a reworked GPU detection pipeline. Wayland sessions only.

### Highlights

- Redesigned GUI with new pages, tray polish and better state handling ([#171](https://github.com/OpenGamingCollective/cardwire/pull/171))
- New Smart Mode page to browse apps and set per-app policies ([#167](https://github.com/OpenGamingCollective/cardwire/pull/167))
- New Logs page showing blocked GPU access attempts ([#159](https://github.com/OpenGamingCollective/cardwire/pull/159))
- eBPF core rewritten from C to Rust with aya-ebpf ([#137](https://github.com/OpenGamingCollective/cardwire/pull/137))
- GPU enumeration rework with Vulkan, EGL and udev, plus discrete and virtual GPU detection ([#144](https://github.com/OpenGamingCollective/cardwire/pull/144))
- External display auto-switch exposed over D-Bus and the CLI ([#139](https://github.com/OpenGamingCollective/cardwire/pull/139), [#170](https://github.com/OpenGamingCollective/cardwire/pull/170), [#172](https://github.com/OpenGamingCollective/cardwire/pull/172))
- `cardwire launch` command to start apps with the right GPU environment ([#145](https://github.com/OpenGamingCollective/cardwire/pull/145), [#166](https://github.com/OpenGamingCollective/cardwire/pull/166))

### New features

- Smart mode now works end to end
  - Exec tracepoint events drive per-app allow and force decisions ([#137](https://github.com/OpenGamingCollective/cardwire/pull/137))
  - NVIDIA dGPUs are allowed again in smart mode ([#137](https://github.com/OpenGamingCollective/cardwire/pull/137))
  - GPU ids are stored as values inside the BPF maps ([#137](https://github.com/OpenGamingCollective/cardwire/pull/137))
  - SmartPolicy D-Bus interface to request process access and set app policies ([#155](https://github.com/OpenGamingCollective/cardwire/pull/155))
  - PID maps cleaned directly from the eBPF program on exec and exit ([#158](https://github.com/OpenGamingCollective/cardwire/pull/158))
  - Force GPU and blocked-access reporting in manual mode ([#174](https://github.com/OpenGamingCollective/cardwire/pull/174))
- Blocked app logging
  - D-Bus API plus live `ProcessBlockedChanged` signal ([#147](https://github.com/OpenGamingCollective/cardwire/pull/147))
  - Wayland app id resolution and non-desktop process reporting ([#147](https://github.com/OpenGamingCollective/cardwire/pull/147))
  - Comm and GPU id reported directly from the eBPF program ([#151](https://github.com/OpenGamingCollective/cardwire/pull/151))
- Per-app policy engine
  - Internal application list from XDG, Steam and Flatpak ([#162](https://github.com/OpenGamingCollective/cardwire/pull/162))
  - Steam games are discovered automatically and blocked by default until allowed ([#162](https://github.com/OpenGamingCollective/cardwire/pull/162))
  - Live refresh when an app is installed ([#164](https://github.com/OpenGamingCollective/cardwire/pull/164))
  - `NewAppAdded` signal ([#169](https://github.com/OpenGamingCollective/cardwire/pull/169))
- New `Env` GPU property to fetch the launch environment over D-Bus ([#165](https://github.com/OpenGamingCollective/cardwire/pull/165))
  - The CLI launch command uses it instead of the switcheroo shim ([#166](https://github.com/OpenGamingCollective/cardwire/pull/166))
- `CARDWIRE_ALLOW`, `CARDWIRE_FORCE_DGPU` and `CARDWIRE_FORCE_GPU` environment variables to allow or force a GPU for a single launch
- `AvailableModes` API used by both the CLI and the GUI ([#168](https://github.com/OpenGamingCollective/cardwire/pull/168))
- hwmon folder blocking ([#153](https://github.com/OpenGamingCollective/cardwire/pull/153))
- nvidia-powerd restart and DRM uevent on GPU operations ([#173](https://github.com/OpenGamingCollective/cardwire/pull/173))
- Hardened systemd unit and D-Bus name replacement prevention ([#161](https://github.com/OpenGamingCollective/cardwire/pull/161))
- App id renamed to `org.opengamingcollective.cardwire` ([#138](https://github.com/OpenGamingCollective/cardwire/pull/138))
- App store metadata with metainfo and screenshot ([#135](https://github.com/OpenGamingCollective/cardwire/pull/135), [#136](https://github.com/OpenGamingCollective/cardwire/pull/136))
- Default mode changed from Manual to Hybrid at startup ([#146](https://github.com/OpenGamingCollective/cardwire/pull/146))
- GUI settings file `~/.config/cardwire/gui.toml` with `start_in_tray`, `primary_click_action` and `primary_click_modes`

### Fixes

- getdents64 hiding keyed by tid instead of pid to prevent directory corruption ([#154](https://github.com/OpenGamingCollective/cardwire/pull/154))
- Atomic config saves with unique temp files and automatic cleanup ([#154](https://github.com/OpenGamingCollective/cardwire/pull/154))
- Battery auto-switch mode validated before storage ([#154](https://github.com/OpenGamingCollective/cardwire/pull/154))
- Switcheroo `PropertiesChanged` signals emitted correctly ([#154](https://github.com/OpenGamingCollective/cardwire/pull/154))
- Inode resolution moved to blocking threads ([#153](https://github.com/OpenGamingCollective/cardwire/pull/153))
- `refresh_gpu` race conditions and deadlocks ([#144](https://github.com/OpenGamingCollective/cardwire/pull/144), [#163](https://github.com/OpenGamingCollective/cardwire/pull/163))
- `cardwire launch` returns the original exit code of the process
- lsof stability and duplicate comm filtering ([#163](https://github.com/OpenGamingCollective/cardwire/pull/163))
- The GUI keeps its GPU list on daemon errors and listens to GPU hotplug ([#154](https://github.com/OpenGamingCollective/cardwire/pull/154))
- GUI fixes for the log page, power state display and blocked colors ([#171](https://github.com/OpenGamingCollective/cardwire/pull/171))
- The default GPU is no longer reported as blocked in smart mode, and app matching now works for all launched apps ([#171](https://github.com/OpenGamingCollective/cardwire/pull/171))
- getdents64 hiding falls back to a weakened state when the kernel is under lockdown ([#140](https://github.com/OpenGamingCollective/cardwire/pull/140))
- Blocked inode map size bump for future proofing

### Internal

- Daemon restructure with a `DaemonContext` and a `SystemType` enum ([#163](https://github.com/OpenGamingCollective/cardwire/pull/163))
- GPU enumeration pipeline rewritten around Vulkan, EGL and udev ([#144](https://github.com/OpenGamingCollective/cardwire/pull/144))
- nixpkgs bump ([#142](https://github.com/OpenGamingCollective/cardwire/pull/142))
- `PreferNonDefaultGpus` detection dropped ([#160](https://github.com/OpenGamingCollective/cardwire/pull/160))
- CI runs 2 GPU, 3 GPU and 15 GPU VM tests in a matrix ([#174](https://github.com/OpenGamingCollective/cardwire/pull/174))

### Notes for integrators

- New D-Bus interfaces and members
  - `SmartPolicy` with `RequestProcessAccess`, `GetProcessStatus`, `GetAppPolicies`, `SetAppPolicy` and `NewAppAdded`
  - `Logger` with `ProcessBlocked` and `ProcessBlockedChanged`
  - `Env` property on the `Gpu` interface
  - `AvailableModes` method on the `Mode` interface
  - `ExternalDisplayAutoSwitch` property on the `Config` interface
  - Driver field added to `DbusGpuDevice`
- The forced app policy is removed from this release and will come back in a later one ([#162](https://github.com/OpenGamingCollective/cardwire/pull/162))
- Deprecated in favor of the internal per-app policy list: Steam auto-allow in smart mode, the `PrefersNonDefaultGpu` desktop-entry detection and the automatic approval of GPU environment variables (`DRI_PRIME`, `__NV_PRIME_RENDER_OFFLOAD`) ([#160](https://github.com/OpenGamingCollective/cardwire/pull/160), [#162](https://github.com/OpenGamingCollective/cardwire/pull/162))
- Smart mode is laptop-only, `DRI_PRIME` and `__NV_PRIME_RENDER_OFFLOAD` are not analyzer inputs, only `CARDWIRE_ALLOW`, `CARDWIRE_FORCE_DGPU`, `CARDWIRE_FORCE_GPU` and `SteamAppId` are evaluated
- `cardwire debug diagnostic-gpu` remains a dead end on the daemon side
