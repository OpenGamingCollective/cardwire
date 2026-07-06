//! Watch the power state and send a signal when it changes, one task is spawned per gpu
use crate::{
    core::gpu::PowerState, interface::{GpuInterface, GpuInterfaceSignals}
};
use log::{error, info, warn};
use std::{fs, str::FromStr, time::Duration};
use tokio::time::sleep;
use zbus::object_server::{self};

pub async fn watch_power_state(
    gpu: GpuInterface,
    interface: object_server::InterfaceRef<GpuInterface>,
) -> anyhow::Result<()> {
    let power_path = format!(
        "/sys/bus/pci/devices/{}/power_state",
        gpu.device.pci.pci_address()
    );

    // Default is unknown
    let mut power_state = PowerState::default();

    let signal = interface.signal_emitter();

    loop {
        sleep(Duration::from_millis(500)).await;
        let new_power_state_str = match fs::read_to_string(&power_path) {
            Ok(state) => state,
            Err(e) => {
                error!(
                    "failed to read power_state file for {}: {}",
                    gpu.device.name(),
                    e
                );
                continue;
            }
        };
        // it shouldn't return err
        let new_power_state = PowerState::from_str(new_power_state_str.trim())?;

        // Skip if unknown powerstate
        if new_power_state == PowerState::Unknown {
            warn!("power state couldn't be read: {}", &new_power_state_str);
            continue;
        }
        if power_state != new_power_state {
            info!(
                "{}: Power state changed: {}",
                gpu.device.name(),
                new_power_state
            );
            signal
                .power_state_changed(&format!("{}", new_power_state))
                .await?;

            power_state = new_power_state;
        }
    }
}
