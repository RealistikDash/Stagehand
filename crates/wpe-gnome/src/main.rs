//! `wpe-gnome`: the GTK4/libadwaita settings application. A pure client of the daemon's D-Bus
//! API; closing it never affects the wallpaper.

mod autostart;
mod client;
mod detail;
mod gallery;
mod monitors;
mod preferences;
mod preview;
mod window;

use std::process::ExitCode;

use gtk4::prelude::*;
use libadwaita as adw;
use tracing_subscriber::EnvFilter;
use wpe_gnome_ipc::names;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let application = adw::Application::builder()
        .application_id(names::UI_APP_ID)
        .build();

    application.connect_activate(|application| {
        if let Some(existing) = application.active_window() {
            existing.present();

            return;
        }

        window::present(application);
    });

    let status = application.run();

    if status == glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
