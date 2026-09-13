use crate::core::gpu::models::GpuType;

use std::{fs, path::Path, thread, time::Duration};

use log::{error, info, warn};
use nvml_wrapper::{
    Nvml, enum_wrappers::device::{Brand, GpuVirtualizationMode}, enums::device::DeviceArchitecture, error::NvmlError
};
use tokio::{process::Command, time::timeout};

#[allow(unused, dead_code)]
pub fn nvidia_get_device_type(pci_id: &str, gpu_name: &str) -> GpuType {
    GpuType::Unknown
}

/// Get nvidia minor id
pub fn nvidia_get_device_minor(pci_address: &str) -> Option<u32> {
    let nvidia_driver_proc = Path::new("/proc/driver/nvidia/gpus/")
        .join(pci_address)
        .join("information");
    let information = fs::read_to_string(nvidia_driver_proc).ok()?;
    information
        .lines()
        .find(|line| line.starts_with("Device Minor:"))?
        .split_once(':')?
        .1
        .trim()
        .parse::<u32>()
        .ok()
}

/// find the nvidia model using the device information file
pub fn nvidia_get_device_name(pci_address: &str) -> Option<String> {
    let nvidia_driver_proc = Path::new("/proc/driver/nvidia/gpus/")
        .join(pci_address)
        .join("information");
    let information = fs::read_to_string(nvidia_driver_proc).ok()?;
    let model = information
        .lines()
        .find(|line| line.starts_with("Model:"))?
        .split_once(':')?
        .1
        .trim()
        .to_string();
    match !model.is_empty() {
        true => Some(model),
        false => None,
    }
}

const SERVICE: &str = "nvidia-powerd.service";

/// run a systemctl command against the nvidia-powerd service and log the result
async fn run_systemctl(action: &str, extra_args: &[&str]) {
    let output_cmd = Command::new("systemctl")
        .arg(action)
        .arg(SERVICE)
        .args(extra_args)
        .kill_on_drop(true)
        .output();

    let output = match timeout(Duration::from_secs(10), output_cmd).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            error!("error while trying to {action} nvidia-powerd: {err}");
            return;
        }
        Err(_) => {
            error!("timed out after 10s while trying to {action} nvidia-powerd");
            return;
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        info!("successfully sent {action} on nvidia-powerd.service");
    } else {
        let code = output.status.code();
        let detail = match code {
            Some(code) => {
                if stderr.is_empty() {
                    format!("systemctl exited with code {code}")
                } else {
                    format!("systemctl exited with code {code}: {stderr}")
                }
            }
            None => {
                if stderr.is_empty() {
                    "systemctl was terminated".to_string()
                } else {
                    format!("systemctl was terminated: {stderr}")
                }
            }
        };
        warn!("error while trying to {action} nvidia-powerd: {detail}");
    }
}

/// stop the nvidia-powerd service using systemctl
pub async fn stop_nvidia_powerd() {
    if nvidia_powerd_enabled().await {
        run_systemctl("stop", &[]).await;
    }
}

/// start the nvidia-powerd service using systemctl, resetting its failed state first
pub async fn start_nvidia_powerd() {
    if nvidia_powerd_enabled().await {
        run_systemctl("reset-failed", &[]).await;
        run_systemctl("start", &[]).await;
    }
}

/// check whether the nvidia-powerd service is enabled
async fn nvidia_powerd_enabled() -> bool {
    let output = match timeout(
        Duration::from_secs(10),
        Command::new("systemctl")
            .arg("is-enabled")
            .arg(SERVICE)
            .kill_on_drop(true)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            error!("error while trying to detect nvidia-powerd: {err}");
            return false;
        }
        Err(_) => {
            error!("timed out after 10s while trying to detect nvidia-powerd");
            return false;
        }
    };
    if let Ok(output_str) = str::from_utf8(&output.stdout) {
        output_str.contains("enabled")
    } else {
        false
    }
}

/// Wait for the nvidia device is be initialized and return the lib
pub fn wait_for_nvidia(pci_id: &str, retries: usize) -> Option<Nvml> {
    for _attempt in 0..retries {
        if let Ok(nvml) = Nvml::init()
            && let Ok(nvidia_dev) = nvml.device_by_pci_bus_id(pci_id)
            && let Ok(_) = nvidia_dev.architecture()
        {
            return Some(nvml);
        } else {
            thread::sleep(Duration::from_millis(250));
        }
    }
    None
}
/// Get the device name using NVML
pub fn nvidia_get_device_name_nvml(nvml: &Nvml, pci_id: &str) -> Option<String> {
    if let Ok(nvidia_dev) = nvml.device_by_pci_bus_id(pci_id)
        && let Ok(name) = nvidia_dev.name()
    {
        return Some(name);
    }
    None
}
/// Get the device minor using NVML
pub fn nvidia_get_device_minor_nvml(nvml: &Nvml, pci_id: &str) -> Option<u32> {
    if let Ok(nvidia_dev) = nvml.device_by_pci_bus_id(pci_id)
        && let Ok(minor) = nvidia_dev.minor_number()
    {
        return Some(minor);
    }
    None
}
/// Get the device type using NVML
pub fn nvidia_get_device_type_nvml(nvml: &Nvml, pci_id: &str) -> GpuType {
    if let Ok(nvidia_dev) = nvml.device_by_pci_bus_id(pci_id) {
        if let Ok(virt_mode) = nvidia_dev.virtualization_mode() {
            match virt_mode {
                GpuVirtualizationMode::Vgpu => return GpuType::Virtual,
                // Do not want to assume this one
                GpuVirtualizationMode::Bare => {}
                // Others SHOULD be discrete
                _ => return GpuType::Discrete,
            }
        }
        match nvidia_dev.architecture() {
            Ok(
                DeviceArchitecture::Kepler
                | DeviceArchitecture::Maxwell
                | DeviceArchitecture::Pascal
                | DeviceArchitecture::Turing
                | DeviceArchitecture::Volta
                | DeviceArchitecture::Ampere
                | DeviceArchitecture::Ada
                | DeviceArchitecture::Hopper
                | DeviceArchitecture::Blackwell,
            ) => {
                return GpuType::Discrete;
            }
            // Architecture not implemented by nvml_wrapper yet
            // 11 is DLA
            // 12 is DLA2
            // 15 is NPU3
            // 13 is RUBIN
            // <https://github.com/NVIDIA/nvidia-settings/blob/df684ed9c29fd116c24a68198b681ef69e4f2c53/src/nvml.h#L1759-L1779>
            Err(NvmlError::UnexpectedVariant(raw)) => match raw {
                11 | 12 | 15 => return GpuType::Integrated,
                13 => return GpuType::Discrete,
                _ => {}
            },
            _ => {}
        }
        // Pretty much a fallback, i hope it doesnt get used
        if let Ok(brand) = nvidia_dev.brand() {
            match brand {
                Brand::Quadro
                | Brand::Tesla
                | Brand::GeForce
                | Brand::Titan
                | Brand::QuadroRTX
                | Brand::NvidiaRTX
                | Brand::GeForceRTX
                | Brand::NVS
                | Brand::TitanRTX => return GpuType::Discrete,
                Brand::GRID
                | Brand::VApps
                | Brand::VPC
                | Brand::VCS
                | Brand::VWS
                | Brand::CloudGaming => return GpuType::Virtual,
                _ => {
                    return GpuType::Unknown;
                }
            }
        }
    }
    GpuType::Unknown
}
