//! Functions for static analysis, contains:
//! - FDO desktop entries analysis
use freedesktop_desktop_entry::{DesktopEntry, get_languages_from_env};
use inotify::{EventMask, Inotify, StreamExt, WatchDescriptor, WatchMask};
use log::error;
use std::{
    collections::HashMap, fs, path::{Path, PathBuf}, sync::Arc
};
use tokio::sync::RwLock;
use xdg::BaseDirectories;

#[derive(Clone, Debug)]
pub struct AppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
}

/// Return a list of fdo apps present in the system
pub async fn get_fdo_apps() -> anyhow::Result<(HashMap<String, AppMetadata>, Vec<PathBuf>)> {
    let mut app_directories: Vec<PathBuf> = Vec::new();
    // get from ENV
    let xdg_dir = BaseDirectories::new();
    let system_dirs = xdg_dir.get_data_dirs();
    for dir in system_dirs {
        let path = dir.join("applications");
        if path.exists() && path.is_dir() {
            app_directories.push(path);
        }
    }

    // Read /home to get a list of users
    if let Ok(home_entries) = fs::read_dir("/home") {
        for entry in home_entries.flatten() {
            // if it's a dir
            if let Ok(file_type) = entry.file_type()
                && file_type.is_dir()
            {
                // get username
                let user = entry.file_name();
                // store the home path of the user, eg: /home/john/
                let mut user_app_dir = entry.path();
                // .desktop often reside in this directory
                user_app_dir.push(".local/share/applications");

                if user_app_dir.exists() && user_app_dir.is_dir() {
                    app_directories.push(user_app_dir);
                }

                // this is for flatpaks .desktop
                let mut user_flatpak_dir = entry.path();
                user_flatpak_dir.push(".local/share/flatpak/exports/share/applications");
                if user_flatpak_dir.exists() && user_flatpak_dir.is_dir() {
                    app_directories.push(user_flatpak_dir);
                }
                let nix_path_hm = format!(
                    "/etc/profiles/per-user/{}/share/applications/",
                    user.to_string_lossy()
                );
                let nix_path_hm = Path::new(&nix_path_hm);
                if nix_path_hm.exists() {
                    app_directories.push(nix_path_hm.to_path_buf());
                }
            }
        }
    }
    // Now read the paths to get the .desktop entries
    let mut app_list: HashMap<String, AppMetadata> = HashMap::new();
    let locales = get_languages_from_env();

    for app_directory in &app_directories {
        // if directory is readable proceed, else just ignore it
        if let Ok(app_directory) = app_directory.read_dir() {
            // each file is an app entry
            for app in app_directory {
                let app = app?;
                let path = app.path();
                // ignore if app doesnt end with .desktop
                if let Some(ext) = path.extension()
                    && ext == "desktop"
                    && let Ok(app_fdo) = DesktopEntry::from_path(&path, Some(&locales))
                    && let Some(name) = app_fdo.name(&locales)
                {
                    let new_app_map = parse_fdo_app(&app_fdo, &name, &path);
                    app_list.extend(new_app_map);
                }
            }
        }
    }
    Ok((app_list, app_directories))
}

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

fn parse_fdo_app(app_fdo: &DesktopEntry, name: &str, path: &Path) -> HashMap<String, AppMetadata> {
    let mut app_list: HashMap<String, AppMetadata> = HashMap::new();

    let display_name = name.to_string();
    let icon_name = app_fdo.icon().map(|icon| icon.to_string());

    let desktop_file_id = path
        .file_name()
        .map(|s| s.to_string_lossy().trim_end_matches(".desktop").to_string());

    let meta = AppMetadata {
        display_name,
        desktop_file_id,
        icon_name,
    };

    // Push both lowercase and normal name as fallbacks
    app_list.insert(name.to_ascii_lowercase(), meta.clone());
    app_list.insert(meta.display_name.clone(), meta.clone());

    if let Some(flatpak_id) = app_fdo.flatpak() {
        app_list.insert(flatpak_id.to_ascii_lowercase(), meta.clone());
    }
    if let Some(exec_str) = app_fdo.exec() {
        let exec_parts: Vec<&str> = exec_str.split_whitespace().collect();

        // Scan all parts for a steam URI before applying the wrapper-binary
        // stop condition, so `Exec=steam steam://rungameid/<id>` still maps
        // to the steam app metadata
        if let Some(uri_part) = exec_parts
            .iter()
            .find(|part| part.starts_with("steam://rungameid/"))
        {
            let app_id = uri_part
                .trim_start_matches("steam://rungameid/")
                .trim_matches('/');
            app_list.insert(format!("steam_app_{}", app_id), meta.clone());
        } else {
            for part in exec_parts {
                if part == "env" || part.contains('=') {
                    continue;
                }
                let binary = part.split('/').next_back().unwrap_or(part);
                if ["flatpak", "steam", "sh", "bash", "bwrap"].contains(&binary) {
                    break;
                }
                if !binary.is_empty() {
                    app_list.insert(binary.to_lowercase(), meta.clone());
                }
                break;
            }
        }
    }

    app_list
}
