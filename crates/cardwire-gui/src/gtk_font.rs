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

// idk maybe this person has such a slow drive, but let it be one second, whatever
const GSETTINGS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

fn query_gsettings() -> Option<String> {
    use std::{io::Read, process::Stdio, time::Instant};

    let mut child = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "font-name"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + GSETTINGS_TIMEOUT;
    let status = loop {
        match child.try_wait().ok()? {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    // murder this so we don't leave a zombie/orphan around.
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    };

    if !status.success() {
        return None;
    }

    let mut stdout = String::new();
    child.stdout.take()?.read_to_string(&mut stdout).ok()?;
    // gsettings wraps the value in single quotes for WHATEVER reason
    Some(stdout.trim().trim_matches('\'').to_string())
}

// needed for font family stripping
const STYLE_TOKENS: &[&str] = &[
    "Bold",
    "Italic",
    "Oblique",
    "Regular",
    "Light",
    "Medium",
    "Thin",
    "Black",
    "Heavy",
    "Semibold",
    "Semi-Bold",
    "Extrabold",
    "Extra-Bold",
    "Condensed",
    "Extralight",
];

fn strip_style_tokens(name: &str) -> String {
    let mut parts: Vec<&str> = name.split_whitespace().collect();
    while let Some(last) = parts.last() {
        if STYLE_TOKENS.iter().any(|t| t.eq_ignore_ascii_case(last)) {
            parts.pop();
        } else {
            break;
        }
    }
    if parts.is_empty() {
        name.to_string()
    } else {
        parts.join(" ")
    }
}

fn parse_family(gtk_font_name: &str) -> String {
    // if it SOMEHOW has "Inter,  10" format
    if let Some((family, _)) = gtk_font_name.split_once(',') {
        return strip_style_tokens(family.trim());
    }

    let parts: Vec<&str> = gtk_font_name.split_whitespace().collect();
    match parts.as_slice() {
        [] => "Sans".to_string(),
        [name] => name.to_string(),
        [.., last] => {
            let family = if last.parse::<f32>().is_ok() {
                parts[..parts.len() - 1].join(" ")
            } else {
                gtk_font_name.to_string()
            };
            strip_style_tokens(&family)
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
        assert_eq!(parse_family("Ubuntu Bold 12"), "Ubuntu");
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
