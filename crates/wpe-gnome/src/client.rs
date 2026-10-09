//! The daemon, reached over D-Bus. The only module that knows zbus.

use std::cell::RefCell;

use async_channel::Receiver;
use futures_lite::StreamExt;
use thiserror::Error;
use wpe_gnome_core::config::Config;
use wpe_gnome_core::config::PropertyValues;
use wpe_gnome_core::model::AssignmentLayout;
use wpe_gnome_core::model::MonitorInfo;
use wpe_gnome_core::model::MonitorKey;
use wpe_gnome_core::model::Settings;
use wpe_gnome_core::model::Status;
use wpe_gnome_core::model::WallpaperChoice;
use wpe_gnome_core::model::WallpaperId;
use wpe_gnome_core::steam::Library;
use wpe_gnome_ipc::DaemonProxy;
use wpe_gnome_ipc::IpcError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("the wallpaper service is not running: {0}")]
    Connect(String),
    #[error("{0}")]
    Call(String),
    #[error("malformed reply from the service: {0}")]
    Decode(#[from] IpcError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Library,
    Monitors,
    Status,
    Config,
}

pub struct DaemonClient {
    proxy: DaemonProxy<'static>,
    listeners: RefCell<Vec<glib::JoinHandle<()>>>,
}

impl std::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonClient").finish_non_exhaustive()
    }
}

fn call_error(error: zbus::Error) -> ClientError {
    match error {
        zbus::Error::MethodError(_, Some(message), _) => ClientError::Call(message),
        zbus::Error::FDO(fdo) => ClientError::Call(fdo.to_string()),
        other => ClientError::Call(other.to_string()),
    }
}

impl DaemonClient {
    pub async fn connect() -> Result<Self, ClientError> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|error| ClientError::Connect(error.to_string()))?;
        let proxy = DaemonProxy::new(&connection)
            .await
            .map_err(|error| ClientError::Connect(error.to_string()))?;

        Ok(Self {
            proxy,
            listeners: RefCell::new(Vec::new()),
        })
    }

    pub async fn library(&self) -> Result<Library, ClientError> {
        let text = self.proxy.get_library().await.map_err(call_error)?;

        Ok(wpe_gnome_ipc::decode(&text)?)
    }

    pub async fn monitors(&self) -> Result<Vec<MonitorInfo>, ClientError> {
        let text = self.proxy.get_monitors().await.map_err(call_error)?;

        Ok(wpe_gnome_ipc::decode(&text)?)
    }

    pub async fn status(&self) -> Result<Status, ClientError> {
        let text = self.proxy.get_status().await.map_err(call_error)?;

        Ok(wpe_gnome_ipc::decode(&text)?)
    }

    pub async fn config(&self) -> Result<Config, ClientError> {
        let text = self.proxy.get_config().await.map_err(call_error)?;

        Ok(wpe_gnome_ipc::decode(&text)?)
    }

    pub async fn set_settings(&self, settings: &Settings) -> Result<(), ClientError> {
        let text = wpe_gnome_ipc::encode(settings)?;

        self.proxy.set_settings(&text).await.map_err(call_error)
    }

    pub async fn assign(
        &self,
        key: &MonitorKey,
        choice: &WallpaperChoice,
    ) -> Result<(), ClientError> {
        let text = wpe_gnome_ipc::encode(choice)?;

        self.proxy
            .assign(key.as_str(), &text)
            .await
            .map_err(call_error)?;
        self.set_layout(AssignmentLayout::PerMonitor).await
    }

    pub async fn assign_span(&self, choice: &WallpaperChoice) -> Result<(), ClientError> {
        let text = wpe_gnome_ipc::encode(choice)?;

        self.proxy.assign_span(&text).await.map_err(call_error)?;
        self.set_layout(AssignmentLayout::Span).await
    }

    pub async fn set_layout(&self, layout: AssignmentLayout) -> Result<(), ClientError> {
        let name = match layout {
            AssignmentLayout::PerMonitor => "per_monitor",
            AssignmentLayout::Span => "span",
        };

        self.proxy.set_layout(name).await.map_err(call_error)
    }

    pub async fn unassign(&self, key: &MonitorKey) -> Result<(), ClientError> {
        self.proxy.unassign(key.as_str()).await.map_err(call_error)
    }

    pub async fn set_property_values(
        &self,
        wallpaper: &WallpaperId,
        values: &PropertyValues,
    ) -> Result<(), ClientError> {
        let text = wpe_gnome_ipc::encode(values)?;

        self.proxy
            .set_property_values(&wallpaper.to_string(), &text)
            .await
            .map_err(call_error)
    }

    pub async fn rescan(&self) -> Result<(), ClientError> {
        self.proxy.rescan().await.map_err(call_error)
    }

    pub async fn stop(&self) -> Result<(), ClientError> {
        self.proxy.stop().await.map_err(call_error)
    }

    /// Forwards the daemon's change signals on the GTK main loop until the client is dropped.
    pub async fn subscribe(&self) -> Result<Receiver<Change>, ClientError> {
        let (sender, receiver) = async_channel::unbounded();
        let mut library = self
            .proxy
            .receive_library_changed()
            .await
            .map_err(call_error)?;
        let mut monitors = self
            .proxy
            .receive_monitors_changed()
            .await
            .map_err(call_error)?;
        let mut status = self
            .proxy
            .receive_status_changed()
            .await
            .map_err(call_error)?;
        let mut config = self
            .proxy
            .receive_config_changed()
            .await
            .map_err(call_error)?;
        let mut listeners = self.listeners.borrow_mut();

        listeners.push(glib::spawn_future_local({
            let sender = sender.clone();

            async move {
                while library.next().await.is_some() {
                    if sender.send(Change::Library).await.is_err() {
                        break;
                    }
                }
            }
        }));
        listeners.push(glib::spawn_future_local({
            let sender = sender.clone();

            async move {
                while monitors.next().await.is_some() {
                    if sender.send(Change::Monitors).await.is_err() {
                        break;
                    }
                }
            }
        }));
        listeners.push(glib::spawn_future_local({
            let sender = sender.clone();

            async move {
                while status.next().await.is_some() {
                    if sender.send(Change::Status).await.is_err() {
                        break;
                    }
                }
            }
        }));
        listeners.push(glib::spawn_future_local(async move {
            while config.next().await.is_some() {
                if sender.send(Change::Config).await.is_err() {
                    break;
                }
            }
        }));

        Ok(receiver)
    }
}

impl Drop for DaemonClient {
    fn drop(&mut self) {
        for listener in self.listeners.borrow_mut().drain(..) {
            listener.abort();
        }
    }
}
