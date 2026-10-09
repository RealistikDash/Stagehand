//! `wpe-gnomectl`: inspect the wallpaper library and drive the daemon from a terminal.

mod commands;
mod daemon;

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
    /// Show the daemon's renderer slots and the Shell extension state.
    Status,
    /// Show the monitors the daemon sees, with their assignment keys.
    Monitors,
    /// Apply a wallpaper to a monitor (by key from `monitors`) or, with --span, across all of them.
    Apply {
        /// A wallpaper id such as `workshop:2954387038`.
        wallpaper: String,
        /// The monitor key, e.g. `edid:SAM/Odyssey G85SB/H1AK500000`.
        #[arg(long, conflicts_with = "span")]
        monitor: Option<String>,
        /// Span the wallpaper across every monitor.
        #[arg(long)]
        span: bool,
        /// Scaling mode for this slot: fill, fit, stretch or centre.
        #[arg(long)]
        scaling: Option<String>,
    },
    /// Remove every assignment and stop all renderers.
    Stop,
    /// Ask the daemon to rescan the Steam directories.
    Rescan,
    /// Print the daemon's configuration as JSON.
    Config,
    /// Put the original GNOME background keys back.
    RestoreBackground,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::List { json, kind } => commands::list(json, kind.as_deref()).map_err(Into::into),
        Command::Show { id } => commands::show(&id).map_err(Into::into),
        Command::Paths => commands::paths().map_err(Into::into),
        Command::Status => daemon::status().map_err(Into::into),
        Command::Monitors => daemon::monitors().map_err(Into::into),
        Command::Apply { wallpaper, monitor, span, scaling } => {
            daemon::apply(&wallpaper, monitor.as_deref(), span, scaling.as_deref()).map_err(Into::into)
        }
        Command::Stop => daemon::stop().map_err(Into::into),
        Command::Rescan => daemon::rescan().map_err(Into::into),
        Command::Config => daemon::config().map_err(Into::into),
        Command::RestoreBackground => daemon::restore_background().map_err(Into::into),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");

            ExitCode::FAILURE
        }
    }
}
