use std::{fs, time::Duration};

use log::error;

use crate::interface::{GpuInterface, GpuInterfaceSignals};
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
    let mut current_power_state = fs::read_to_string(&power_path)?;
    let signal = interface.signal_emitter();
    loop {
        sleep(Duration::from_millis(500)).await;
        let new_power_state = match fs::read_to_string(&power_path) {
            Ok(state) => state,
            Err(e) => {
                error!(
                    "failed to read power_state for {}: {}",
                    gpu.device.name(),
                    e
                );
                continue;
            }
        };
        if current_power_state != new_power_state {
            signal.power_state_changed(&new_power_state).await?;

            current_power_state = new_power_state;
        }
    }
}
