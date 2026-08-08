# Cardwire Analyzer

The goal of the analyzer is to allow or block an app on the fly.

The analyzer combines a database, and both dynamic and static analysis to determine
if an app should be allowed or not.

The database stores known entities (apps discovered via static analysis) and their
policy. Dynamic results are never stored.

## Modules

- `models.rs` — the `CardwireAnalyzer` runtime: eBPF ring buffer consumers, process
  evaluation (`evaluate_app`) and app discovery (`discover_app`), blocked-event
  reporting.
- `dynamic_analysis.rs` — runtime checks: `CARDWIRE_*` environment parsing, GPU env
  detection, Steam app id detection, wayland app id lookup.
- `static_analysis.rs` — FDO desktop entry scanning, builds the `AppMetadata` map.
- `helpers.rs` — generic proc/cmdline helpers shared by the runtime: real process
  name parsing (wine/proton, java, flatpak, steam), kernel comm decoding, proc
  checks

## Evaluation order

When a process exec is reported by eBPF, `evaluate_app` runs:

1. `CARDWIRE_ALLOW=1` - allow
2. `CARDWIRE_FORCE_DGPU=value` - force dGPU
3. `CARDWIRE_FORCE_GPU=value` - force the given GPU
4. `DRI_PRIME=1` / `__NV_PRIME_RENDER_OFFLOAD=1` - allow
5. Database lookup by app name (or `steam_app_<id>` when `SteamAppId` is set):
   - `Blocked` - block
   - `Allowed` - allow
   - `Forced`  force
6. XDG list lookup - app is new: persist it to the database (blocked by default),
   then block
7. Steam fallback - unknown `steam_app_<id>`: persist and block

If static says blocked but dynamic says allow, the app is allowed.
