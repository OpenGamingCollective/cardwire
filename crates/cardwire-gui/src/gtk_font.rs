pub fn default_font() -> iced::Font {
    match query_gsettings() {
        Some(raw) => {
            let family = parse_family(&raw);
            log::info!("Using GTK interface font: {family}");
            // ugly leak hack
            let name: &'static str = Box::leak(family.into_boxed_str());
            iced::Font::with_name(name)
        }
        None => {
            log::warn!("Could not read GTK font via gsettings, falling back to iced default");
            iced::Font::DEFAULT
        }
    }
}

fn query_gsettings() -> Option<String> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "font-name"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout);
    // gsettings wraps the value in single quotes for WHATEVER reason
    Some(s.trim().trim_matches('\'').to_string())
}

fn parse_family(gtk_font_name: &str) -> String {
    // if it SOMEHOW has "Inter,  10" format
    if let Some((family, _)) = gtk_font_name.split_once(',') {
        return family.trim().to_string();
    }

    let parts: Vec<&str> = gtk_font_name.split_whitespace().collect();
    match parts.as_slice() {
        [] => "Sans".to_string(),
        [name] => name.to_string(),
        [.., last] => {
            if last.parse::<f32>().is_ok() {
                parts[..parts.len() - 1].join(" ")
            } else {
                gtk_font_name.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_family;

    #[test]
    fn parses_gnome_style() {
        assert_eq!(parse_family("Adwaita Sans 11"), "Adwaita Sans");
    }

    #[test]
    fn parses_gnome_style_with_weight() {
        assert_eq!(parse_family("Ubuntu Bold 12"), "Ubuntu Bold");
    }

    #[test]
    fn parses_kde_style() {
        assert_eq!(parse_family("Inter,  10"), "Inter");
    }

    #[test]
    fn handles_no_size() {
        assert_eq!(parse_family("Sans"), "Sans");
    }
}
