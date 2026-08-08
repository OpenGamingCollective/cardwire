use std::{
    collections::HashSet, path::{Path, PathBuf}
};

use crate::models::{DbusAppMetadata, ResolvedApp};
use freedesktop_desktop_entry::DesktopEntry;

/// Returns all XDG data directories to search for applications and icons.
fn get_xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        let user_share = PathBuf::from(home.clone()).join(".local/share");
        if user_share.exists() {
            dirs.push(user_share);
        }
        let user_icons = PathBuf::from(home).join(".icons");
        if user_icons.exists() {
            dirs.push(user_icons);
        }
    }

    if let Ok(val) = std::env::var("XDG_DATA_DIRS") {
        for p in val.split(':') {
            if !p.is_empty() {
                let pb = PathBuf::from(p);
                if pb.exists() && !dirs.contains(&pb) {
                    dirs.push(pb);
                }
            }
        }
    }

    // Fallback standard locations on Linux and nix
    let fallbacks = [
        "/run/current-system/sw/share",
        "/var/lib/flatpak/exports/share",
        "/usr/local/share",
        "/usr/share",
    ];

    for fb in fallbacks {
        let pb = PathBuf::from(fb);
        if pb.exists() && !dirs.contains(&pb) {
            dirs.push(pb);
        }
    }

    dirs
}

/// Resolves raw DbusAppMetadata into a ResolvedApp
pub fn resolve_app_metadata(app_id: &str, raw: &DbusAppMetadata) -> ResolvedApp {
    let data_dirs = get_xdg_data_dirs();
    let locales = freedesktop_desktop_entry::get_languages_from_env();

    let mut resolved_name: Option<String> = None;
    let mut resolved_icon_name: Option<String> = raw.icon_name.clone();

    let mut candidate_filenames = Vec::new();
    if let Some(ref dt_id) = raw.desktop_file_id {
        if dt_id.ends_with(".desktop") {
            candidate_filenames.push(dt_id.clone());
        } else {
            candidate_filenames.push(format!("{}.desktop", dt_id));
        }
    }
    candidate_filenames.push(format!("{}.desktop", app_id));
    candidate_filenames.push(format!("{}.desktop", app_id.to_lowercase()));
    let mut chars = app_id.chars();
    if let Some(first) = chars.next() {
        let capitalized = format!("{}{}.desktop", first.to_uppercase(), chars.as_str());
        if !candidate_filenames.contains(&capitalized) {
            candidate_filenames.push(capitalized);
        }
    }

    'search_desktop: for data_dir in &data_dirs {
        let apps_dir = data_dir.join("applications");
        for candidate in &candidate_filenames {
            let path = apps_dir.join(candidate);
            if path.exists()
                && let Ok(entry) = DesktopEntry::from_path(&path, Some(&locales))
            {
                if let Some(name) = entry.name(&locales) {
                    let name_str = name.to_string();
                    if !name_str.trim().is_empty() {
                        resolved_name = Some(name_str);
                    }
                }
                if resolved_icon_name.is_none()
                    && let Some(icon) = entry.icon()
                {
                    resolved_icon_name = Some(icon.to_string());
                }
                if resolved_name.is_some() {
                    break 'search_desktop;
                }
            }
        }
    }

    let mut resolved_desktop_id = raw.desktop_file_id.clone();

    // If it's a Steam app (e.g. "steam_app_1070560"), scan desktop files for steam://rungameid/<id>
    if resolved_name.is_none()
        && let Some(steam_id) = app_id.strip_prefix("steam_app_")
    {
        let rungame_uri = format!("steam://rungameid/{}", steam_id);
        'search_steam: for data_dir in &data_dirs {
            let apps_dir = data_dir.join("applications");
            if let Ok(entries) = std::fs::read_dir(&apps_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension()
                        && ext == "desktop"
                        && let Ok(entry_fdo) = DesktopEntry::from_path(&path, Some(&locales))
                        && let Some(exec) = entry_fdo.exec()
                        && exec.contains(&rungame_uri)
                    {
                        if let Some(name) = entry_fdo.name(&locales) {
                            let name_str = name.to_string();
                            if !name_str.trim().is_empty() {
                                resolved_name = Some(name_str);
                            }
                        }
                        if resolved_icon_name.is_none()
                            && let Some(icon) = entry_fdo.icon()
                        {
                            resolved_icon_name = Some(icon.to_string());
                        }
                        if resolved_desktop_id.is_none() {
                            resolved_desktop_id = path.file_name().map(|s| {
                                s.to_string_lossy().trim_end_matches(".desktop").to_string()
                            });
                        }
                        if resolved_name.is_some() {
                            break 'search_steam;
                        }
                    }
                }
            }
        }
    }

    // Determine display name
    let display_name = if let Some(name) = resolved_name {
        name
    } else if !raw.display_name.trim().is_empty() && !raw.display_name.starts_with("Steam Game ") {
        raw.display_name.clone()
    } else if let Some(steam_id) = app_id.strip_prefix("steam_app_") {
        format!("Steam Game {}", steam_id)
    } else {
        app_id
            .split(&['-', '_'][..])
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    let icon_path = resolve_icon_path(resolved_icon_name.as_deref(), app_id, &data_dirs);

    ResolvedApp {
        app_id: app_id.to_string(),
        display_name,
        desktop_file_id: resolved_desktop_id,
        icon_name: resolved_icon_name,
        icon_path,
        gpu_policy: raw.gpu_policy,
    }
}

/// Resolves an icon name or app_id to a image
fn resolve_icon_path(
    icon_name: Option<&str>,
    app_id: &str,
    data_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let mut names_to_check = Vec::new();
    if let Some(name) = icon_name
        && !name.trim().is_empty()
    {
        let p = Path::new(name);
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }
        names_to_check.push(name.to_string());
        names_to_check.push(name.to_lowercase());
        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            let capitalized = format!("{}{}", first.to_uppercase(), chars.as_str());
            if !names_to_check.contains(&capitalized) {
                names_to_check.push(capitalized);
            }
        }
    }
    if !names_to_check.contains(&app_id.to_string()) {
        names_to_check.push(app_id.to_string());
        names_to_check.push(app_id.to_lowercase());
        let mut chars = app_id.chars();
        if let Some(first) = chars.next() {
            let capitalized = format!("{}{}", first.to_uppercase(), chars.as_str());
            if !names_to_check.contains(&capitalized) {
                names_to_check.push(capitalized);
            }
        }
    }

    let extensions = ["png", "svg", "xpm"];
    let icon_subdirs = [
        "icons/hicolor/128x128/apps",
        "icons/hicolor/256x256/apps",
        "icons/hicolor/512x512/apps",
        "icons/hicolor/64x64/apps",
        "icons/hicolor/48x48/apps",
        "icons/hicolor/scalable/apps",
        "pixmaps",
        "icons/hicolor/32x32/apps",
    ];

    let mut searched_paths = HashSet::new();

    for dir in data_dirs {
        for sub in &icon_subdirs {
            let base = dir.join(sub);
            if !base.exists() {
                continue;
            }
            for name in &names_to_check {
                for ext in &extensions {
                    let file_path = base.join(format!("{}.{}", name, ext));
                    if searched_paths.insert(file_path.clone()) && file_path.exists() {
                        return Some(file_path);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_app_metadata_steam_fallback() {
        let raw = DbusAppMetadata {
            display_name: "".to_string(),
            desktop_file_id: None,
            icon_name: None,
            gpu_policy: 0,
        };
        let resolved = resolve_app_metadata("steam_app_1070560", &raw);
        assert_eq!(resolved.app_id, "steam_app_1070560");
        assert_eq!(resolved.display_name, "Steam Game 1070560");
    }

    #[test]
    fn test_resolve_app_metadata_generic_name() {
        let raw = DbusAppMetadata {
            display_name: "blender".to_string(),
            desktop_file_id: None,
            icon_name: None,
            gpu_policy: 1,
        };
        let resolved = resolve_app_metadata("blender", &raw);
        assert_eq!(resolved.app_id, "blender");
        assert_eq!(resolved.gpu_policy, 1);
    }
}
