//! `wpe-gnome-renderer`: one GTK window showing a video or a web wallpaper. The daemon sizes it,
//! the Shell extension pins it, and commands arrive as JSON lines on stdin.

mod content;
mod stdin;
mod video;
mod web;
mod window;

use std::process::ExitCode;

use clap::Parser;
use gtk4::prelude::*;
use tracing_subscriber::EnvFilter;
use wpe_gnome_core::protocol::RendererArguments;

const APP_ID: &str = "io.github.realistik.WpeGnome.Renderer";

#[derive(Debug, Parser)]
#[command(name = "wpe-gnome-renderer", about = "Renders one wpe-gnome wallpaper window")]
struct Cli {
    /// The `RendererArguments` JSON produced by the daemon.
    #[arg(long)]
    arguments: String,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let arguments: RendererArguments = match serde_json::from_str(&cli.arguments) {
        Ok(arguments) => arguments,
        Err(error) => {
            tracing::error!(error = %error, "Invalid renderer arguments.");

            return ExitCode::FAILURE;
        }
    };
    let application = gtk4::Application::new(Some(APP_ID), gio::ApplicationFlags::NON_UNIQUE);

    application.connect_activate(move |application| {
        if let Err(error) = window::build(application, &arguments) {
            tracing::error!(error = %error, "Could not build renderer window.");
            application.quit();
        }
    });

    // GTK must not see our arguments; they were consumed by clap.
    let status = application.run_with_args::<&str>(&[]);

    if status == glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
