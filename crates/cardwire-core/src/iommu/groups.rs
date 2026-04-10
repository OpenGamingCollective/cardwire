use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::errors::IommuError;

pub struct IommuGroup {
    pub id: usize,
    pub devices: Vec<String>,
}

pub fn read_iommu_groups() -> Result<BTreeMap<usize, IommuGroup>, IommuError> {
    let base_path = Path::new("/sys/kernel/iommu_groups");
    if !base_path.exists() {
        return Err(IommuError::NotEnabled);
    }

    let mut groups: BTreeMap<usize, IommuGroup> = BTreeMap::new();

    for entry in fs::read_dir(base_path)? {
        let entry = entry?;
        let group_dir = entry.path();
        let Some(group_id_str) = group_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(group_id) = group_id_str.parse::<usize>() else {
            continue;
        };

        let devices = read_group_devices(&group_dir)?;
        groups.insert(
            group_id,
            IommuGroup {
                id: group_id,
                devices,
            },
        );
    }

    Ok(groups)
}

fn read_group_devices(group_dir: &Path) -> Result<Vec<String>, IommuError> {
    let devices_dir = group_dir.join("devices");
    if !devices_dir.exists() {
        return Err(IommuError::MissingDevicesDir(group_dir.to_path_buf()));
    }

    let mut devices = Vec::new();
    for device_entry in fs::read_dir(devices_dir)? {
        let device_entry = device_entry?;
        let Ok(name_str) = device_entry.file_name().into_string() else {
            continue;
        };
        if name_str.starts_with("0000:") {
            devices.push(name_str);
        }
    }

    Ok(devices)
}
