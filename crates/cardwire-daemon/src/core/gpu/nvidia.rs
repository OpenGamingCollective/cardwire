use std::time::Duration;

use log::{error, info, warn};
use tokio::{process::Command, time::timeout};

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
