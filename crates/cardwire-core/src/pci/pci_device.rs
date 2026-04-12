use crate::pci::{IommuError, PciDevice, is_iommu_enabled, read_iommu_groups};
use log::{error, info, warn};
use std::{collections::HashMap, fs, fs::File, io, io::BufRead, path::Path};

pub fn read_pci_devices() -> Result<HashMap<String, PciDevice>, IommuError> {
    match is_iommu_enabled() {
        true => {
            info!("IOMMU detected, reading pci devices using iommu dir");
            read_pci_devices_using_iommu()
        }
        false => {
            info!("IOMMU not detected, reading pci devices using sysfs dir");
            read_pci_devices_using_sysfs()
        }
    }
}

fn read_pci_devices_using_iommu() -> Result<HashMap<String, PciDevice>, IommuError> {
    let iommu_groups = read_iommu_groups()?;
    let pci_names = load_pci_name_db(Path::new("/usr/share/hwdata/pci.ids")).unwrap_or_else(|e| {
        warn!("Failed to load PCI name DB: {}", e);
        PciNameDb::default()
    });
    let mut devices_map = HashMap::new();
    for (group_id, group) in iommu_groups {
        // read "device" folder, look at each PCI ADDRESS
        for pci_address in group.devices {
            let vendor_id = get_vendor_id(&pci_address);
            let device_id = get_device_id(&pci_address);

            let vendor_key = vendor_id.as_deref().map(normalize_device_id);
            let device_key = device_id.as_deref().map(normalize_device_id);

            let vendor_name = vendor_key
                .as_ref()
                .and_then(|k| pci_names.vendors.get(k))
                .cloned();

            let device_name = vendor_key
                .as_ref()
                .zip(device_key.as_ref())
                .and_then(|(v, d)| pci_names.devices.get(&(v.clone(), d.clone())))
                .cloned();

            let device = PciDevice {
                pci_address: pci_address.clone(),
                iommu_group: Some(group_id),
                vendor_id,
                device_id,
                vendor_name,
                device_name,
                driver: get_driver(&pci_address),
                class: get_class(&pci_address),
            };
            devices_map.insert(pci_address, device);
        }
    }
    Ok(devices_map)
}
fn read_pci_devices_using_sysfs() -> Result<HashMap<String, PciDevice>, IommuError> {
    let sysfs = Path::new("/sys/bus/pci/devices");
    let pci_names = load_pci_name_db(Path::new("/usr/share/hwdata/pci.ids")).unwrap_or_else(|e| {
        warn!("Failed to load PCI name DB: {}", e);
        PciNameDb::default()
    });
    let mut devices_map = HashMap::new();
    let sysfs_dir = fs::read_dir(sysfs).map_err(|e| {
        error!("Failed to read sysfs PCI devices at {:?}: {}", sysfs, e);
        IommuError::Io(e)
    })?;
    for folder in sysfs_dir.flatten() {
        let file_name = folder.file_name();
        let name = file_name
            .to_str()
            .ok_or("File name contains invalid UTF-8")?;
        let vendor_id = get_vendor_id(name);
        let device_id = get_device_id(name);

        let vendor_key = vendor_id.as_deref().map(normalize_device_id);
        let device_key = device_id.as_deref().map(normalize_device_id);

        let vendor_name = vendor_key
            .as_ref()
            .and_then(|k| pci_names.vendors.get(k))
            .cloned();

        let device_name = vendor_key
            .as_ref()
            .zip(device_key.as_ref())
            .and_then(|(v, d)| pci_names.devices.get(&(v.clone(), d.clone())))
            .cloned();

        let device = PciDevice {
            pci_address: name.to_string(),
            iommu_group: None,
            vendor_id,
            device_id,
            vendor_name,
            device_name,
            driver: get_driver(name),
            class: get_class(name),
        };
        devices_map.insert(name.to_string(), device);
    }
    Ok(devices_map)
}
fn get_vendor_id(pci_address: &str) -> Option<String> {
    read_sysfs_trim(
        Path::new("/sys/bus/pci/devices")
            .join(pci_address)
            .join("vendor"),
    )
    .ok()
}

fn get_device_id(pci_address: &str) -> Option<String> {
    read_sysfs_trim(
        Path::new("/sys/bus/pci/devices")
            .join(pci_address)
            .join("device"),
    )
    .ok()
}

fn get_class(pci_address: &str) -> Option<String> {
    read_sysfs_trim(
        Path::new("/sys/bus/pci/devices")
            .join(pci_address)
            .join("class"),
    )
    .ok()
}

fn get_driver(pci_address: &str) -> Option<String> {
    let driver_path = match fs::canonicalize(
        Path::new("/sys/bus/pci/devices")
            .join(pci_address)
            .join("driver"),
    ) {
        Ok(driver_path) => driver_path,
        Err(_) => return None,
    };
    driver_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn read_sysfs_trim(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path).map(|content| content.trim_end().to_string())
}
#[derive(Default)]
struct PciNameDb {
    vendors: HashMap<String, String>,
    devices: HashMap<(String, String), String>,
}

fn load_pci_name_db(path: &Path) -> io::Result<PciNameDb> {
    if !path.exists() {
        return Ok(PciNameDb::default());
    }

    let file = File::open(path)?;
    let reader = io::BufReader::new(file);

    let mut db = PciNameDb::default();
    let mut current_vendor: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if !line.starts_with('\t') {
            current_vendor = parse_pci_ids_line(&line).map(|(vendor_id, vendor_name)| {
                db.vendors.insert(vendor_id.clone(), vendor_name);
                vendor_id
            });
            continue;
        }

        if !line.starts_with("\t\t")
            && let (Some(vendor_id), Some((device_id, device_name))) = (
                current_vendor.as_ref(),
                parse_pci_ids_line(line.trim_start_matches('\t')),
            )
        {
            db.devices
                .insert((vendor_id.clone(), device_id), device_name);
        }
    }

    Ok(db)
}

fn parse_pci_ids_line(line: &str) -> Option<(String, String)> {
    let mut split = line.split_whitespace();
    let raw_id = split.next()?;
    if raw_id.len() != 4 || !raw_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    let name = split.collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return None;
    }

    Some((raw_id.to_ascii_lowercase(), name))
}

fn normalize_device_id(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .to_ascii_lowercase()
}
