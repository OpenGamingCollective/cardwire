use std::{
    collections::{BTreeMap, HashMap}, fmt
};

use ksni::{MenuItem, Tray, TrayMethods};
use strum::VariantArray;
use tokio::sync::mpsc;

use crate::{helpers::GpuDevice, models::Mode};

const ICON_PREFIX: &str = "org.opengamingcollective.cardwire.tray";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    PrimaryClick,
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
        let _ = self.action_tx.send(TrayAction::PrimaryClick);
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

pub async fn notify(message: String) {
    let Ok(connection) = zbus::Connection::session().await else {
        return;
    };
    let _ = connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Cardwire",
                0_u32,
                ICON_PREFIX,
                "Cardwire",
                message,
                Vec::<String>::new(),
                HashMap::<String, zbus::zvariant::OwnedValue>::new(),
                -1_i32,
            ),
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn primary_activation_requests_primary_click() {
        let (mut tray, mut actions) = tray(Some(Mode::Integrated));
        tray.activate(0, 0);
        assert_eq!(actions.try_recv().unwrap(), TrayAction::PrimaryClick);
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
