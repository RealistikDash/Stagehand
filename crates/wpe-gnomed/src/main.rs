//! `wpe-gnomed`: the wpe-gnome daemon. Runs as a systemd user service bound to the graphical
//! session and owns everything that outlives the UI.

mod background;
mod clock;
mod dbus;
mod launcher;
mod monitors;
mod session;
mod shell;
mod watcher;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use async_channel::Sender;
use async_signal::Signal;
use async_signal::Signals;
use clap::Parser;
use futures_lite::StreamExt;
use thiserror::Error;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use wpe_gnome_core::config::ConfigError;
use wpe_gnome_core::config::ConfigRepository;
use wpe_gnome_core::config::Paths;
use wpe_gnome_core::config::PathsError;
use wpe_gnome_core::ports::StillError;
use wpe_gnome_core::service::Context;
use wpe_gnome_core::service::Event;
use wpe_gnome_core::service::Service;
use wpe_gnome_core::steam::SteamLayout;
use wpe_gnome_core::steam::WorkshopRepository;
use wpe_gnome_ipc::names;

use crate::background::GSettingsBackground;
use crate::clock::SystemClock;
use crate::dbus::DaemonInterface;
use crate::launcher::ProcessLauncher;
use crate::monitors::MutterMonitors;
use crate::shell::ShellPinner;
use crate::watcher::LibraryWatcher;

const TICK: Duration = Duration::from_secs(5);
const RENDERER_BINARY: &str = "wpe-gnome-renderer";
const SCENE_BINARY: &str = "lwe/linux-wallpaperengine";

#[derive(Debug, Parser)]
#[command(
    name = "wpe-gnomed",
    about = "Wallpaper Engine wallpapers on GNOME: the daemon",
    version
)]
struct Cli {
    /// Path of the GTK renderer. Defaults to `wpe-gnome-renderer` next to this executable.
    #[arg(long)]
    renderer: Option<PathBuf>,
    /// Path of linux-wallpaperengine. Defaults to `~/.local/libexec/wpe-gnome/lwe/linux-wallpaperengine`.
    #[arg(long)]
    scene_renderer: Option<PathBuf>,
}

#[derive(Debug, Error)]
enum DaemonError {
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("D-Bus: {0}")]
    Bus(#[from] zbus::Error),
    #[error(transparent)]
    Settings(#[from] StillError),
    #[error("could not create {0}: {1}")]
    Directory(PathBuf, std::io::Error),
}

fn main() -> ExitCode {
    initialise_logging();
    let cli = Cli::parse();

    match smol::block_on(run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "Daemon failed.");

            ExitCode::FAILURE
        }
    }
}

fn initialise_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Under systemd the journal is the log; anywhere else (a nested devkit Shell, a terminal)
    // stderr is.
    match tracing_journald::layer() {
        Ok(journal) if std::env::var_os("JOURNAL_STREAM").is_some() => {
            tracing_subscriber::registry()
                .with(filter)
                .with(journal)
                .init();
        }
        Ok(_) | Err(_) => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .init();
        }
    }
}

async fn run(cli: Cli) -> Result<(), DaemonError> {
    let paths = Paths::from_environment()?;
    let config_repository = ConfigRepository::new(&paths.config_file);
    let config = config_repository.load()?;
    std::fs::create_dir_all(&paths.cache_dir)
        .map_err(|error| DaemonError::Directory(paths.cache_dir.clone(), error))?;

    let renderer_binary = cli.renderer.unwrap_or_else(|| sibling(RENDERER_BINARY));
    let scene_binary = cli
        .scene_renderer
        .unwrap_or_else(|| paths.libexec_dir.join(SCENE_BINARY));
    tracing::info!(renderer = %renderer_binary.display(), scene = %scene_binary.display(), "Renderer binaries.");

    let (events, event_receiver) = async_channel::unbounded::<Event>();
    let (notifications, notification_receiver) = async_channel::unbounded();
    let connection = zbus::Connection::session().await?;
    let system = zbus::Connection::system().await.ok();

    connection
        .object_server()
        .at(
            names::DAEMON_OBJECT_PATH,
            DaemonInterface::new(events.clone()),
        )
        .await?;
    connection.request_name(names::DAEMON_BUS_NAME).await?;
    tracing::info!(name = names::DAEMON_BUS_NAME, "Acquired bus name.");

    let monitors = MutterMonitors::new(&connection).await?;
    let pinner = ShellPinner::new(&connection).await?;
    let layout = SteamLayout::discover(&paths.home, &config.extra_steam_roots);
    let workshop = WorkshopRepository::new(layout);
    let watch_dirs = workshop.watch_dirs();

    // Every background task is held here and dropped, which cancels it, when the service ends.
    let mut tasks: Vec<smol::Task<()>> = Vec::new();
    tasks.push(monitors.watch(events.clone()).await?);
    tasks.extend(pinner.watch(events.clone()).await?);
    tasks.extend(session::watch(&connection, system.as_ref(), events.clone()).await?);
    tasks.push(smol::spawn(ticker(events.clone())));
    tasks.push(smol::spawn(signals(events.clone())));
    tasks.push(smol::spawn(dbus::forward_notifications(
        connection.clone(),
        names::DAEMON_OBJECT_PATH,
        notification_receiver,
    )));
    let library_watcher = match LibraryWatcher::new(&watch_dirs, events.clone()) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(error = %error, "Library changes will not be noticed automatically.");

            None
        }
    };

    let ctx = Context {
        monitors,
        pinner,
        launcher: ProcessLauncher::new(renderer_binary, scene_binary, events.clone()),
        still: GSettingsBackground,
        clock: SystemClock,
        config_repository,
        workshop,
        cache_dir: paths.cache_dir.clone(),
        notifications,
    };

    Service::new(ctx, config, event_receiver).run().await;
    tracing::info!("Service stopped.");
    drop(library_watcher);
    drop(tasks);
    connection.release_name(names::DAEMON_BUS_NAME).await?;

    Ok(())
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

async fn ticker(events: Sender<Event>) {
    loop {
        smol::Timer::after(TICK).await;

        if events.send(Event::Tick).await.is_err() {
            break;
        }
    }
}

async fn signals(events: Sender<Event>) {
    let Ok(mut signals) = Signals::new([Signal::Term, Signal::Int, Signal::Hup]) else {
        tracing::warn!("Could not install signal handlers.");

        return;
    };

    while let Some(signal) = signals.next().await {
        tracing::info!(signal = ?signal, "Received termination signal.");

        if events.send(Event::Shutdown).await.is_err() {
            break;
        }
    }
}
