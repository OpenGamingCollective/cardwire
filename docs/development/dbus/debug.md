# Debug

`org.opengamingcollective.cardwire.Debug`

Served at the root object path `/org/opengamingcollective/cardwire`.

**Methods:**

- **`GetPciDevices`**
  Get a dictionary of all detected PCI devices.
  - **Inputs:** None
  - **Outputs:**
    - (out): `a{s(sssssssss)}` -- A dictionary mapping PCI addresses to a struct containing:
      - `iommu_group`: `s` - IOMMU group number (empty string if none)
      - `vendor_id`: `s` - PCI vendor ID (empty string if unknown)
      - `device_id`: `s` - PCI device ID (empty string if unknown)
      - `vendor_name`: `s` - Vendor name (empty string if unknown)
      - `device_name`: `s` - Device name (empty string if unknown)
      - `driver`: `s` - Kernel driver in use (empty string if unknown)
      - `class`: `s` - PCI class (empty string if unknown)
      - `parent_pci`: `s` - Parent PCI address (empty string if unknown)
      - `child_pci`: `s` - Child PCI address (empty string if unknown)

- **`RefreshGpu`**
  Refresh the internal GPU list from the system. Performs a full re-enumeration, re-serves the GPU objects and re-applies the current mode. Useful after a hotplug event.
  - **Inputs:** None
  - **Outputs:** None
