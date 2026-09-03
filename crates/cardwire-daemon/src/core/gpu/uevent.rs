use log::{error, info, warn};

use crate::{Result, core::errors::CardwireError};
use std::{
    fmt::Display, fs::{File, write}, os::fd::{self, OwnedFd}, path::Path
};

/// List of drm uevents cardwire can send
#[derive(Debug, PartialEq)]
pub enum DrmUEvents {
    Add,
    Remove,
}
impl Display for DrmUEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "add"),
            Self::Remove => write!(f, "remove"),
        }
    }
}
pub fn send_uevent_blocking(uevent: DrmUEvents, card_id: u32, render_id: u32) -> Result<()> {
    // If it's an add event, we need to the GPU before sending the signal to allow the DE to
    // detect it
    // since it's an Owned fd, it should be dropped once the function ends
    let _owned_gpu_fd = {
        warn!("Waking up the GPU before sending add event...");
        get_gpu_fd(render_id)?
    };

    info!("sending this event: {} to card: {}", uevent, card_id);
    if let Err(err) = send_drm_uevent(&uevent, card_id) {
        error!(
            "Couldn't send drm uevent {} for card{}: {}",
            uevent, card_id, err
        );
        return Err(err);
    }
    std::thread::sleep(std::time::Duration::from_millis(2000));
    Ok(())
}

/// Get a file descriptor of the GPU, this function is blocking and file descriptor must be dropped
/// as fast as possible
fn get_gpu_fd(render_id: u32) -> Result<fd::OwnedFd> {
    let path = format!("/dev/dri/renderD{}", render_id);
    let path = Path::new(&path);
    if !path.exists() {
        return Err(CardwireError::DriRenderdNotFound(render_id));
    }
    let render_fd = File::open(path)?;
    Ok(OwnedFd::from(render_fd))
}

/// Send a "change" uevent for a DRM card, prompting the display server to
/// rescan connectors.
pub fn send_drm_uevent(uevent: &DrmUEvents, card: u32) -> Result<()> {
    let msg = format!("{}\n", uevent);
    write(format!("/sys/class/drm/card{card}/uevent"), msg).map_err(CardwireError::Io)
}
