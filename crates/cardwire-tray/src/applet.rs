use std::collections::BTreeMap;

use ksni::{MenuItem, Tray};
use tokio::sync::mpsc;

use crate::config::TrayMode;

const ICON_PREFIX: &str = "com.github.opengamingcollective.cardwire.tray";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub id: u32,
    pub name: String,
    pub default: bool,
    pub blocked: bool,
    pub power_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    ToggleConfiguredMode,
    SetMode(TrayMode),
    SetGpuBlock { id: u32, blocked: bool },
    OpenGui,
    Quit,
}

pub struct CardwireTray {
    pub online: bool,
    pub mode: Option<TrayMode>,
    pub gpus: BTreeMap<u32, GpuInfo>,
    pub action_tx: mpsc::UnboundedSender<TrayAction>,
}

impl CardwireTray {
    pub fn offline(action_tx: mpsc::UnboundedSender<TrayAction>) -> Self {
        Self {
            online: false,
            mode: None,
            gpus: BTreeMap::new(),
            action_tx,
        }
    }

    fn icon(mode: TrayMode) -> String {
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
        if !self.online {
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
                gpu.power_state.trim(),
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

        if self.online {
            let options = TrayMode::ALL
                .into_iter()
                .map(|mode| ksni::menu::RadioItem {
                    label: format!("{mode} Mode"),
                    icon_name: Self::icon(mode),
                    ..Default::default()
                })
                .collect();
            // `TrayMode::value` deliberately matches the order of `TrayMode::ALL`,
            // which lets the D-Bus value double as the radio-group index.
            items.push(
                ksni::menu::RadioGroup {
                    selected: self.mode.map_or(0, |mode| mode.value() as usize),
                    select: Box::new(|tray: &mut Self, index| {
                        if let Some(mode) = TrayMode::from_value(index as u32) {
                            let _ = tray.action_tx.send(TrayAction::SetMode(mode));
                        }
                    }),
                    options,
                }
                .into(),
            );

            if self.mode == Some(TrayMode::Manual) {
                let gpu_items = self
                    .gpus
                    .values()
                    // The daemon protects the default GPU as well, but omitting it
                    // here prevents the tray from offering an unsafe action at all.
                    .filter(|gpu| !gpu.default)
                    .map(|gpu| {
                        let id = gpu.id;
                        let blocked = gpu.blocked;
                        ksni::menu::CheckmarkItem {
                            label: gpu.name.clone(),
                            checked: blocked,
                            activate: Box::new(move |tray: &mut Self| {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tray(mode: Option<TrayMode>) -> (CardwireTray, mpsc::UnboundedReceiver<TrayAction>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut tray = CardwireTray::offline(tx);
        tray.online = mode.is_some();
        tray.mode = mode;
        (tray, rx)
    }

    #[test]
    fn primary_activation_requests_configured_toggle() {
        let (mut tray, mut actions) = tray(Some(TrayMode::Integrated));
        tray.activate(0, 0);
        assert_eq!(
            actions.try_recv().unwrap(),
            TrayAction::ToggleConfiguredMode
        );
    }

    #[test]
    fn offline_menu_disables_mutations() {
        let (tray, _) = tray(None);
        let menu = tray.menu();
        assert!(
            !menu
                .iter()
                .any(|item| matches!(item, MenuItem::RadioGroup(_)))
        );
    }

    #[test]
    fn manual_menu_only_lists_non_default_gpus() {
        let (mut tray, _) = tray(Some(TrayMode::Manual));
        for gpu in [
            GpuInfo {
                id: 0,
                name: "Integrated".to_string(),
                default: true,
                blocked: false,
                power_state: "active".to_string(),
            },
            GpuInfo {
                id: 1,
                name: "Discrete".to_string(),
                default: false,
                blocked: true,
                power_state: "suspended".to_string(),
            },
        ] {
            tray.gpus.insert(gpu.id, gpu);
        }
        let submenu = tray.menu().into_iter().find_map(|item| match item {
            MenuItem::SubMenu(item) => Some(item),
            _ => None,
        });
        let submenu = submenu.unwrap();
        assert_eq!(submenu.label, "Blocked GPUs");
        assert_eq!(submenu.submenu.len(), 1);
    }

    #[test]
    fn blocked_gpu_checkmark_requests_unblock() {
        let (mut tray, mut actions) = tray(Some(TrayMode::Manual));
        tray.gpus.insert(
            1,
            GpuInfo {
                id: 1,
                name: "Discrete".to_string(),
                default: false,
                blocked: true,
                power_state: "suspended".to_string(),
            },
        );
        let checkmark = tray.menu().into_iter().find_map(|item| match item {
            MenuItem::SubMenu(submenu) => submenu.submenu.into_iter().find_map(|item| match item {
                MenuItem::Checkmark(checkmark) => Some(checkmark),
                _ => None,
            }),
            _ => None,
        });
        let checkmark = checkmark.unwrap();
        assert!(checkmark.checked);
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
        let (mut tray, _) = tray(Some(TrayMode::Hybrid));
        tray.gpus.insert(
            0,
            GpuInfo {
                id: 0,
                name: "Integrated".to_string(),
                default: true,
                blocked: false,
                power_state: "active\n".to_string(),
            },
        );
        let tooltip = tray.tool_tip();
        assert!(
            tooltip
                .description
                .contains("Integrated | active | yes | no")
        );
    }
}
