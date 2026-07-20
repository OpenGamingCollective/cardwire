mod core;

use iced::{
    self, Color, Element, Task, widget::{self, Column, Row, button, column, row, text}
};
use std::fmt::Display;
use strum::{EnumIter, IntoEnumIterator};

use crate::core::{CardwireDbus, GpuDevice};

#[derive(Debug)]
struct AppState {
    current_tab: Page,
    zbus_conn: Option<CardwireDbus>,
}

impl AppState {
    fn new() -> (Self, Task<Message>) {
        let initial_state = AppState {
            current_tab: Page::default(),
            zbus_conn: None,
        };

        let initial_task = Task::perform(CardwireDbus::new(), Message::DbusConnected);
        (initial_state, initial_task)
    }

    fn view(&self) -> Element<'_, Message> {
        let row: Row<'_, Message> = row![
            self.page_bar(),
            match &self.current_tab {
                Page::Main => GpuManagement::default().view(),
                Page::About => about(),
            }
            .explain(Color::BLACK)
        ]
        .spacing(15);
        row.into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchPage(page) => {
                self.current_tab = page;
                println!("switching to {}", page);
            }
            Message::DbusConnected(conn) => {
                self.zbus_conn = Some(conn);
                println!("DBUS Connected successfully!");
            }
            Message::GetDevice(device_id) => {
                if let Some(conn) = self.zbus_conn.clone() {
                    return Task::perform(async move { conn.get_device(device_id).await }, |res| {
                        match res {
                            Ok(device) => Message::DeviceFetched(Ok(device)),
                            Err(err) => Message::DeviceFetched(Err(err.to_string())),
                        }
                    });
                }
            }
            Message::DeviceFetched(Ok(device)) => {
                println!("Got device: {:?}", device);
            }
            Message::DeviceFetched(Err(err_msg)) => {
                println!("Failed to get device: {}", err_msg);
            }
        }

        Task::none()
    }

    fn title(&self) -> String {
        format!("Cardwire - {}", self.current_tab)
    }

    fn page_bar(&self) -> widget::Column<'_, Message> {
        let mut column: widget::Column<'_, Message> = column![];
        for page in Page::iter() {
            let label = text!("{}", page);
            column = column.push(button(label).on_press(Message::SwitchPage(page)));
        }
        column
    }
}

#[derive(Debug, Clone, Copy, EnumIter, Default)]
enum Page {
    #[default]
    Main,
    About,
}

impl Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Page::Main => write!(f, "Main"),
            Page::About => write!(f, "About"),
        }
    }
}

fn about() -> iced::Element<'static, Message> {
    text("Made by luytan").into()
}

#[derive(Debug, Clone)]
struct GpuManagement {
    gpu_list: Vec<u32>,
}

impl Default for GpuManagement {
    fn default() -> Self {
        GpuManagement {
            gpu_list: vec![0, 1],
        }
    }
}

impl GpuManagement {
    fn view(self) -> iced::Element<'static, Message> {
        let mut gpu_table: Column<'_, Message> = column![];
        for gpu in &self.gpu_list {
            let gpu_row = text!("Fetch GPU {}", gpu);
            let gpu_btn = button(gpu_row).on_press(Message::GetDevice(*gpu));
            gpu_table = gpu_table.push(gpu_btn);
        }
        gpu_table.into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    DbusConnected(CardwireDbus),
    GetDevice(u32),
    DeviceFetched(Result<GpuDevice, String>),

    SwitchPage(Page),
}

fn main() -> iced::Result {
    iced::application(AppState::new, AppState::update, AppState::view)
        .title(AppState::title)
        .run()
}
