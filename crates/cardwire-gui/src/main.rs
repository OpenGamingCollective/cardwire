mod app;
mod args;
mod gtk_font;
mod gui_config;
mod helpers;
mod message;
mod models;
mod subscription;
mod tray;
mod ui;

use app::AppState;
use args::CardwireArgs;
use clap::Parser;
use env_logger::Env;
use helpers::AppInstance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    let args = CardwireArgs::parse();

    unsafe {
        // Vulkan wakes the dGPU
        std::env::set_var("WGPU_BACKEND", "gl");
        // This prevent iced from using the dGPU, will be fixed in the next iced release
        std::env::set_var("WGPU_POWER_PREF", "low");
    }

    // Keep D-Bus processing alive while Iced runs its event loop on this thread.
    let runtime = tokio::runtime::Runtime::new()?;
    let Some(instance) = runtime.block_on(AppInstance::acquire(args.background != Some(true)))?
    else {
        return Ok(());
    };

    iced::daemon(
        move || AppState::new(&args),
        AppState::update,
        AppState::view,
    )
    .title(AppState::title)
    .theme(iced::Theme::Dark)
    .subscription(move |state: &AppState| state.subscription(&instance))
    .default_font(gtk_font::default_font())
    .run()?;
    Ok(())
}
