use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ksni::{MenuItem, Tray, TrayMethods};
use serde::{Deserialize, Serialize};
use strum::VariantArray;
use tokio::sync::mpsc;

use crate::{helpers::GpuDevice, models::Mode};

const CONFIG_FILE: &str = "tray.toml";
const ICON_PREFIX: &str = "com.github.opengamingcollective.cardwire.tray";
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrayConfig {
    pub toggle_from: Mode,
    pub toggle_to: Mode,
    #[serde(default)]
    pub start_in_tray: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            toggle_from: Mode::Integrated,
            toggle_to: Mode::Hybrid,
            start_in_tray: false,
        }
    }
}

impl TrayConfig {
    pub fn load() -> io::Result<Self> {
        let path = config_path()?;
        if path.exists() {
            Self::load_from(&path)
        } else {
            Ok(Self::default())
        }
    }

    fn load_from(path: &Path) -> io::Result<Self> {
        let config = toml::from_str(&fs::read_to_string(path)?).map_err(io::Error::other)?;
        Self::validate(config)
    }

    pub fn save(self) -> io::Result<()> {
        self.save_to(&config_path()?)
    }

    fn save_to(self, path: &Path) -> io::Result<()> {
        Self::validate(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(&self).map_err(io::Error::other)?;
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("toml.tmp-{}-{sequence}", std::process::id()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temporary)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    pub fn next_mode(self, current: Mode) -> Mode {
        if current == self.toggle_from {
            self.toggle_to
        } else {
            self.toggle_from
        }
    }

    pub fn with_toggle_from(mut self, mode: Mode) -> Self {
        let previous = self.toggle_from;
        self.toggle_from = mode;
        if self.toggle_to == mode {
            self.toggle_to = previous;
        }
        self
    }

    pub fn with_toggle_to(mut self, mode: Mode) -> Self {
        let previous = self.toggle_to;
        self.toggle_to = mode;
        if self.toggle_from == mode {
            self.toggle_from = previous;
        }
        self
    }

    pub const fn with_start_in_tray(mut self, start_in_tray: bool) -> Self {
        self.start_in_tray = start_in_tray;
        self
    }

    fn validate(config: Self) -> io::Result<Self> {
        if config.toggle_from == config.toggle_to {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tray toggle modes must be different",
            ))
        } else {
            Ok(config)
        }
    }
}

fn config_path() -> io::Result<PathBuf> {
    xdg::BaseDirectories::with_prefix("cardwire")
        .get_config_home()
        .map(|path| path.join(CONFIG_FILE))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "could not determine config home"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    ToggleConfiguredMode,
    SetMode(Mode),
    SetGpuBlock { id: usize, blocked: bool },
    OpenGui,
    Quit,
}

pub struct CardwireTray {
    mode: Option<Mode>,
    gpus: BTreeMap<usize, GpuDevice>,
    action_tx: mpsc::UnboundedSender<TrayAction>,
}

impl CardwireTray {
    fn new(action_tx: mpsc::UnboundedSender<TrayAction>) -> Self {
        Self {
            mode: None,
            gpus: BTreeMap::new(),
            action_tx,
        }
    }

    fn icon(mode: Mode) -> String {
        format!("{ICON_PREFIX}-{}", mode.to_string().to_lowercase())
    }
}

impl Tray for CardwireTray {
    fn id(&self) -> String {
        ICON_PREFIX.to_string()
    }

    fn title(&self) -> String {
        "Cardwire".to_string()
    }

    fn icon_name(&self) -> String {
        self.mode
            .map(Self::icon)
            .unwrap_or_else(|| ICON_PREFIX.to_string())
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.action_tx.send(TrayAction::ToggleConfiguredMode);
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        if self.mode.is_none() {
            return ksni::ToolTip {
                title: "Cardwire".to_string(),
                description: "Cardwire daemon is unavailable; reconnecting…".to_string(),
                ..Default::default()
            };
        }

        let mut description = String::from("Name | Power state | Default | Blocked");
        for gpu in self.gpus.values() {
            description.push_str(&format!(
                "\n{} | {} | {} | {}",
                gpu.name,
                gpu.power_state.as_deref().unwrap_or("Unknown").trim(),
                if gpu.default { "yes" } else { "no" },
                if gpu.blocked { "yes" } else { "no" }
            ));
        }
        ksni::ToolTip {
            title: "Cardwire GPUs".to_string(),
            description,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = vec![
            ksni::menu::StandardItem {
                label: "Open Cardwire".to_string(),
                icon_name: "preferences-system".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_tx.send(TrayAction::OpenGui);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
        ];

        if self.mode.is_some() {
            let options = Mode::VARIANTS
                .iter()
                .copied()
                .map(|mode| ksni::menu::RadioItem {
                    label: format!("{mode} Mode"),
                    icon_name: Self::icon(mode),
                    ..Default::default()
                })
                .collect();
            items.push(
                ksni::menu::RadioGroup {
                    selected: self.mode.map_or(0, |mode| mode as usize),
                    select: Box::new(|tray: &mut Self, index| {
                        if let Some(mode) = Mode::from_repr(index as u32) {
                            let _ = tray.action_tx.send(TrayAction::SetMode(mode));
                        }
                    }),
                    options,
                }
                .into(),
            );

            if self.mode == Some(Mode::Manual) {
                let gpu_items = self
                    .gpus
                    .iter()
                    .filter(|(_, gpu)| !gpu.default)
                    .map(|(&id, gpu)| {
                        ksni::menu::CheckmarkItem {
                            label: gpu.name.clone(),
                            checked: gpu.blocked,
                            activate: Box::new(move |tray: &mut Self| {
                                let blocked =
                                    tray.gpus.get(&id).map(|gpu| gpu.blocked).unwrap_or(false);
                                let _ = tray.action_tx.send(TrayAction::SetGpuBlock {
                                    id,
                                    blocked: !blocked,
                                });
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect::<Vec<_>>();
                if !gpu_items.is_empty() {
                    items.push(MenuItem::Separator);
                    items.push(
                        ksni::menu::SubMenu {
                            label: "Blocked GPUs".to_string(),
                            icon_name: ICON_PREFIX.to_string(),
                            submenu: gpu_items,
                            ..Default::default()
                        }
                        .into(),
                    );
                }
            }
        } else {
            items.push(
                ksni::menu::StandardItem {
                    label: "Cardwire daemon unavailable".to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        items.extend([
            MenuItem::Separator,
            ksni::menu::StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.action_tx.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]);
        items
    }
}

#[derive(Clone)]
pub struct TrayHandle(ksni::Handle<CardwireTray>);

impl fmt::Debug for TrayHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TrayHandle")
    }
}

impl TrayHandle {
    pub async fn shutdown(self) {
        self.0.shutdown().await;
    }
}

pub async fn spawn() -> Result<(TrayHandle, mpsc::UnboundedReceiver<TrayAction>), ksni::Error> {
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let handle = TrayHandle(CardwireTray::new(action_tx).spawn().await?);
    Ok((handle, action_rx))
}

pub async fn update(handle: TrayHandle, mode: Option<Mode>, gpus: BTreeMap<usize, GpuDevice>) {
    let _ = handle
        .0
        .update(|tray| {
            tray.mode = mode;
            tray.gpus = gpus;
        })
        .await;
}

pub async fn notify_failure(message: String) {
    let _ = tokio::task::spawn_blocking(move || {
        notify_rust::Notification::new()
            .summary("Cardwire")
            .body(&message)
            .icon(ICON_PREFIX)
            .show()
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cardwire-tray-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    fn gpu(name: &str, default: bool, blocked: bool, power_state: &str) -> GpuDevice {
        GpuDevice {
            id: 0,
            name: name.to_string(),
            pci: String::new(),
            render: 0,
            card: 0,
            default,
            blocked,
            nvidia: false,
            nvidia_minor: String::new(),
            power_state: Some(power_state.to_string()),
        }
    }

    fn tray(mode: Option<Mode>) -> (CardwireTray, mpsc::UnboundedReceiver<TrayAction>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut tray = CardwireTray::new(tx);
        tray.mode = mode;
        (tray, rx)
    }

    #[test]
    fn defaults_to_integrated_and_hybrid() {
        let config = TrayConfig::default();
        assert_eq!(config.toggle_from, Mode::Integrated);
        assert_eq!(config.toggle_to, Mode::Hybrid);
        assert!(!config.start_in_tray);
    }

    #[test]
    fn rejects_duplicate_modes() {
        let path = temporary_path("duplicate");
        fs::write(&path, "toggle_from = 'smart'\ntoggle_to = 'smart'\n").unwrap();
        assert_eq!(
            TrayConfig::load_from(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn saves_and_loads_current_schema() {
        let path = temporary_path("roundtrip");
        let expected = TrayConfig {
            toggle_from: Mode::Manual,
            toggle_to: Mode::Smart,
            start_in_tray: true,
        };
        expected.save_to(&path).unwrap();
        assert_eq!(TrayConfig::load_from(&path).unwrap(), expected);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_saves_use_distinct_temporary_files() {
        let path = temporary_path("concurrent");
        let configs = [
            TrayConfig::default().with_start_in_tray(true),
            TrayConfig::default().with_toggle_to(Mode::Smart),
        ];
        let writers = configs.map(|config| {
            let path = path.clone();
            std::thread::spawn(move || config.save_to(&path))
        });

        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        let saved = TrayConfig::load_from(&path).unwrap();
        assert!(configs.contains(&saved));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn selecting_duplicate_endpoint_swaps_the_other_endpoint() {
        let config = TrayConfig::default().with_toggle_from(Mode::Hybrid);
        assert_eq!(config.toggle_from, Mode::Hybrid);
        assert_eq!(config.toggle_to, Mode::Integrated);
    }

    #[test]
    fn chooses_configured_toggle_destination() {
        let config = TrayConfig {
            toggle_from: Mode::Smart,
            toggle_to: Mode::Manual,
            start_in_tray: false,
        };
        assert_eq!(config.next_mode(Mode::Smart), Mode::Manual);
        assert_eq!(config.next_mode(Mode::Hybrid), Mode::Smart);
    }

    #[test]
    fn loads_legacy_schema_with_visible_gui_default() {
        let path = temporary_path("legacy");
        fs::write(&path, "toggle_from = 'integrated'\ntoggle_to = 'hybrid'\n").unwrap();
        assert!(!TrayConfig::load_from(&path).unwrap().start_in_tray);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn primary_activation_requests_configured_toggle() {
        let (mut tray, mut actions) = tray(Some(Mode::Integrated));
        tray.activate(0, 0);
        assert_eq!(
            actions.try_recv().unwrap(),
            TrayAction::ToggleConfiguredMode
        );
    }

    #[test]
    fn offline_menu_disables_mutations() {
        let (tray, _) = tray(None);
        assert!(
            !tray
                .menu()
                .iter()
                .any(|item| matches!(item, MenuItem::RadioGroup(_)))
        );
    }

    #[test]
    fn manual_menu_only_lists_non_default_gpus() {
        let (mut tray, _) = tray(Some(Mode::Manual));
        tray.gpus
            .insert(0, gpu("Integrated", true, false, "active"));
        tray.gpus
            .insert(1, gpu("Discrete", false, true, "suspended"));
        let submenu = tray.menu().into_iter().find_map(|item| match item {
            MenuItem::SubMenu(item) => Some(item),
            _ => None,
        });
        assert_eq!(submenu.unwrap().submenu.len(), 1);
    }

    #[test]
    fn blocked_gpu_checkmark_requests_unblock() {
        let (mut tray, mut actions) = tray(Some(Mode::Manual));
        tray.gpus
            .insert(1, gpu("Discrete", false, true, "suspended"));
        let checkmark = tray.menu().into_iter().find_map(|item| match item {
            MenuItem::SubMenu(submenu) => submenu.submenu.into_iter().find_map(|item| match item {
                MenuItem::Checkmark(checkmark) => Some(checkmark),
                _ => None,
            }),
            _ => None,
        });
        let checkmark = checkmark.unwrap();
        (checkmark.activate)(&mut tray);
        assert_eq!(
            actions.try_recv().unwrap(),
            TrayAction::SetGpuBlock {
                id: 1,
                blocked: false,
            }
        );
    }

    #[test]
    fn tooltip_reports_gpu_state() {
        let (mut tray, _) = tray(Some(Mode::Hybrid));
        tray.gpus
            .insert(0, gpu("Integrated", true, false, "active\n"));
        assert!(
            tray.tool_tip()
                .description
                .contains("Integrated | active | yes | no")
        );
    }
}
