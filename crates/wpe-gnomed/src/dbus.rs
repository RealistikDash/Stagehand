//! The daemon's D-Bus object. Each method becomes a request on the service channel and waits for
//! its reply; this is the only place that turns a `ServiceError` into a bus error.

use async_channel::Receiver;
use async_channel::Sender;
use wpe_gnome_core::model::AssignmentLayout;
use wpe_gnome_core::model::MonitorKey;
use wpe_gnome_core::model::Settings;
use wpe_gnome_core::model::WallpaperChoice;
use wpe_gnome_core::model::WallpaperId;
use wpe_gnome_core::service::Event;
use wpe_gnome_core::service::Notification;
use wpe_gnome_core::service::Reply;
use wpe_gnome_core::service::Request;
use wpe_gnome_core::service::ServiceError;
use zbus::fdo;
use zbus::object_server::SignalEmitter;

#[derive(Debug)]
pub struct DaemonInterface {
    events: Sender<Event>,
}

impl DaemonInterface {
    pub fn new(events: Sender<Event>) -> Self {
        Self { events }
    }

    async fn request<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> fdo::Result<T> {
        let (reply, receiver) = Reply::new();

        self.events
            .send(Event::Request(build(reply)))
            .await
            .map_err(|_| fdo::Error::Failed("the service is shutting down".to_owned()))?;

        receiver
            .recv()
            .await
            .map_err(|_| fdo::Error::Failed("the service dropped the request".to_owned()))
    }

    async fn request_unit(
        &self,
        build: impl FnOnce(Reply<Result<(), ServiceError>>) -> Request,
    ) -> fdo::Result<()> {
        self.request(build).await?.map_err(service_error)
    }
}

fn service_error(error: ServiceError) -> fdo::Error {
    match error {
        ServiceError::UnknownWallpaper(_) | ServiceError::NothingToRestore => {
            fdo::Error::InvalidArgs(error.to_string())
        }
        ServiceError::Unsupported(_, _) => fdo::Error::NotSupported(error.to_string()),
        ServiceError::Config(_) | ServiceError::Still(_) => fdo::Error::Failed(error.to_string()),
    }
}

fn encode<T: serde::Serialize>(value: &T) -> fdo::Result<String> {
    serde_json::to_string(value).map_err(|error| fdo::Error::Failed(error.to_string()))
}

fn decode<T: serde::de::DeserializeOwned>(text: &str) -> fdo::Result<T> {
    serde_json::from_str(text).map_err(|error| fdo::Error::InvalidArgs(error.to_string()))
}

#[zbus::interface(name = "io.github.realistik.WpeGnome.Daemon")]
impl DaemonInterface {
    async fn get_library(&self) -> fdo::Result<String> {
        encode(&self.request(Request::GetLibrary).await?)
    }

    async fn get_monitors(&self) -> fdo::Result<String> {
        encode(&self.request(Request::GetMonitors).await?)
    }

    async fn get_status(&self) -> fdo::Result<String> {
        encode(&self.request(Request::GetStatus).await?)
    }

    async fn get_config(&self) -> fdo::Result<String> {
        encode(&self.request(Request::GetConfig).await?)
    }

    async fn rescan(&self) -> fdo::Result<()> {
        self.request(Request::Rescan).await
    }

    async fn set_settings(&self, settings_json: &str) -> fdo::Result<()> {
        let settings: Settings = decode(settings_json)?;

        self.request_unit(|reply| Request::SetSettings { settings, reply })
            .await
    }

    async fn assign(&self, monitor_key: &str, choice_json: &str) -> fdo::Result<()> {
        let key = MonitorKey::new(monitor_key);
        let choice: WallpaperChoice = decode(choice_json)?;

        self.request_unit(|reply| Request::Assign { key, choice, reply })
            .await
    }

    async fn assign_span(&self, choice_json: &str) -> fdo::Result<()> {
        let choice: WallpaperChoice = decode(choice_json)?;

        self.request_unit(|reply| Request::AssignSpan { choice, reply })
            .await
    }

    async fn set_layout(&self, layout: &str) -> fdo::Result<()> {
        let layout: AssignmentLayout =
            decode(&serde_json::Value::String(layout.to_owned()).to_string())?;

        self.request_unit(|reply| Request::SetLayout { layout, reply })
            .await
    }

    async fn unassign(&self, monitor_key: &str) -> fdo::Result<()> {
        let key = MonitorKey::new(monitor_key);

        self.request_unit(|reply| Request::Unassign { key, reply })
            .await
    }

    async fn set_property_values(&self, wallpaper_id: &str, values_json: &str) -> fdo::Result<()> {
        let wallpaper: WallpaperId =
            wallpaper_id
                .parse()
                .map_err(|error: wpe_gnome_core::model::WallpaperIdError| {
                    fdo::Error::InvalidArgs(error.to_string())
                })?;
        let values = decode(values_json)?;

        self.request_unit(|reply| Request::SetPropertyValues {
            wallpaper,
            values,
            reply,
        })
        .await
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.request_unit(Request::Stop).await
    }

    async fn restore_background(&self) -> fdo::Result<()> {
        self.request_unit(Request::RestoreBackground).await
    }

    #[zbus(signal)]
    async fn library_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn monitors_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn config_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Turns service notifications into bus signals until the channel closes.
pub async fn forward_notifications(
    connection: zbus::Connection,
    path: &str,
    notifications: Receiver<Notification>,
) {
    let Ok(emitter) = SignalEmitter::new(&connection, path.to_owned()) else {
        tracing::error!("Could not create signal emitter.");

        return;
    };

    while let Ok(notification) = notifications.recv().await {
        let result = match notification {
            Notification::LibraryChanged => DaemonInterface::library_changed(&emitter).await,
            Notification::MonitorsChanged => DaemonInterface::monitors_changed(&emitter).await,
            Notification::StatusChanged => DaemonInterface::status_changed(&emitter).await,
            Notification::ConfigChanged => DaemonInterface::config_changed(&emitter).await,
        };

        if let Err(error) = result {
            tracing::warn!(error = %error, "Could not emit signal.");
        }
    }
}
