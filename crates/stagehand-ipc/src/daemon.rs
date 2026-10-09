use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error(transparent)]
    Bus(#[from] zbus::Error),
    #[error("malformed reply: {0}")]
    Decode(#[from] serde_json::Error),
}

pub fn encode<T: Serialize>(value: &T) -> Result<String, IpcError> {
    Ok(serde_json::to_string(value)?)
}

pub fn decode<T: DeserializeOwned>(text: &str) -> Result<T, IpcError> {
    Ok(serde_json::from_str(text)?)
}

/// The daemon's interface. JSON arguments and replies use the `stagehand_core` models:
/// `Library`, `Vec<MonitorInfo>`, `Status`, `Config`, `Settings`, `WallpaperChoice` and
/// `PropertyValues`.
#[zbus::proxy(
    interface = "io.github.realistikdash.Stagehand.Daemon",
    default_service = "io.github.realistikdash.Stagehand.Daemon",
    default_path = "/io/github/realistikdash/Stagehand"
)]
pub trait Daemon {
    fn get_library(&self) -> zbus::Result<String>;
    fn get_monitors(&self) -> zbus::Result<String>;
    fn get_status(&self) -> zbus::Result<String>;
    fn get_config(&self) -> zbus::Result<String>;
    fn rescan(&self) -> zbus::Result<()>;
    fn set_settings(&self, settings_json: &str) -> zbus::Result<()>;
    fn assign(&self, monitor_key: &str, choice_json: &str) -> zbus::Result<()>;
    fn assign_span(&self, choice_json: &str) -> zbus::Result<()>;
    fn set_layout(&self, layout: &str) -> zbus::Result<()>;
    fn unassign(&self, monitor_key: &str) -> zbus::Result<()>;
    fn set_property_values(&self, wallpaper_id: &str, values_json: &str) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn restore_background(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn library_changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn monitors_changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn status_changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn config_changed(&self) -> zbus::Result<()>;
}
