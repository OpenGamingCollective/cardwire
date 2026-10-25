use crate::{app_metadata::types::AppMetadata, fdo::fdo_entries::parse_fdo_app};
use freedesktop_desktop_entry::{DesktopEntry, get_languages_from_env};
use inotify::{EventMask, Inotify, StreamExt, WatchDescriptor, WatchMask};
use log::error;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// Create a inotify for each folders, watch for changes and update the xdg-list
pub async fn watch_fdo_folders(
    xdg_folders: Vec<PathBuf>,
    xdg_list: Arc<RwLock<HashMap<String, AppMetadata>>>,
) {
    if xdg_folders.is_empty() {
        error!("xdg_folder is empty, exiting notify task...");
        return;
    }

    let inotify = match Inotify::init() {
        Ok(v) => v,
        Err(err) => {
            error!("Couldn't init inotify: {}", err);
            return;
        }
    };
    let mut watched_dirs: HashMap<WatchDescriptor, PathBuf> = HashMap::new();
    let watch_mask =
        WatchMask::CREATE | WatchMask::MODIFY | WatchMask::MOVED_TO | WatchMask::CLOSE_WRITE;

    for folder in xdg_folders {
        match inotify.watches().add(&folder, watch_mask) {
            Ok(wd) => {
                watched_dirs.insert(wd, folder);
            }

            Err(err) => {
                error!("Cannot watch {}: {err}", folder.display());
            }
        }
    }

    let mut buffer = [0; 4096];
    let mut stream = match inotify.into_event_stream(&mut buffer) {
        Ok(s) => s,
        Err(err) => {
            error!("Couldn't convert inotify into a stream: {}", err);
            return;
        }
    };
    let locales = get_languages_from_env();

    loop {
        while let Some(event_result) = stream.next().await {
            let event = match event_result {
                Ok(event) => event,
                Err(err) => {
                    error!("Error reading inotify event: {err}");
                    continue;
                }
            };

            let Some(name) = event.name else {
                if event.mask.contains(EventMask::Q_OVERFLOW) {
                    error!("inotify queue overflowed");
                }

                continue;
            };

            let Some(folder) = watched_dirs.get(&event.wd) else {
                error!("Received event for unknown watch descriptor");
                continue;
            };

            // `name` is relative to the watched directory.
            let path = folder.join(name);

            if let Some(ext) = path.extension()
                && ext == "desktop"
                && let Ok(app_fdo) = DesktopEntry::from_path(&path, Some(&locales))
                && let Some(name) = app_fdo.name(&locales)
            {
                let new_app_map = parse_fdo_app(&app_fdo, &name, &path);
                let mut xdg_list = xdg_list.write().await;
                xdg_list.extend(new_app_map);
            }
        }
    }
}
