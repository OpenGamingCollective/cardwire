# Cardwire Profiler

The goal of the profiler is to allow or block an app on the fly

The profiler will uses a database, and both dynamic and static analysis to determine if an app should be allowed or not.

Database is for known entities, the result of the static analysis should be stored in this database

dynamic result will never be stored

if static return blocked but dynamic return allow, app should be allowed

## Dynamic

### Gamemode
If the game uses gamemoderun

Example with Persona 4 Golden launched with gamemoderun:

```bash
❯ grep -i gamemode /proc/24710/maps
76631d641000-76631d642000 r--p 00000000 00:25 98239241                   /nix/store/mi8lmzjfkmcmiwhsir8z8v4fyihi1mwf-gamemode-1.8.2-lib/lib/libgamemodeauto.so.0.0.0
76631d642000-76631d644000 r-xp 00001000 00:25 98239241                   /nix/store/mi8lmzjfkmcmiwhsir8z8v4fyihi1mwf-gamemode-1.8.2-lib/lib/libgamemodeauto.so.0.0.0
76631d644000-76631d645000 r--p 00003000 00:25 98239241                   /nix/store/mi8lmzjfkmcmiwhsir8z8v4fyihi1mwf-gamemode-1.8.2-lib/lib/libgamemodeauto.so.0.0.0
76631d645000-76631d646000 r--p 00003000 00:25 98239241                   /nix/store/mi8lmzjfkmcmiwhsir8z8v4fyihi1mwf-gamemode-1.8.2-lib/lib/libgamemodeauto.so.0.0.0
76631d646000-76631d647000 rw-p 00004000 00:25 98239241                   /nix/store/mi8lmzjfkmcmiwhsir8z8v4fyihi1mwf-gamemode-1.8.2-lib/lib/libgamemodeauto.so.0.0.0
```
Allow

### Electron
If the daemon detects an app is using electron

check via /proc/PID/cmdline ?

Block

## Static

### XDG
XDG-dir if category = game or run on dgpu = true

Allow

