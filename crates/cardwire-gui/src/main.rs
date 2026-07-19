use std::{fmt::Display, process::exit};

use iced::{
    self, Element, Task, widget::{self, Container, button, column, row, text}, window
};

#[derive(Debug)]
struct AppState {
    current_tab: Page,
}
impl AppState {
    fn new() -> Self {
        AppState {
            current_tab: Page::Main,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let mut content: widget::Row<'_, Message> =
            row![text!("Current page: {}", self.current_tab)];
        content = content.push(self.page_bar());
        content.into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Exit => window::latest().and_then(|id| window::close(id)),
        }
    }
    fn page_bar(&self) -> widget::Column<'_, Message> {
        let mut column: widget::Column<'_, Message> = column![];
        let text = text!("{}", Page::Main);
        column = column.push(button(text));
        column
    }
}
#[derive(Debug, Clone, Copy)]
enum Page {
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

#[derive(Debug, Clone, Copy)]
enum Message {
    Exit,
}

fn main() -> iced::Result {
    iced::application(AppState::new, AppState::update, AppState::view)
        .title("Cardwire")
        .run()
}
