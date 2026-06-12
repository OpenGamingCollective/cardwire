//! Functions for static analysis, contains:
//! - FDO desktop entries analysis
use freedesktop_desktop_entry::{DesktopEntry, get_languages_from_env};
use std::{
    collections::HashMap, fs, path::{Path, PathBuf}
};
use xdg::BaseDirectories;

/// Return a list of fdo apps present in the system
pub async fn get_fdo_apps() -> anyhow::Result<HashMap<String, bool>> {
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
    let mut app_list: HashMap<String, bool> = HashMap::new();
    let locales = get_languages_from_env();

    for app_directory in app_directories {
        // if directory is readable proceed, else just ignore it
        if let Ok(app_directory) = app_directory.read_dir() {
            // each file is an app entry
            for app in app_directory {
                let app = app?;
                let path = app.path();
                // ignore if app doesnt end with .desktop
                if let Some(ext) = path.extension()
                    && ext == "desktop"
                {
                    // for now, only keep flatpak apps that prefers a non default gpu
                    // the reason we only keep flatpak apps is because i can match a process with
                    // it's FLATPAK_ID env
                    if let Ok(app_fdo) = DesktopEntry::from_path(path, Some(&locales))
                        && app_fdo.prefers_non_default_gpu()
                        && let Some(flatpak_id) = app_fdo.flatpak()
                    {
                        app_list.insert(flatpak_id.to_string(), true);
                    }
                }
            }
        }
    }
    Ok(app_list)
}
