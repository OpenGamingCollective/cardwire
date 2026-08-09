# DBus Interfaces

Cardwire exposes several D-Bus interfaces on the system bus, one page per interface in this section.

## Service

- **Bus Name:** `org.opengamingcollective.cardwire`

> [!NOTE]
> Cardwire also implements the SwitcherooControl interface for desktop environment integration. See [switcheroo.md](dbus/switcheroo.md) for details.

## Object Path

`/org/opengamingcollective/cardwire`

GPU objects live at `/org/opengamingcollective/cardwire/Gpu/{id}` and are exposed through the standard `org.freedesktop.DBus.ObjectManager` interface at the root path. Watch `InterfacesAdded` and `InterfacesRemoved` to track GPU hotplug.

## Interfaces

- [Manager](dbus/manager.md) -- daemon liveness probe
- [Mode](dbus/mode.md) -- mode switching and available modes
- [Config](dbus/config.md) -- daemon settings
- [Gpu](dbus/gpu.md) -- per-GPU state, device info, environment, power state
- [Logger](dbus/logger.md) -- blocked GPU access attempts
- [SmartPolicy](dbus/smart-policy.md) -- per-application GPU policies
- [Debug](dbus/debug.md) -- PCI devices and GPU list refresh
- [Switcheroo Control](dbus/switcheroo.md) -- compatibility shim for desktop environments

## Notes

- `Option<T>` serializes as `a<T>` (empty or one-element array), not as a variant. This comes from the `option-as-array` zbus feature
