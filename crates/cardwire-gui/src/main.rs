mod app;
mod helpers;
mod message;
mod models;
mod subscription;
mod ui;

use app::AppState;
use env_logger::Env;

fn main() -> iced::Result {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    // This prevent iced from using the dGPU, will be fixed in the next iced release
    unsafe {
        std::env::set_var("WGPU_BACKEND", "gl");
        std::env::set_var("WGPU_POWER_PREF", "low");
    }

    iced::application(AppState::new, AppState::update, AppState::view)
        .title(AppState::title)
        .subscription(|_| subscription::dbus_sub())
        .run()
}
