//! The purpose of this file is to format the received String from daemon into a displayable format
//! for the user

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
// Define the struct here instead of importing from cardwire_core,
// I want cardwire-cli to be independent of the rest of cardwire
// This allow other dev to make their own client for cardwire
// Here the struct are used to parse the json
#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type)]
pub struct GpuDevice {
    id: u32,
    name: String,
    pci: String,
    render: u32,
    card: u32,
    default: Option<bool>,
    blocked: Option<bool>,
    nvidia: bool,
    nvidia_minor: Option<u32>,
}
#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type)]
pub struct PciDevice {
    pci_address: String,
    iommu_group: Option<usize>,
    vendor_id: Option<String>,
    device_id: Option<String>,
    vendor_name: Option<String>,
    device_name: Option<String>,
    driver: Option<String>,
    class: Option<String>,
}
/// turn a json into a string
pub fn parse_json(json: &str) -> String {
    serde_json::from_str(json).unwrap_or("Error parsing json".to_string())
}

/// Take a jsonified String and print it  
pub fn print_devices(gpu_list: BTreeMap<usize, GpuDevice>, is_json: bool) -> Result<()> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&gpu_list)?);
    } else {
        pretty_print_gpu(gpu_list);
    };

    Ok(())
}

pub fn print_devices_pci(pci_list: BTreeMap<String, PciDevice>) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&pci_list)?);
    Ok(())
}

fn pretty_print_gpu(gpu_list: BTreeMap<usize, GpuDevice>) {
    let mut id_w = 2usize;
    let mut name_w = 4usize;
    let mut pci_w = 3usize;
    let mut render_w = 6usize;
    let mut card_w = 4usize;
    let default_w = 7usize;
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
        "{:<id_w$}  {:<name_w$}  {:<pci_w$}  {:<render_w$}  {:<card_w$}  {:<default_w$}  {:<blocked_w$}",
        "ID",
        "NAME",
        "PCI",
        "RENDER",
        "CARD",
        "DEFAULT",
        "BLOCKED",
        id_w = id_w,
        name_w = name_w,
        pci_w = pci_w,
        render_w = render_w,
        card_w = card_w,
        default_w = default_w,
        blocked_w = blocked_w,
    );
    println!(
        "{}  {}  {}  {}  {}  {}  {}",
        "-".repeat(id_w),
        "-".repeat(name_w),
        "-".repeat(pci_w),
        "-".repeat(render_w),
        "-".repeat(card_w),
        "-".repeat(default_w),
        "-".repeat(blocked_w),
    );
    for (_, gpu) in gpu_list {
        let render_full = format!("renderD{}", gpu.render);
        let card_full = format!("card{}", gpu.card);
        println!(
            "{:<id_w$}  {:<name_w$}  {:<pci_w$}  {:<render_w$}  {:<card_w$}  {:<default_w$}  {:<blocked_w$}",
            gpu.id,
            gpu.name,
            gpu.pci,
            render_full,
            card_full,
            gpu.default.unwrap(),
            if gpu.blocked.unwrap() { "on" } else { "off" },
            id_w = id_w,
            name_w = name_w,
            pci_w = pci_w,
            render_w = render_w,
            card_w = card_w,
            default_w = default_w,
            blocked_w = blocked_w,
        );
    }
}
