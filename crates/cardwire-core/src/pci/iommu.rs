use crate::pci::{IommuError, IommuGroup};
use log::error;
use std::{collections::BTreeMap, fs, path::Path};

pub fn read_iommu_groups() -> Result<BTreeMap<usize, IommuGroup>, IommuError> {
    let base_path = Path::new("/sys/kernel/iommu_groups");
    let mut dir_iter = base_path.read_dir().map_err(|e| {
        error!(
            "Failed to read IOMMU groups directory {:?}: {}",
            base_path, e
        );
        IommuError::Io(e)
    })?;

    if dir_iter.next().is_none() {
        return Err(IommuError::IOMMUNotEnabled);
    }

    let mut groups: BTreeMap<usize, IommuGroup> = BTreeMap::new();

    for entry in fs::read_dir(base_path).map_err(|e| {
        error!(
            "Failed to re-read IOMMU groups directory {:?}: {}",
            base_path, e
        );
        IommuError::Io(e)
    })? {
        let entry = entry.map_err(|e| {
            error!("Failed to read IOMMU group entry: {}", e);
            IommuError::Io(e)
        })?;
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
    let devices_iter = fs::read_dir(&devices_dir).map_err(|e| {
        error!(
            "Failed to read IOMMU group devices {:?}: {}",
            devices_dir, e
        );
        IommuError::Io(e)
    })?;

    for device_entry in devices_iter {
        let device_entry = device_entry.map_err(|e| {
            error!(
                "Failed to read device entry in IOMMU group {:?}: {}",
                devices_dir, e
            );
            IommuError::Io(e)
        })?;
        let Ok(name_str) = device_entry.file_name().into_string() else {
            continue;
        };
        if name_str.starts_with("0000:") {
            devices.push(name_str);
        }
    }

    Ok(devices)
}

pub fn is_iommu_enabled() -> bool {
    match Path::new("/sys/kernel/iommu_groups").read_dir() {
        Ok(mut iommu_folder) => iommu_folder.next().is_some(),
        Err(_) => false,
    }
}
