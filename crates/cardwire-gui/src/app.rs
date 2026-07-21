use iced::{
    Alignment, Element, Length::{Fill, Fixed}, Task, widget::{column, container, row, text}
};
use std::collections::BTreeMap;

use crate::{
    helpers::{CardwireDbus, GpuDevice}, message::Message, models::{MainState, Mode, Page}, ui::{self, error_bar}
};

#[derive(Debug)]
pub struct AppState {
    pub current_tab: Page,
    pub error: Option<String>,
    pub zbus_conn: CardwireDbus,
    pub gpu_list: BTreeMap<usize, GpuDevice>,
    pub main_state: MainState,
}

impl AppState {
    pub fn new() -> (Self, Task<Message>) {
        let initial_state = AppState {
            current_tab: Page::default(),
            error: None,
            zbus_conn: CardwireDbus::new(),
            gpu_list: BTreeMap::default(),
            main_state: MainState::default(),
        };

        let conn_gpus = initial_state.zbus_conn.clone();
        let gpu_task =
            Task::perform(
                async move { conn_gpus.get_devices_list().await },
                |res| match res {
                    Ok(device) => Message::AllDevicesFetched(Ok(device)),
                    Err(err) => Message::AllDevicesFetched(Err(err.to_string())),
                },
            );

        let conn_mode = initial_state.zbus_conn.clone();
        let mode_task = Task::perform(async move { conn_mode.get_mode().await }, |res| match res {
            Ok(val) => match Mode::from_repr(val) {
                Some(m) => Message::FetchedMode(Ok(m)),
                None => Message::FetchedMode(Err(format!("Unknown mode: {}", val))),
            },
            Err(err) => Message::FetchedMode(Err(err.to_string())),
        });

        (initial_state, Task::batch([gpu_task, mode_task]))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchPage(page) => {
                self.current_tab = page;
                self.error = None;

                if page == Page::Main {
                    // We fetch the mode and the list again to prevent outdated datas
                    let conn_mode = self.zbus_conn.clone();
                    let mode_task =
                        Task::perform(async move { conn_mode.get_mode().await }, |res| match res {
                            Ok(val) => match Mode::from_repr(val) {
                                Some(m) => Message::FetchedMode(Ok(m)),
                                None => Message::FetchedMode(Err(format!("Unknown mode: {}", val))),
                            },
                            Err(err) => Message::FetchedMode(Err(err.to_string())),
                        });

                    let conn_gpus = self.zbus_conn.clone();
                    let gpu_task =
                        Task::perform(async move { conn_gpus.get_devices_list().await }, |res| {
                            match res {
                                Ok(devices) => Message::AllDevicesFetched(Ok(devices)),
                                Err(err) => Message::AllDevicesFetched(Err(err.to_string())),
                            }
                        });
                    return Task::batch([mode_task, gpu_task]);
                }
            }
            Message::AllDevicesFetched(res) => match res {
                Ok(map) => {
                    self.gpu_list = map;
                    // Clear error
                    self.error = None;
                }
                Err(err) => self.error = Some(format!("Error fetching GPUs: {}", err)),
            },
            Message::FetchedMode(mode) => match mode {
                Ok(mode) => self.main_state.current_mode = Some(mode),
                Err(err) => self.error = Some(format!("Error fetching Mode: {}", err)),
            },
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
            Message::ClearError => self.error = None,
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut app = column![].spacing(10).width(Fill).height(Fill);
        if let Some(err) = &self.error {
            app = app.push(error_bar(err))
        }
        app = app.push(container(match &self.current_tab {
            Page::Main => ui::main_page(&self.main_state, &self.gpu_list),
            Page::SmartMode => text("Smart Mode TODO").into(),
            Page::CardwireSettings => text!("TODO").into(),
            Page::AccessLogs => text!("TODO").into(),
            Page::About => ui::about_page(),
        }));
        row![
            container(ui::page_bar())
                .width(Fixed(200.0))
                .height(Fill)
                .style(container::rounded_box)
                .padding(5),
            app.width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .padding(30)
        ]
        .spacing(15)
        .into()
    }

    pub fn title(&self) -> String {
        format!("Cardwire - {}", self.current_tab)
    }
}
