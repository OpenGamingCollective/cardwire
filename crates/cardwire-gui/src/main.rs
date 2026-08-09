mod app;
mod gtk_font;
mod gui_config;
mod helpers;
mod message;
mod models;
mod subscription;
mod tray;
mod ui;

use app::AppState;
use env_logger::Env;

fn main() -> iced::Result {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    unsafe {
        // Vulkan wakes the dGPU
        std::env::set_var("WGPU_BACKEND", "gl");
        // This prevent iced from using the dGPU, will be fixed in the next iced release
        std::env::set_var("WGPU_POWER_PREF", "low");
    }

    iced::daemon(AppState::new, AppState::update, AppState::view)
        .title(AppState::title)
        .theme(iced::Theme::Dark)
        .subscription(AppState::subscription)
        .default_font(gtk_font::default_font())
        .run()
}
