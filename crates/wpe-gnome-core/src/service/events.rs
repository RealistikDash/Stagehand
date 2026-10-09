use async_channel::Receiver;
use async_channel::Sender;
use thiserror::Error;

use crate::config::Config;
use crate::config::ConfigError;
use crate::config::PropertyValues;
use crate::model::AssignmentLayout;
use crate::model::ConnectorName;
use crate::model::MonitorInfo;
use crate::model::MonitorKey;
use crate::model::ProcessId;
use crate::model::Settings;
use crate::model::Status;
use crate::model::WallpaperChoice;
use crate::model::WallpaperId;
use crate::ports::StillError;
use crate::steam::Library;

/// One-shot reply channel. Dropping it without sending tells the requester the service went away.
#[derive(Debug)]
pub struct Reply<T>(Sender<T>);

impl<T> Reply<T> {
    #[must_use]
    pub fn new() -> (Self, Receiver<T>) {
        let (sender, receiver) = async_channel::bounded(1);

        (Self(sender), receiver)
    }

    pub fn send(self, value: T) {
        // NOTE: A requester that has already gone away is not an error for the service.
        let _ = self.0.try_send(value);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Occlusion {
    #[default]
    None,
    Maximised,
    Fullscreen,
}

#[derive(Debug)]
pub enum Event {
    MonitorsChanged,
    LibraryChanged,
    OcclusionChanged { connector: ConnectorName, occlusion: Occlusion },
    ScreenLocked(bool),
    SessionIdle(bool),
    OnBattery(bool),
    OtherAudioPlaying(bool),
    ShellReady(bool),
    WindowPinned(ProcessId),
    WindowLost(ProcessId),
    RendererExited { pid: ProcessId, success: bool },
    Tick,
    Request(Request),
    Shutdown,
}

#[derive(Debug)]
pub enum Request {
    GetLibrary(Reply<Library>),
    GetMonitors(Reply<Vec<MonitorInfo>>),
    GetStatus(Reply<Status>),
    GetConfig(Reply<Config>),
    Rescan(Reply<()>),
    SetSettings { settings: Settings, reply: Reply<Result<(), ServiceError>> },
    Assign { key: MonitorKey, choice: WallpaperChoice, reply: Reply<Result<(), ServiceError>> },
    AssignSpan { choice: WallpaperChoice, reply: Reply<Result<(), ServiceError>> },
    SetLayout { layout: AssignmentLayout, reply: Reply<Result<(), ServiceError>> },
    Unassign { key: MonitorKey, reply: Reply<Result<(), ServiceError>> },
    SetPropertyValues {
        wallpaper: WallpaperId,
        values: PropertyValues,
        reply: Reply<Result<(), ServiceError>>,
    },
    Stop(Reply<Result<(), ServiceError>>),
    RestoreBackground(Reply<Result<(), ServiceError>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notification {
    LibraryChanged,
    MonitorsChanged,
    StatusChanged,
    ConfigChanged,
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("unknown wallpaper `{0}`")]
    UnknownWallpaper(WallpaperId),
    #[error("wallpaper `{0}` cannot be shown: {1}")]
    Unsupported(WallpaperId, String),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Still(#[from] StillError),
    #[error("no original background has been recorded")]
    NothingToRestore,
}
