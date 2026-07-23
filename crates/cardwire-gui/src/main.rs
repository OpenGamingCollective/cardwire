mod app;
mod helpers;
mod message;
mod models;
mod ui;

use app::AppState;

fn main() -> iced::Result {
    iced::application(AppState::new, AppState::update, AppState::view)
        .title(AppState::title)
        .run()
}
