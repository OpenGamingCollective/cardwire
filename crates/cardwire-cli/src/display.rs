//! The purpose of this file is to format the received String from daemon into a displayable format
//! for the user

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
// Define the struct here instead of importing from cardwire_core,
// I want cardwire-cli to be independent of the rest of cardwire
// This allow other dev to make their own client for cardwire
// Here the struct are used to parse the json
#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug)]
pub struct GpuDevice {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub discrete: bool,
    pub virtual_gpu: bool,
    pub available: bool,
    pub vendor: String,
    pub driver: String,
    pub blocked: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}
#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type)]
pub struct PciDevice {
    iommu_group: String,
    vendor_id: String,
    device_id: String,
    vendor_name: String,
    device_name: String,
    driver: String,
    class: String,
    parent_pci: String,
    child_pci: String,
}

/// Take a Map and print it
pub fn print_devices(gpu_list: BTreeMap<usize, GpuDevice>, is_json: bool) -> Result<()> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&gpu_list)?);
    } else {
        pretty_print_gpu(gpu_list);
    };

    Ok(())
}
/// Take a Map and print it
pub fn print_devices_pci(pci_list: BTreeMap<String, PciDevice>) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&pci_list)?);
    Ok(())
}
/// Take a Map and print it into a good looking table
fn pretty_print_gpu(gpu_list: BTreeMap<usize, GpuDevice>) {
    let mut id_w = 2usize;
    let mut name_w = 4usize;
    let mut pci_w = 3usize;
    let mut render_w = 6usize;
    let mut card_w = 4usize;
    let default_w = 7usize;
    let discrete_w = 7usize;
    let blocked_w = 7usize;

    // Calculate widths
    for (id, gpu) in &gpu_list {
        id_w = id_w.max(*id);
        name_w = name_w.max(gpu.name.len());
        pci_w = pci_w.max(gpu.pci.len());
        // Full render string is "renderD" + device number
        let render_full = format!("renderD{}", gpu.render);
        render_w = render_w.max(render_full.len());
        let card_full = format!("card{}", gpu.card);
        card_w = card_w.max(card_full.len());
    }

    // Header
    println!(
        "{:<id_w$}  {:<name_w$}  {:<pci_w$}  {:<render_w$}  {:<card_w$}  {:<default_w$} {:<discrete_w$}  {:<blocked_w$}",
        "ID",
        "NAME",
        "PCI",
        "RENDER",
        "CARD",
        "DEFAULT",
        "DISCRETE",
        "BLOCKED",
        id_w = id_w,
        name_w = name_w,
        pci_w = pci_w,
        render_w = render_w,
        card_w = card_w,
        default_w = default_w,
        discrete_w = discrete_w,
        blocked_w = blocked_w,
    );
    println!(
        "{}  {}  {}  {}  {}  {}  {}  {}",
        "-".repeat(id_w),
        "-".repeat(name_w),
        "-".repeat(pci_w),
        "-".repeat(render_w),
        "-".repeat(card_w),
        "-".repeat(default_w),
        "-".repeat(discrete_w),
        "-".repeat(blocked_w),
    );
    for (id, gpu) in gpu_list {
        let render_full = format!("renderD{}", gpu.render);
        let card_full = format!("card{}", gpu.card);
        println!(
            "{:<id_w$}  {:<name_w$}  {:<pci_w$}  {:<render_w$}  {:<card_w$}  {:<default_w$}  {:<discrete_w$}  {:<blocked_w$}",
            id,
            gpu.name,
            gpu.pci,
            render_full,
            card_full,
            if gpu.default { "(*)" } else { "( )" },
            if gpu.discrete { "(*)" } else { "( )" },
            gpu.blocked,
            id_w = id_w,
            name_w = name_w,
            pci_w = pci_w,
            render_w = render_w,
            card_w = card_w,
            default_w = default_w,
            discrete_w = discrete_w,
            blocked_w = blocked_w,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[allow(clippy::too_many_arguments)]
    fn make_gpu(
        id: u32,
        name: &str,
        pci: &str,
        default: bool,
        discrete: bool,
        virtual_gpu: bool,
        available: bool,
        vendor: &str,
        driver: &str,
        blocked: bool,
    ) -> GpuDevice {
        GpuDevice {
            id,
            name: name.to_string(),
            pci: pci.to_string(),
            render: 128,
            card: 0,
            default,
            discrete,
            virtual_gpu,
            available,
            vendor: vendor.to_string(),
            driver: driver.to_string(),
            blocked,
            nvidia: false,
            nvidia_minor: String::new(),
        }
    }

    #[test]
    fn test_print_devices_json_produces_valid_json() {
        let mut map = BTreeMap::new();
        map.insert(
            0,
            make_gpu(
                0,
                "Intel UHD",
                "0000:00:02.0",
                true,
                false,
                false,
                true,
                "Intel",
                "xe",
                false,
            ),
        );
        map.insert(
            1,
            make_gpu(
                1,
                "RTX 4060",
                "0000:01:00.0",
                false,
                true,
                false,
                true,
                "Nvidia",
                "nouveau",
                true,
            ),
        );

        let json_str = serde_json::to_string_pretty(&map).unwrap();
        // Verify it parses back
        let parsed: BTreeMap<usize, GpuDevice> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[&0].name, "Intel UHD");
        assert!(parsed[&1].blocked);
    }

    #[test]
    fn test_print_devices_json_empty_map() {
        let map: BTreeMap<usize, GpuDevice> = BTreeMap::new();
        let json_str = serde_json::to_string_pretty(&map).unwrap();
        assert_eq!(json_str, "{}");
    }

    #[test]
    fn test_gpu_device_fields_roundtrip_through_serde() {
        let gpu = make_gpu(
            42,
            "RX 7900 XTX",
            "0000:03:00.0",
            false,
            true,
            false,
            true,
            "AMD",
            "amdgpu",
            false,
        );
        let json = serde_json::to_string(&gpu).unwrap();
        let parsed: GpuDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 42);
        assert_eq!(parsed.name, "RX 7900 XTX");
        assert_eq!(parsed.pci, "0000:03:00.0");
        assert_eq!(parsed.render, 128);
        assert_eq!(parsed.card, 0);
        assert!(!parsed.default);
        assert!(!parsed.blocked);
        assert!(!parsed.nvidia);
    }
}
