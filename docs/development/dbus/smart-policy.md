# SmartPolicy

`org.opengamingcollective.cardwire.SmartPolicy`

Served at the root object path `/org/opengamingcollective/cardwire`. This is the per-application policy API of [Smart Mode](../smart.md).

**Methods:**

- **`RequestProcessAccess`**
  Request a policy for a process
  - **Inputs:**
    - `pid`: `u` - The process id
    - `policy`: `s` - One of `"Default"`, `"Allow_dGPU"`, `"Force_dGPU"`, `"Force_GPU"`
    - `value`: `u` - GPU id (used by `Force_GPU`)
  - **Outputs:** None
  - **Notes:**
    - The process must already exist (`/proc/<pid>`). It can be called right after the process is spawned or on a running process, this makes it equivalent to the `CARDWIRE_*` environment variables for launch-time routing.
    - `"Default"` is a no-op. `"Allow_dGPU"` is equivalent to `CARDWIRE_ALLOW=1`, `"Force_dGPU"` to `CARDWIRE_FORCE_DGPU=<value>` and `"Force_GPU"` to `CARDWIRE_FORCE_GPU=<value>`

- **`GetProcessStatus`**
  Get the current policy for a process
  - **Inputs:**
    - `pid`: `u` - The process id
  - **Outputs:**
    - (out): `(sau)` -- `("Allowed" | "Forced" | "")` and the optional forced GPU id. An empty string means the process is unclassified

- **`GetAppPolicies`**
  Get all known applications and their policies
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{s(sasasu)}` -- A dictionary mapping app ids to `DbusAppMetadata` structs

- **`SetAppPolicy`**
  Set the policy for a known application
  - **Inputs:**
    - `app_id`: `s` - The application id
    - `policy`: `i` - `0` Blocked, `1` Allowed
  - **Outputs:** None

**Signals:**

- **`NewAppAdded`**
  Emitted when a new application is discovered
  - **Parameters:** `(s(sasasu))` -- The app id and its `DbusAppMetadata`
