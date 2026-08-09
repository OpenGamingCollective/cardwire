//! Used to get GPU env for specific GPU

use crate::{core::gpu::GpuVendor, types::Modes};

/// Launchable if not blocked and availble, or if in smart mode
pub fn is_gpu_launchable(is_available: bool, is_blocked: bool, mode: Modes) -> bool {
    is_available && (!is_blocked || mode == Modes::Smart)
}

pub fn compute_switcheroo_env(
    gpu_count: usize,
    is_default: bool,
    is_discrete: bool,
    gpu_id: u32,
    vendor: GpuVendor,
    pci_address: &str,
) -> Vec<String> {
    // Return early for the default display GPU
    if is_default {
        return if is_discrete {
            // Primary dGPU (desktop): allow direct access to primary discrete GPU
            vec!["CARDWIRE_ALLOW".to_string(), "1".to_string()]
        } else {
            // Primary iGPU (laptop): restrict default rendering to integrated GPU
            vec!["CARDWIRE_ALLOW".to_string(), "0".to_string()]
        };
    }

    let mut env = Vec::new();

    // Cardwire-specific routing variable
    if gpu_count == 2 && is_discrete {
        // Dual-GPU hybrid setup: force offloaded process onto dGPU
        env.push("CARDWIRE_FORCE_DGPU".to_string());
        env.push("1".to_string());
    } else {
        // Multi-GPU (3+) or secondary iGPU: route using explicit GPU ID
        env.push("CARDWIRE_FORCE_GPU".to_string());
        env.push(gpu_id.to_string());
    }

    // Standard switcheroo-control / Mesa / NVIDIA offload variables
    // DRI_PRIME=pci-<addr> selects the render node by PCI address
    let dri_prime_val = format!("pci-{}", pci_address.replace([':', '.'], "_"));
    match vendor {
        GpuVendor::Nvidia => {
            env.push("__NV_PRIME_RENDER_OFFLOAD".to_string());
            env.push("1".to_string());
            env.push("__GLX_VENDOR_LIBRARY_NAME".to_string());
            env.push("nvidia".to_string());
            env.push("__VK_LAYER_NV_optimus".to_string());
            env.push("NVIDIA_only".to_string());
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*nvidia*,*nouveau*".to_string());
        }
        GpuVendor::Amd => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*radeon*".to_string());
        }
        GpuVendor::Intel => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
            env.push("VK_LOADER_DRIVERS_SELECT".to_string());
            env.push("*intel*".to_string());
        }
        GpuVendor::Other => {
            env.push("DRI_PRIME".to_string());
            env.push(dri_prime_val);
        }
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_gpu_launchable_laptop() {
        // Blocked dGPU is launchable only in Smart mode
        assert!(is_gpu_launchable(true, true, Modes::Smart));
        assert!(!is_gpu_launchable(true, true, Modes::Integrated));
        assert!(!is_gpu_launchable(true, true, Modes::Hybrid));
        // Unblocked GPUs are always launchable
        assert!(is_gpu_launchable(true, false, Modes::Integrated));
        assert!(is_gpu_launchable(true, false, Modes::Hybrid));
        assert!(is_gpu_launchable(true, false, Modes::Smart));
    }

    #[test]
    fn test_is_gpu_launchable_desktop_and_multi_gpu() {
        // No Smart mode on desktops/multi-GPU systems: blocked GPUs are never launchable
        assert!(!is_gpu_launchable(true, true, Modes::Manual));
        assert!(!is_gpu_launchable(true, true, Modes::Hybrid));
        assert!(is_gpu_launchable(true, false, Modes::Manual));
        assert!(is_gpu_launchable(true, false, Modes::Hybrid));
    }

    #[test]
    fn test_is_gpu_launchable_unavailable_gpu() {
        assert!(!is_gpu_launchable(false, false, Modes::Hybrid));
        assert!(!is_gpu_launchable(false, false, Modes::Smart));
    }

    #[test]
    fn test_compute_switcheroo_env_laptop_hybrid_nvidia() {
        // Laptop: iGPU default, Nvidia dGPU secondary
        let igpu_env = compute_switcheroo_env(2, true, false, 0, GpuVendor::Intel, "0000:00:02.0");
        assert_eq!(igpu_env, vec!["CARDWIRE_ALLOW", "0"]);

        let dgpu_env = compute_switcheroo_env(2, false, true, 1, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(
            dgpu_env,
            vec![
                "CARDWIRE_FORCE_DGPU",
                "1",
                "__NV_PRIME_RENDER_OFFLOAD",
                "1",
                "__GLX_VENDOR_LIBRARY_NAME",
                "nvidia",
                "__VK_LAYER_NV_optimus",
                "NVIDIA_only",
                "VK_LOADER_DRIVERS_SELECT",
                "*nvidia*,*nouveau*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_laptop_hybrid_amd() {
        // Laptop: iGPU default, AMD dGPU secondary
        let dgpu_env = compute_switcheroo_env(2, false, true, 1, GpuVendor::Amd, "0000:03:00.0");
        assert_eq!(
            dgpu_env,
            vec![
                "CARDWIRE_FORCE_DGPU",
                "1",
                "DRI_PRIME",
                "pci-0000_03_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_desktop_dgpu_primary() {
        // Desktop: dGPU default, iGPU secondary
        let dgpu_env = compute_switcheroo_env(2, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(dgpu_env, vec!["CARDWIRE_ALLOW", "1"]);

        let igpu_env = compute_switcheroo_env(2, false, false, 1, GpuVendor::Amd, "0000:0d:00.0");
        assert_eq!(
            igpu_env,
            vec![
                "CARDWIRE_FORCE_GPU",
                "1",
                "DRI_PRIME",
                "pci-0000_0d_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }

    #[test]
    fn test_compute_switcheroo_env_single_gpu() {
        let single_env =
            compute_switcheroo_env(1, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(single_env, vec!["CARDWIRE_ALLOW", "1"]);

        let single_igpu =
            compute_switcheroo_env(1, true, false, 0, GpuVendor::Intel, "0000:00:02.0");
        assert_eq!(single_igpu, vec!["CARDWIRE_ALLOW", "0"]);
    }

    #[test]
    fn test_compute_switcheroo_env_multi_gpu() {
        let default_env =
            compute_switcheroo_env(3, true, true, 0, GpuVendor::Nvidia, "0000:01:00.0");
        assert_eq!(default_env, vec!["CARDWIRE_ALLOW", "1"]);

        let secondary_amd =
            compute_switcheroo_env(3, false, true, 2, GpuVendor::Amd, "0000:04:00.0");
        assert_eq!(
            secondary_amd,
            vec![
                "CARDWIRE_FORCE_GPU",
                "2",
                "DRI_PRIME",
                "pci-0000_04_00_0",
                "VK_LOADER_DRIVERS_SELECT",
                "*radeon*"
            ]
        );
    }
}
