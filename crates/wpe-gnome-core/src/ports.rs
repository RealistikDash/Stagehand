//! Boundary traits. Each external system the service depends on is reached through one of these;
//! the daemon implements them with real systems and tests with in-memory fakes.
#![expect(
    async_fn_in_trait,
    reason = "the service runs on one thread; no auto-trait bounds are needed"
)]

use std::path::PathBuf;
use std::time::Instant;

use thiserror::Error;

use crate::config::BackgroundKeys;
use crate::model::ConnectorName;
use crate::model::DimLevel;
use crate::model::Fps;
use crate::model::MonitorInfo;
use crate::model::ProcessId;
use crate::model::ScalingMode;
use crate::model::Size;
use crate::model::Volume;
use crate::protocol::RendererArguments;
use crate::protocol::RendererCommand;

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("display configuration unavailable: {0}")]
    Unavailable(String),
}

pub trait MonitorSource {
    async fn current_monitors(&mut self) -> Result<Vec<MonitorInfo>, MonitorError>;
}

#[derive(Debug, Error)]
pub enum PinError {
    #[error("the shell extension is not running")]
    ExtensionMissing,
    #[error("shell extension call failed: {0}")]
    Call(String),
}

/// The GNOME Shell extension, which places renderer windows into the background layer.
pub trait BackgroundPinner {
    async fn is_ready(&mut self) -> bool;
    async fn pin(&mut self, pid: ProcessId, connectors: &[ConnectorName]) -> Result<(), PinError>;
    async fn unpin(&mut self, pid: ProcessId) -> Result<(), PinError>;
    async fn set_visible(&mut self, pid: ProcessId, visible: bool) -> Result<(), PinError>;
    async fn set_dim(&mut self, pid: ProcessId, dim: DimLevel) -> Result<(), PinError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneLaunch {
    pub directory: PathBuf,
    pub assets_dir: PathBuf,
    /// `name=value` pairs in the form linux-wallpaperengine's `--set-property` accepts.
    pub properties: Vec<(String, String)>,
    pub fps: Fps,
    pub scaling: ScalingMode,
    pub muted: bool,
    pub volume: Volume,
    pub mute_when_other_app_plays: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LaunchKind {
    Scene(SceneLaunch),
    Gtk(RendererArguments),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchPlan {
    pub kind: LaunchKind,
    pub size: Size,
    pub connectors: Vec<ConnectorName>,
    pub label: String,
}

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("could not start renderer: {0}")]
    Spawn(std::io::Error),
    #[error("renderer pipe failed: {0}")]
    Pipe(std::io::Error),
    #[error("renderer is no longer running")]
    NotRunning,
    #[error("could not signal renderer: {0}")]
    Signal(String),
}

pub trait RendererHandle {
    fn pid(&self) -> ProcessId;
    /// Lets the gated process `exec` the real renderer. Called once, after the window is pinned.
    async fn open_gate(&mut self) -> Result<(), RendererError>;
    async fn send(&mut self, command: &RendererCommand) -> Result<(), RendererError>;
    /// Stops the whole process group (`SIGSTOP`); no frames, no CPU, no GPU until [`thaw`].
    ///
    /// [`thaw`]: RendererHandle::thaw
    fn freeze(&mut self) -> Result<(), RendererError>;
    fn thaw(&mut self) -> Result<(), RendererError>;
    async fn terminate(&mut self) -> Result<(), RendererError>;
}

pub trait RendererLauncher {
    type Handle: RendererHandle;

    async fn launch(&mut self, plan: &LaunchPlan) -> Result<Self::Handle, RendererError>;
}

#[derive(Debug, Error)]
pub enum StillError {
    #[error("could not read background settings: {0}")]
    Read(String),
    #[error("could not write background settings: {0}")]
    Write(String),
}

/// The `org.gnome.desktop.background` keys that static consumers (Blur my Shell static blur,
/// the lock screen) read.
pub trait StillImageSink {
    async fn read_keys(&mut self) -> Result<BackgroundKeys, StillError>;
    async fn write_keys(&mut self, keys: &BackgroundKeys) -> Result<(), StillError>;
}

pub trait Clock {
    fn now(&self) -> Instant;
}
