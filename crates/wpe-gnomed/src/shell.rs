//! The GNOME Shell extension, reached through the object it exports on `org.gnome.Shell`.

use async_channel::Sender;
use futures_lite::StreamExt;
use wpe_gnome_core::model::ConnectorName;
use wpe_gnome_core::model::DimLevel;
use wpe_gnome_core::model::ProcessId;
use wpe_gnome_core::ports::BackgroundPinner;
use wpe_gnome_core::ports::PinError;
use wpe_gnome_core::service::Event;
use wpe_gnome_core::service::Occlusion;
use wpe_gnome_ipc::ShellProxy;
use wpe_gnome_ipc::names;

#[derive(Debug)]
pub struct ShellPinner {
    proxy: ShellProxy<'static>,
}

impl ShellPinner {
    pub async fn new(connection: &zbus::Connection) -> Result<Self, zbus::Error> {
        // The Shell may not be up yet, and its name may come and go; the proxy must not cache.
        let proxy = ShellProxy::builder(connection)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;

        Ok(Self { proxy })
    }

    /// Forwards the extension's signals and (dis)appearance to the service until the returned
    /// tasks are dropped.
    pub async fn watch(&self, events: Sender<Event>) -> Result<Vec<smol::Task<()>>, zbus::Error> {
        let mut tasks = Vec::new();
        let mut pinned = self.proxy.receive_window_pinned().await?;
        let mut lost = self.proxy.receive_window_lost().await?;
        let mut occlusion = self.proxy.receive_occlusion_changed().await?;
        let mut ready = self.proxy.receive_ready().await?;
        let mut owner = self.proxy.inner().receive_owner_changed().await?;

        tasks.push(smol::spawn({
            let events = events.clone();

            async move {
                while let Some(signal) = pinned.next().await {
                    let Ok(args) = signal.args() else {
                        continue;
                    };

                    if events.send(Event::WindowPinned(ProcessId::new(args.pid))).await.is_err() {
                        break;
                    }
                }
            }
        }));
        tasks.push(smol::spawn({
            let events = events.clone();

            async move {
                while let Some(signal) = lost.next().await {
                    let Ok(args) = signal.args() else {
                        continue;
                    };

                    if events.send(Event::WindowLost(ProcessId::new(args.pid))).await.is_err() {
                        break;
                    }
                }
            }
        }));
        tasks.push(smol::spawn({
            let events = events.clone();

            async move {
                while let Some(signal) = occlusion.next().await {
                    let Ok(args) = signal.args() else {
                        continue;
                    };
                    let event = Event::OcclusionChanged {
                        connector: ConnectorName::new(args.connector),
                        occlusion: parse_occlusion(args.state),
                    };

                    if events.send(event).await.is_err() {
                        break;
                    }
                }
            }
        }));
        tasks.push(smol::spawn({
            let events = events.clone();
            let proxy = self.proxy.clone();

            async move {
                while ready.next().await.is_some() {
                    if events.send(Event::ShellReady(true)).await.is_err() {
                        break;
                    }

                    send_occlusion_snapshot(&proxy, &events).await;
                }
            }
        }));
        tasks.push(smol::spawn(async move {
            while let Some(new_owner) = owner.next().await {
                if new_owner.is_none() {
                    tracing::info!("GNOME Shell left the bus.");

                    if events.send(Event::ShellReady(false)).await.is_err() {
                        break;
                    }
                }
            }
        }));

        Ok(tasks)
    }
}

async fn send_occlusion_snapshot(proxy: &ShellProxy<'static>, events: &Sender<Event>) {
    let Ok(states) = proxy.occlusion_states().await else {
        return;
    };

    for (connector, state) in states {
        let event = Event::OcclusionChanged {
            connector: ConnectorName::new(connector),
            occlusion: parse_occlusion(&state),
        };

        if events.send(event).await.is_err() {
            return;
        }
    }
}

fn parse_occlusion(state: &str) -> Occlusion {
    match state {
        names::OCCLUSION_FULLSCREEN => Occlusion::Fullscreen,
        names::OCCLUSION_MAXIMISED => Occlusion::Maximised,
        _ => Occlusion::None,
    }
}

fn call_error(error: zbus::Error) -> PinError {
    match error {
        zbus::Error::FDO(ref fdo)
            if matches!(
                **fdo,
                zbus::fdo::Error::ServiceUnknown(_)
                    | zbus::fdo::Error::UnknownObject(_)
                    | zbus::fdo::Error::UnknownInterface(_)
                    | zbus::fdo::Error::UnknownMethod(_)
                    | zbus::fdo::Error::NameHasNoOwner(_)
            ) =>
        {
            PinError::ExtensionMissing
        }
        other => PinError::Call(other.to_string()),
    }
}

impl BackgroundPinner for ShellPinner {
    async fn is_ready(&mut self) -> bool {
        match self.proxy.version().await {
            Ok(version) if version == names::SHELL_INTERFACE_VERSION => true,
            Ok(version) => {
                tracing::error!(version, expected = names::SHELL_INTERFACE_VERSION, "Shell extension version mismatch.");

                false
            }
            Err(error) => {
                tracing::debug!(error = %error, "Shell extension not reachable.");

                false
            }
        }
    }

    async fn pin(&mut self, pid: ProcessId, connectors: &[ConnectorName]) -> Result<(), PinError> {
        let names: Vec<&str> = connectors.iter().map(ConnectorName::as_str).collect();

        self.proxy.pin(pid.get(), &names).await.map_err(call_error)
    }

    async fn unpin(&mut self, pid: ProcessId) -> Result<(), PinError> {
        self.proxy.unpin(pid.get()).await.map_err(call_error)
    }

    async fn set_visible(&mut self, pid: ProcessId, visible: bool) -> Result<(), PinError> {
        self.proxy.set_visible(pid.get(), visible).await.map_err(call_error)
    }

    async fn set_dim(&mut self, pid: ProcessId, dim: DimLevel) -> Result<(), PinError> {
        self.proxy.set_dim(pid.get(), dim.fraction()).await.map_err(call_error)
    }
}
