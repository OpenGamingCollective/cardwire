mod app;
mod gtk_font;
mod gui_config;
mod helpers;
mod message;
mod models;
mod single_instance;
mod subscription;
mod tray;
mod ui;

use app::AppState;
use env_logger::Env;
use log::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    // Keep alive for the process's lifetime: dropping it releases the D-Bus name and a
    // subsequent launch would no longer detect this instance as running.
    let _single_instance_guard = match single_instance::acquire() {
        single_instance::Acquisition::Acquired(connection) => connection,
        single_instance::Acquisition::AlreadyRunning => {
            info!("cardwire-gui is already running; exiting");
            return Ok(());
        }
        single_instance::Acquisition::Unchecked => {
            return Err("could not determine whether cardwire-gui is already running".into());
        }
    };

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
        .run()?;

    Ok(())
}
