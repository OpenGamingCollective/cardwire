use iced::{
    Alignment, Element, Length::{Fill, Fixed}, Subscription, Task, widget::{column, container, row, stack, text}, window
};
use log::error;
use std::collections::BTreeMap;

use crate::{
    gui_config::{GuiConfig, PrimaryClickAction}, helpers::{CardwireDbus, GpuDevice}, message::Message, models::{DaemonSettings, MainState, Mode, Page, PciDevice, SettingState}, tray::{self, TrayAction, TrayHandle}, ui::{self, daemon_setting_page, error_bar, info_bar, pci_page}
};

#[derive(Debug)]
pub struct AppState {
    pub current_tab: Page,
    pub error: Option<String>,
    pub info: Option<String>,
    pub zbus_conn: CardwireDbus,
    pub gpu_list: BTreeMap<usize, GpuDevice>,
    pub pci_list: BTreeMap<String, PciDevice>,
    pub main_state: MainState,
    pub setting_state: SettingState,
    window_id: Option<window::Id>,
    tray_handle: Option<TrayHandle>,
    tray_available: bool,
}

impl AppState {
    pub fn new() -> (Self, Task<Message>) {
        let (gui_config, error) = match GuiConfig::load() {
            Ok(config) => (config, None),
            Err(error) => (
                GuiConfig::default(),
                Some(format!("Could not load GUI settings: {error}")),
            ),
        };
        let (window_id, open_window) = if gui_config.start_in_tray {
            (None, Task::none())
        } else {
            let (id, task) = window::open(window::Settings::default());
            (Some(id), task.discard())
        };
        let state = AppState {
            current_tab: Page::default(),
            error,
            info: None,
            zbus_conn: CardwireDbus::new(),
            gpu_list: BTreeMap::default(),
            pci_list: BTreeMap::default(),
            main_state: MainState::default(),
            setting_state: SettingState {
                gui_config,
                ..SettingState::default()
            },
            window_id,
            tray_handle: None,
            tray_available: true,
        };
        (state, open_window)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let mut notification = None;
        let sync_tray = matches!(
            &message,
            Message::AllDevicesFetched(_)
                | Message::FetchedMode(_)
                | Message::TrayReady(_)
                | Message::UpdateGpuPowerState(..)
                | Message::UpdateBlockState(..)
                | Message::GpuBlockResult(_)
        );
        match message {
            // Switch to a new page, clearing the pop-ups at the same time
            Message::SwitchPage(page) => {
                self.current_tab = page;
                self.error = None;
                self.info = None;
            }
            // Happen when a gpu_list is received from dbus
            Message::AllDevicesFetched(res) => match res {
                Ok(map) => {
                    self.gpu_list = map;
                    // Clear error
                    self.error = None;
                }
                Err(err) => {
                    self.gpu_list.clear();
                    self.error = Some(format!("Error fetching GPUs: {}", err));
                }
            },
            // Happen when a mode is received from dbus
            Message::FetchedMode(mode) => match mode {
                Ok(mode) => {
                    notification = mode_change_notification(self.main_state.current_mode, mode);
                    self.main_state.current_mode = Some(mode);
                    self.error = None;
                }
                Err(err) => {
                    self.main_state.current_mode = None;
                    self.error = Some(format!("Error fetching Mode: {}", err));
                }
            },
            // Send the new mode to dbus
            Message::SetMode(mode) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_mode(mode.into())
                            .await
                            .map_err(|e| e.to_string())?;
                        conn.get_mode().await.map_err(|e| e.to_string())
                    },
                    |res| match res {
                        Ok(val) => match Mode::from_repr(val) {
                            Some(m) => Message::FetchedMode(Ok(m)),
                            None => Message::FetchedMode(Err(format!("Unknown mode: {}", val))),
                        },
                        Err(err) => Message::FetchedMode(Err(err)),
                    },
                );
            }
            // Used to update the exp nvidia setting and send it to dbus
            Message::UpdateNvidiaSetting(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(DaemonSettings::ExpNvidiaBlock, setting, None)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((DaemonSettings::ExpNvidiaBlock, Some(setting), None::<Mode>))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            // Used to update auto apply state and send it to dbus
            Message::UpdateStateSetting(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(DaemonSettings::AutoApplyGpuState, setting, None)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((
                            DaemonSettings::AutoApplyGpuState,
                            Some(setting),
                            None::<Mode>,
                        ))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            // Used to update the auto switch on battery and send it to dbus
            Message::UpdateBatterySetting(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(DaemonSettings::BattAutoSwitch, setting, None)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((DaemonSettings::BattAutoSwitch, Some(setting), None::<Mode>))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            // Used to update the auto battery switch's Mode and send it to dbus
            Message::UpdateBatteryMode(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(DaemonSettings::BattAutoSwitchMode, false, Some(setting))
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((DaemonSettings::BattAutoSwitchMode, None, Some(setting)))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            Message::UpdateExternalDisplaySetting(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(DaemonSettings::ExternalDisplayAutoSwitch, setting, None)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((
                            DaemonSettings::ExternalDisplayAutoSwitch,
                            Some(setting),
                            None::<Mode>,
                        ))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            Message::UpdateExternalDisplayMode(setting) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_setting(
                            DaemonSettings::ExternalDisplayAutoSwitchMode,
                            false,
                            Some(setting),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        Ok((
                            DaemonSettings::ExternalDisplayAutoSwitchMode,
                            None,
                            Some(setting),
                        ))
                    },
                    |res| match res {
                        Ok(res) => Message::FetchedSetting(Ok(res)),
                        Err(err) => Message::FetchedSetting(Err(err)),
                    },
                );
            }
            Message::UpdateGuiConfig(config) => return self.save_gui_config(config),
            Message::TrayReady(handle) => {
                self.tray_handle = Some(handle);
                self.tray_available = true;
            }
            Message::TrayAction(action) => match action {
                TrayAction::PrimaryClick => {
                    match self.setting_state.gui_config.primary_click_action {
                        PrimaryClickAction::SwitchMode => {
                            return match configured_primary_click_mode(
                                self.main_state.current_mode,
                                &self.setting_state.gui_config,
                            ) {
                                Ok(mode) => self.update(Message::SetMode(mode)),
                                Err(error) => {
                                    self.error = Some(error);
                                    Task::none()
                                }
                            };
                        }
                        PrimaryClickAction::OpenGui => return self.open_or_focus_window(),
                    }
                }
                TrayAction::SetMode(mode) => return self.update(Message::SetMode(mode)),
                TrayAction::SetGpuBlock { id, blocked } => {
                    return self.update(Message::SetGpuBlock(id, blocked));
                }
                TrayAction::OpenGui => return self.open_or_focus_window(),
                TrayAction::Quit => {
                    if let Some(handle) = self.tray_handle.take() {
                        return Task::perform(handle.shutdown(), |_| Message::TrayShutdownComplete);
                    }
                    return iced::exit();
                }
            },
            Message::TrayUnavailable(error) => {
                self.tray_available = false;
                self.tray_handle = None;
                self.error = Some(format!("Tray applet unavailable: {error}"));
                if self.window_id.is_none() {
                    return self.open_or_focus_window();
                }
            }
            Message::TrayShutdownComplete => return iced::exit(),
            Message::WindowClosed(id) => {
                if self.window_id == Some(id) {
                    self.window_id = None;
                }
                if !self.tray_available {
                    return iced::exit();
                }
            }
            // Update the gpu_list power_state using the gpu key id
            Message::UpdateGpuPowerState(id, new_state) => {
                // Get the gpu by key
                if let Some(gpu) = self.gpu_list.get_mut(&id) {
                    // Update the gpu power_state
                    gpu.power_state = Some(new_state);
                } else {
                    let error = format!("UPDATE_GPU_POWER_STATE: Couldn't get gpu key: {}", id);
                    error!("{}", error);
                    self.error = Some(error);
                }
            }
            // Same but with the block state
            Message::UpdateBlockState(id, new_state) => {
                if let Some(gpu) = self.gpu_list.get_mut(&id) {
                    // Update the gpu power_state
                    gpu.blocked = new_state;
                } else {
                    let error = format!("UPDATE_BLOCK_STATE: Couldn't get gpu key: {}", id);
                    error!("{}", error);
                    self.error = Some(error);
                }
            }
            // Happen when a setting is updated and received from dbus
            Message::FetchedSetting(res) => match res {
                Ok(val) => {
                    match val.0 {
                        DaemonSettings::ExpNvidiaBlock => {
                            if let Some(new_val) = val.1 {
                                self.setting_state.nvidia_checked = new_val
                            }
                        }
                        DaemonSettings::AutoApplyGpuState => {
                            if let Some(new_val) = val.1 {
                                self.setting_state.state_checked = new_val
                            }
                        }
                        DaemonSettings::BattAutoSwitch => {
                            if let Some(new_val) = val.1 {
                                self.setting_state.battery_checked = new_val
                            }
                        }
                        DaemonSettings::BattAutoSwitchMode => {
                            if let Some(new_mode) = val.2 {
                                self.setting_state.battery_mode = Some(new_mode)
                            }
                        }
                        DaemonSettings::ExternalDisplayAutoSwitch => {
                            if let Some(new_val) = val.1 {
                                self.setting_state.external_display_checked = new_val
                            }
                        }
                        DaemonSettings::ExternalDisplayAutoSwitchMode => {
                            if let Some(new_mode) = val.2 {
                                self.setting_state.external_display_mode = Some(new_mode)
                            }
                        }
                    }
                    self.error = None;
                }
                Err(err) => self.error = Some(format!("error fetching Setting: {}", err)),
            },
            // Fetched pci_list from dbus
            Message::FetchedPciList(pci_list) => {
                self.pci_list = pci_list;
            }
            // Copy the pci list to the clipboard
            Message::PciListToClipboard() => match serde_json::to_string_pretty(&self.pci_list) {
                Ok(json) => {
                    self.info = Some("Copied pci_list to clipboard!".to_string());
                    return iced::clipboard::write(json);
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            // Toggle the dropdown menu in the main page
            Message::ToggleMenu(index) => {
                self.main_state.open_gpu_menu = index;
            }
            // Block/Unblock a GPU
            Message::SetGpuBlock(id, blocked) => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move {
                        conn.set_gpu_block(id as u32, blocked)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok((id, blocked))
                    },
                    Message::GpuBlockResult,
                );
            }
            Message::GpuBlockResult(res) => match res {
                Ok((id, blocked)) => {
                    if let Some(gpu) = self.gpu_list.get_mut(&id) {
                        gpu.blocked = blocked;
                    }
                    self.info = Some(format!(
                        "GPU {} {}",
                        id,
                        if blocked { "blocked" } else { "unblocked" }
                    ));
                    self.error = None;
                }
                Err(err) => self.error = Some(format!("Block error: {}", err)),
            },
            Message::RequestLsof(id) => {
                self.main_state.open_gpu_menu = None;
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move { conn.lsof(id as u32).await.map_err(|e| e.to_string()) },
                    Message::LsofResult,
                );
            }
            Message::LsofResult(res) => match res {
                Ok(data) => {
                    self.main_state.lsof_window = Some(data);
                    self.error = None;
                }
                Err(err) => self.error = Some(format!("Lsof error: {}", err)),
            },
            Message::CloseLsofWindow => {
                self.main_state.lsof_window = None;
            }
            Message::RefreshGpu => {
                let conn = self.zbus_conn.clone();
                return Task::perform(
                    async move { conn.refresh_gpu().await.map_err(|e| e.to_string()) },
                    Message::RefreshGpuResult,
                );
            }
            Message::RefreshGpuResult(res) => match res {
                Ok(()) => self.info = Some("GPU list refreshed".to_string()),
                Err(err) => self.error = Some(format!("Refresh error: {}", err)),
            },
            Message::OpenUrl(url) => {
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            }
            Message::None => return Task::none(),
            Message::ClearError => self.error = None,
            Message::ClearInfo => self.info = None,
        }
        let sync_task = if sync_tray {
            self.sync_tray()
        } else {
            Task::none()
        };
        if let Some(message) = notification {
            Task::batch([
                sync_task,
                Task::perform(tray::notify(message), |_| Message::None),
            ])
        } else {
            sync_task
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            crate::subscription::dbus_sub(),
            crate::subscription::tray_sub(),
            window::close_events().map(Message::WindowClosed),
        ])
    }

    fn save_gui_config(&mut self, config: GuiConfig) -> Task<Message> {
        match config.save() {
            Ok(()) => {
                self.setting_state.gui_config = config;
                self.info = Some("GUI settings saved".to_string());
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Could not save GUI settings: {error}")),
        }
        Task::none()
    }

    fn sync_tray(&self) -> Task<Message> {
        let Some(handle) = self.tray_handle.clone() else {
            return Task::none();
        };
        let mode = self.main_state.current_mode;
        let gpus = self.gpu_list.clone();
        Task::perform(tray::update(handle, mode, gpus), |_| Message::None)
    }

    fn open_or_focus_window(&mut self) -> Task<Message> {
        if let Some(id) = self.window_id {
            window::gain_focus(id)
        } else {
            let (id, task) = window::open(window::Settings::default());
            self.window_id = Some(id);
            task.discard()
        }
    }

    pub fn view(&self, _window_id: window::Id) -> Element<'_, Message> {
        let mut main_content = column![].spacing(10).width(Fill).height(Fill);
        main_content = main_content.push(container(match &self.current_tab {
            Page::Main => ui::main_page(&self.main_state, &self.gpu_list),
            Page::Pci => pci_page(&self.pci_list),
            Page::SmartMode => text("Smart Mode TODO").into(),
            Page::CardwireSettings => daemon_setting_page(&self.setting_state),
            Page::AccessLogs => text!("TODO").into(),
            Page::Advanced => ui::advanced_page(),
            Page::About => ui::about_page(),
        }));
        let styled_main_content = main_content
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .padding(30);

        let mut content_stack = stack!(styled_main_content).width(Fill).height(Fill);

        if let Some(err) = &self.error {
            content_stack = content_stack.push(error_bar(err));
        }
        if let Some(info) = &self.info {
            content_stack = content_stack.push(info_bar(info));
        }
        if let Some(lsof_data) = &self.main_state.lsof_window {
            content_stack = content_stack.push(ui::lsof_overlay(lsof_data, &self.gpu_list));
        }

        let final_app = row![
            container(ui::page_bar(self.current_tab))
                .width(Fixed(200.0))
                .height(Fill)
                .style(container::rounded_box)
                .padding(5),
            content_stack
        ];
        final_app.into()
    }

    pub fn title(&self, _window_id: window::Id) -> String {
        format!("Cardwire - {}", self.current_tab)
    }
}

fn configured_primary_click_mode(
    current: Option<Mode>,
    config: &GuiConfig,
) -> Result<Mode, String> {
    current
        .map(|mode| config.next_primary_click_mode(mode))
        .ok_or_else(|| "Cardwire daemon is unavailable".to_string())
}

fn mode_change_notification(current: Option<Mode>, next: Mode) -> Option<String> {
    (current.is_some() && current != Some(next)).then(|| format!("Switched to {next} mode"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_click_uses_the_configured_modes() {
        assert_eq!(
            configured_primary_click_mode(Some(Mode::Integrated), &GuiConfig::default()).unwrap(),
            Mode::Hybrid
        );
    }

    #[test]
    fn primary_click_rejects_an_offline_daemon() {
        assert!(configured_primary_click_mode(None, &GuiConfig::default()).is_err());
    }

    #[test]
    fn only_actual_mode_changes_are_notified() {
        assert_eq!(mode_change_notification(None, Mode::Hybrid), None);
        assert_eq!(
            mode_change_notification(Some(Mode::Hybrid), Mode::Hybrid),
            None
        );
        assert_eq!(
            mode_change_notification(Some(Mode::Integrated), Mode::Hybrid).as_deref(),
            Some("Switched to Hybrid mode")
        );
    }
}
