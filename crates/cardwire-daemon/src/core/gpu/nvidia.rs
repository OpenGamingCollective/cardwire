use std::process::Stdio;

use log::{error, info, warn};
use tokio::process::Command;

/// restart the nvidia-powerd service using systemctl
pub async fn restart_nvidia_powerd() {
    let service = "nvidia-powerd.service";

    let enabled = match Command::new("systemctl")
        .arg("is-enabled")
        .arg(service)
        .output()
        .await
    {
        Ok(output) => {
            if let Ok(output_str) = str::from_utf8(&output.stdout) {
                output_str.contains("enabled")
            } else {
                false
            }
        }
        Err(err) => {
            error!("error while trying to detect nvidia-powerd: {}", err);
            return;
        }
    };
    if enabled {
        match Command::new("systemctl")
            .arg("restart")
            .arg(service)
            .arg("--no-block")
            .stdout(Stdio::null())
            .status()
            .await
        {
            Ok(status) => {
                if status.success() {
                    info!("successfully restart nvidia-powerd.service");
                } else {
                    warn!("error restarting nvidia-powerd: {:?}", status.code())
                }
            }
            Err(err) => {
                error!("error while trying to restart nvidia-powerd: {}", err)
            }
        };
    }
}
