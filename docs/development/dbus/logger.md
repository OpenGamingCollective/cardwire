# Logger

`org.opengamingcollective.cardwire.Logger`

Served at the root object path `/org/opengamingcollective/cardwire`.

**Methods:**

- **`ProcessBlocked`**
  Get the recent blocked GPU access attempts
  - **Inputs:** None
  - **Outputs:**
    - (out): `a(tusus)` -- An array of `LogEntry` structs, where timestamp is in seconds:
      - `timestamp`: `t` - Unix timestamp in seconds
      - `pid`: `u` - Process id
      - `comm`: `s` - Process name
      - `gpu_id`: `u` - The blocked GPU id
      - `wayland_app_id`: `s` - The Wayland app id (empty string if unknown)

**Signals:**

- **`ProcessBlockedChanged`**
  Emitted when a new blocked GPU access attempt is logged
  - **Parameters:** `(tusus)` -- A single `LogEntry` struct
