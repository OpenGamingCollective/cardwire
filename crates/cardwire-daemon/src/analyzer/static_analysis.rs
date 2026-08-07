//! Functions for static analysis, contains:
//! - FDO desktop entries analysis
use freedesktop_desktop_entry::{DesktopEntry, get_languages_from_env};
use std::{
    collections::HashMap, fs, path::{Path, PathBuf}
};
use xdg::BaseDirectories;

#[derive(Clone, Debug)]
pub struct AppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
}

/// Return a list of fdo apps present in the system
pub async fn get_fdo_apps() -> anyhow::Result<HashMap<String, AppMetadata>> {
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
                    // Push both lowercase and normal name to the hashmap
                    // the RPCS3 .desktop contain the name `RPCS3` but the comm is `rpcs3`, so
                    // we need to lowercase it On the other, Ryujinx
                    // .desktop's name is `Ryujinx` and the comm is `Ryujinx`, so we also push
                    // the default name
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
                    app_list.insert(name.to_string(), meta.clone());

                    // Also insert the flatpak ID, lowercased since lookups are lowercased
                    if let Some(flatpak_id) = app_fdo.flatpak() {
                        app_list.insert(flatpak_id.to_ascii_lowercase(), meta.clone());
                    }
                    if let Some(exec_str) = app_fdo.exec() {
                        for part in exec_str.split_whitespace() {
                            if part == "env" || part.contains('=') {
                                continue;
                            }

                            let binary = part.split('/').next_back().unwrap_or(part);
                            if !binary.is_empty() {
                                app_list.insert(binary.to_lowercase(), meta.clone());
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(app_list)
}
