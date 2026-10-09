//! Commands that talk to the running daemon over D-Bus.

use std::io::Write;

use thiserror::Error;
use wpe_gnome_core::config::Config;
use wpe_gnome_core::model::MonitorInfo;
use wpe_gnome_core::model::ScalingMode;
use wpe_gnome_core::model::SlotState;
use wpe_gnome_core::model::Status;
use wpe_gnome_core::model::WallpaperChoice;
use wpe_gnome_core::model::WallpaperId;
use wpe_gnome_core::model::WallpaperIdError;
use wpe_gnome_ipc::DaemonProxy;
use wpe_gnome_ipc::IpcError;

#[derive(Debug, Error)]
pub enum DaemonCommandError {
    #[error("cannot reach the daemon: {0}")]
    Connect(zbus::Error),
    #[error(transparent)]
    Ipc(#[from] IpcError),
    #[error(transparent)]
    Bus(#[from] zbus::Error),
    #[error(transparent)]
    Id(#[from] WallpaperIdError),
    #[error("unknown scaling mode `{0}` (use fill, fit, stretch or centre)")]
    Scaling(String),
    #[error("either --monitor <key> or --span is required")]
    NoTarget,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

async fn proxy() -> Result<DaemonProxy<'static>, DaemonCommandError> {
    let connection = zbus::Connection::session().await.map_err(DaemonCommandError::Connect)?;

    DaemonProxy::new(&connection).await.map_err(DaemonCommandError::Connect)
}

pub fn status() -> Result<(), DaemonCommandError> {
    smol::block_on(async {
        let status: Status = wpe_gnome_ipc::decode(&proxy().await?.get_status().await?)?;
        let mut out = std::io::stdout().lock();

        writeln!(out, "Shell extension: {}", if status.shell_ready { "ready" } else { "not running" })?;

        for slot in &status.slots {
            let connectors: Vec<&str> = slot.connectors.iter().map(|c| c.as_str()).collect();
            let wallpaper = slot.wallpaper.as_ref().map_or("-".to_owned(), ToString::to_string);
            let state = match &slot.state {
                SlotState::Idle => "idle".to_owned(),
                SlotState::Starting => "starting".to_owned(),
                SlotState::Running => "running".to_owned(),
                SlotState::Paused { reasons, hidden } => {
                    format!("paused ({reasons:?}{})", if *hidden { ", hidden" } else { "" })
                }
                SlotState::Failed { message } => format!("failed: {message}"),
            };
            let pid = slot.pid.map_or("-".to_owned(), |pid| pid.to_string());

            writeln!(out, "{:<16} {:<28} pid {:<8} {}", connectors.join("+"), wallpaper, pid, state)?;
        }

        Ok(())
    })
}

pub fn monitors() -> Result<(), DaemonCommandError> {
    smol::block_on(async {
        let monitors: Vec<MonitorInfo> = wpe_gnome_ipc::decode(&proxy().await?.get_monitors().await?)?;
        let mut out = std::io::stdout().lock();

        for monitor in &monitors {
            writeln!(
                out,
                "{:<10} {}x{}+{}+{} scale {} {} key `{}`",
                monitor.identity.connector,
                monitor.logical.width,
                monitor.logical.height,
                monitor.logical.x,
                monitor.logical.y,
                monitor.scale,
                if monitor.is_primary { "primary" } else { "" },
                monitor.identity.key(),
            )?;
        }

        Ok(())
    })
}

pub fn apply(
    wallpaper: &str,
    monitor: Option<&str>,
    span: bool,
    scaling: Option<&str>,
) -> Result<(), DaemonCommandError> {
    let wallpaper: WallpaperId = wallpaper.parse()?;
    let scaling = scaling.map(parse_scaling).transpose()?;
    let choice = WallpaperChoice { wallpaper, scaling };

    smol::block_on(async {
        let proxy = proxy().await?;
        let choice_json = wpe_gnome_ipc::encode(&choice)?;

        if span {
            proxy.assign_span(&choice_json).await?;
            proxy.set_layout("span").await?;
        } else if let Some(key) = monitor {
            proxy.assign(key, &choice_json).await?;
            proxy.set_layout("per_monitor").await?;
        } else {
            return Err(DaemonCommandError::NoTarget);
        }

        Ok(())
    })
}

pub fn stop() -> Result<(), DaemonCommandError> {
    smol::block_on(async { Ok(proxy().await?.stop().await?) })
}

pub fn rescan() -> Result<(), DaemonCommandError> {
    smol::block_on(async { Ok(proxy().await?.rescan().await?) })
}

pub fn config() -> Result<(), DaemonCommandError> {
    smol::block_on(async {
        let config: Config = wpe_gnome_ipc::decode(&proxy().await?.get_config().await?)?;
        let text = serde_json::to_string_pretty(&config).map_err(IpcError::Decode)?;

        writeln!(std::io::stdout().lock(), "{text}")?;

        Ok(())
    })
}

pub fn restore_background() -> Result<(), DaemonCommandError> {
    smol::block_on(async { Ok(proxy().await?.restore_background().await?) })
}

fn parse_scaling(text: &str) -> Result<ScalingMode, DaemonCommandError> {
    ScalingMode::ALL
        .into_iter()
        .find(|mode| mode.label().eq_ignore_ascii_case(text))
        .ok_or_else(|| DaemonCommandError::Scaling(text.to_owned()))
}
