use freedesktop_desktop_entry::{DesktopEntry, desktop_entry_from_path, get_languages_from_env};
use std::{fs, path::PathBuf};
use xdg::BaseDirectories;

pub struct CardwireProfiler {}
impl CardwireProfiler {
    pub fn build() -> anyhow::Result<CardwireProfiler> {
        let mut app_directories: Vec<PathBuf> = Vec::new();
        // get from ENV
        let xdg_dir = BaseDirectories::new();
        let system_dirs = xdg_dir.get_data_dirs();
        for dir in system_dirs {
            let path = PathBuf::from(dir).join("applications");
            if path.exists() && path.is_dir() {
                app_directories.push(path);
            }
        }
        if let Ok(home_entries) = fs::read_dir("/home") {
            for entry in home_entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let mut user_app_dir = entry.path();
                        user_app_dir.push(".local/share/applications");

                        if user_app_dir.exists() && user_app_dir.is_dir() {
                            app_directories.push(user_app_dir);
                        }

                        let mut user_flatpak_dir = entry.path();
                        user_flatpak_dir.push(".local/share/flatpak/exports/share/applications");
                        if user_flatpak_dir.exists() && user_flatpak_dir.is_dir() {
                            app_directories.push(user_flatpak_dir);
                        }
                    }
                }
            }
        }
        let mut app_list: Vec<DesktopEntry> = Vec::new();
        let locales = get_languages_from_env();
        for app_directory in app_directories {
            for app in app_directory.read_dir()? {
                let app = app?;
                let path = app.path();
                if let Some(ext) = path.extension() {
                    if ext == "desktop" {
                        let app_fdo = DesktopEntry::from_path(path, Some(&locales)).unwrap();
                        app_list.push(app_fdo);
                    }
                }
            }
        }
        println!("List of found apps: {:?}", app_list);
        Ok(CardwireProfiler {})
    }
}
