//! `wpe-gnomectl`: inspect the wallpaper library and drive the daemon from a terminal.

mod commands;

use std::process::ExitCode;

use clap::Parser;
use clap::Subcommand;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "wpe-gnomectl",
    about = "Wallpaper Engine wallpapers on GNOME",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List every wallpaper found in the Steam libraries and the Wallpaper Engine install.
    List {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Only show wallpapers of this type (scene, video, web, application).
        #[arg(long)]
        kind: Option<String>,
    },
    /// Show one wallpaper with its properties.
    Show {
        /// A wallpaper id such as `workshop:2954387038` or `default:beach`.
        id: String,
    },
    /// Show the Steam libraries and directories that were found.
    Paths,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let result = match cli.command {
        Command::List { json, kind } => commands::list(json, kind.as_deref()),
        Command::Show { id } => commands::show(&id),
        Command::Paths => commands::paths(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");

            ExitCode::FAILURE
        }
    }
}
