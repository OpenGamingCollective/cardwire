use iced::{
    Alignment, Element, Length::{Fill, Fixed}, Task, widget::{column, container, row, stack, text}
};
use log::error;
use std::collections::BTreeMap;

use crate::{
    helpers::{CardwireDbus, GpuDevice}, message::Message, models::{DaemonSettings, MainState, Mode, Page, PciDevice, SettingState}, ui::{self, daemon_setting_page, error_bar, info_bar, pci_page}
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
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            current_tab: Page::default(),
            error: None,
            info: None,
            zbus_conn: CardwireDbus::new(),
            gpu_list: BTreeMap::default(),
            pci_list: BTreeMap::default(),
            main_state: MainState::default(),
            setting_state: SettingState::default(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                Err(err) => self.error = Some(format!("Error fetching GPUs: {}", err)),
            },
            // Happen when a mode is received from dbus
            Message::FetchedMode(mode) => match mode {
                Ok(mode) => {
                    self.main_state.current_mode = Some(mode);
                    self.error = None;
                }
                Err(err) => self.error = Some(format!("Error fetching Mode: {}", err)),
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
            Message::None => {}
            Message::ClearError => self.error = None,
            Message::ClearInfo => self.info = None,
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
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
            container(ui::page_bar())
                .width(Fixed(200.0))
                .height(Fill)
                .style(container::rounded_box)
                .padding(5),
            content_stack
        ];
        final_app.into()
    }

    pub fn title(&self) -> String {
        format!("Cardwire - {}", self.current_tab)
    }
}
