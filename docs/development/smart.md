# Smart

## Goal and integration

Cardwire owns its per-application policy. It does not depend on desktop environment heuristics like `PrefersNonDefaultGPU` or on `DRI_PRIME`, `__NV_PRIME_RENDER_OFFLOAD` or SteamAppId being present in the app environment. Those auto-approval inputs were dropped in 0.12.0 and replaced by the internal application list, which makes cardwire self-sufficient while staying compatible with desktop environments through the [Switcheroo shim](dbus/switcheroo.md).

Third parties that want to integrate with cardwire get three surfaces:

- **Per-process**: the `CARDWIRE_*` environment variables and `RequestProcessAccess` are equivalent ways to route a process to a GPU. Set the env vars on the process before launch, call `RequestProcessAccess` with the pid right after spawning it, or apply it to a process that is already running (Caution, App often scan for GPUs at launch). Both insert the pid into the same eBPF maps. The per-GPU environment can be fetched from the `Env` property of the [Gpu interface](dbus/gpu.md).
- **Management**: the [SmartPolicy D-Bus interface](dbus/smart-policy.md) lists known applications (`GetAppPolicies`), changes their persistent policy (`SetAppPolicy`) and announces discoveries (`NewAppAdded`).

One caveat applies to both routes: the eBPF program clears both pid maps at every exec, so a process that execs again after being classified starts from a clean slate and is re-evaluated.

## Introduction

Having an integrated and hybrid mode is good, but what if we could have the best of both worlds?

This is what cardwire's smart mode was made for. Cardwire uses a mix of kernel-space + userspace to directly allow processes on the fly

### Kernel-Space

Using the eBPF program and the `tracepoint/sched/sched_process_exec` hooks, the kernel program notifies `cardwired` when a new process is executed, sending its pid using the `CW_EXEC_EVENTS` RING_BUF (in Smart and Manual modes). Once the process is received by `cardwired`, it will be analyzed in real-time and its pid will be inserted into the `CW_ALLOWED_PID` map (value always `0`) or the `CW_FORCED_PID` map (value is the GPU id)

When a process exits, the kernel's `tracepoint/sched/sched_process_exit` removes the pid from both maps directly, preventing the maps from overflowing.

If you want to dive deeper into the kernel code, take a look at [BPF](bpf.md)

### Userspace

The userspace of Smart mode acts as the brain. It is responsible for making the actual decisions about whether a process is allowed to use a GPU. It is divided into three main components:

- **`CardwireAnalyzer`**: A dedicated background task that listens to the `CW_EXEC_EVENTS` ring buffer (and the `CW_REPORT_EVENTS` ring for blocked-access logging). When it receives a new PID from the kernel, it invokes the analysis helpers. If the application passes, it populates the `CW_ALLOWED_PID` map (value always `0`) or the `CW_FORCED_PID` map (value is the GPU id).
- **`dynamic_analysis.rs`**: A set of helper functions used to analyze a process in real-time. By reading `/proc/<pid>/environ` and `/proc/<pid>/cmdline`, it checks for explicitly requested GPUs (like `CARDWIRE_ALLOW=1`, `CARDWIRE_FORCE_DGPU=1`, `CARDWIRE_FORCE_GPU=<gpu_id>`) or implicit signs like Steam games (`SteamAppId`, the `0` and `769` ids are excluded).
- **`static_analysis.rs`**: A set of helper functions that analyze system data when the daemon starts. It scans the XDG data directories and watches them with inotify so new apps are picked up at install time. Every discovered app is blocked by default until the user allows it. The `xdg-desktop-portal` process is always blocked.

#### Notes

Technically, it's a pure race condition between the cardwire analyzer and the process, cardwire scans and allow a process in ~60-100 microseconds, from my testing, no process initialized its render before cardwire allowed it

## Complete Execution Flow

Here is a comprehensive breakdown of how the Kernel and Userspace interact in real-time when an application launches: (Please zoom on it)

```mermaid
sequenceDiagram
    participant Proc as Process
    participant Kernel as eBPF Kernel Hooks
    participant Map as BPF Maps
    participant Daemon as CardwireAnalyzer (Userspace)

    Note over Proc,Daemon: 1. Process Launch
    Proc->>Kernel: sched_process_exec
    Kernel->>Map: Send PID via cw_exec_events (RingBuf)
    Map->Daemon: Listen to cw_exec_events and wait for new events

    Note over Daemon: 2. Real-time Analysis
    Daemon->>Daemon: Read /proc/<pid>/environ & cmdline
    Daemon->>Daemon: Check CARDWIRE_* env vars, Steam, XDG lists, SQLite policies

    alt Is Allowed?
        Daemon->>Map: Insert PID into cw_allowed_pid
    else Is Forced?
        Daemon->>Map: Insert PID into cw_forced_pid with the GPU id
    else Not Allowed
        Daemon->>Daemon: Do nothing
    end

    Note over Proc,Kernel: 3. GPU Access & Directory Listing
    Proc->>Kernel: getdents64 / file_open (/dev/dri/)
    Kernel->>Map: Check cw_allowed_pid and cw_forced_pid

    alt PID not in any map
        Kernel-->>Proc: hide GPU (Return -ENOENT)
        Kernel->>Daemon: Send block event (cw_report_events)
    else PID in cw_allowed_pid
        Kernel-->>Proc: Allow dGPU and iGPU
    else PID in cw_forced_pid (value = GPU id)
        Kernel-->>Proc: Allow the forced GPU, hide the others (-ENOENT)
    end

    Note over Proc,Daemon: 4. Application Exit
    Proc->>Kernel: sched_process_exit
    Kernel->>Kernel: Remove PID from cw_allowed_pid and cw_forced_pid
```

## Application policies

Smart mode is only available on laptops (`SystemType::Laptop`). Per-application policies are stored in the `app_policies` table of the daemon's SQLite database, with two values: `Blocked` and `Allowed`. Known apps are blocked by default until the user allows them, and newly discovered apps are announced through the `NewAppAdded` D-Bus signal.

The policy for a process can be overridden at runtime through the `org.opengamingcollective.cardwire.SmartPolicy` D-Bus interface (`RequestProcessAccess`, `GetProcessStatus`, `GetAppPolicies`, `SetAppPolicy`). Note that `GetProcessStatus` returns an empty string (not `"Default"`) for unclassified processes.

For now there is no plan to adapt the per-application policy for non-laptop systems, unless the demand is present.

Force_GPU can be used on all systems with the Manual mode.
