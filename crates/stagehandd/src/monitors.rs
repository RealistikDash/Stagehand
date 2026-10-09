//! Monitor topology from `org.gnome.Mutter.DisplayConfig`.

use std::collections::HashMap;

use async_channel::Sender;
use futures_lite::StreamExt;
use serde::Deserialize;
use stagehand_core::model::ConnectorName;
use stagehand_core::model::MonitorIdentity;
use stagehand_core::model::MonitorInfo;
use stagehand_core::model::Rectangle;
use stagehand_core::ports::MonitorError;
use stagehand_core::ports::MonitorSource;
use stagehand_core::service::Event;
use zbus::zvariant::OwnedValue;
use zbus::zvariant::Type;

// The tuple structs mirror the D-Bus signatures `(ssss)`, `(siiddada{sv})`, `((ssss)a(...)a{sv})`
// and `(iiduba(ssss)a{sv})`; fields that are not read still have to exist for decoding.
#[derive(Clone, Debug, Deserialize, Type)]
struct MonitorSpec(String, String, String, String);

#[expect(
    dead_code,
    reason = "positional D-Bus fields must be decoded even when unused"
)]
#[derive(Clone, Debug, Deserialize, Type)]
struct Mode(
    String,
    i32,
    i32,
    f64,
    f64,
    Vec<f64>,
    HashMap<String, OwnedValue>,
);

#[expect(
    dead_code,
    reason = "positional D-Bus fields must be decoded even when unused"
)]
#[derive(Clone, Debug, Deserialize, Type)]
struct PhysicalMonitor(MonitorSpec, Vec<Mode>, HashMap<String, OwnedValue>);

#[expect(
    dead_code,
    reason = "positional D-Bus fields must be decoded even when unused"
)]
#[derive(Clone, Debug, Deserialize, Type)]
struct LogicalMonitor(
    i32,
    i32,
    f64,
    u32,
    bool,
    Vec<MonitorSpec>,
    HashMap<String, OwnedValue>,
);

type CurrentState = (
    u32,
    Vec<PhysicalMonitor>,
    Vec<LogicalMonitor>,
    HashMap<String, OwnedValue>,
);

#[zbus::proxy(
    interface = "org.gnome.Mutter.DisplayConfig",
    default_service = "org.gnome.Mutter.DisplayConfig",
    default_path = "/org/gnome/Mutter/DisplayConfig"
)]
trait DisplayConfig {
    fn get_current_state(&self) -> zbus::Result<CurrentState>;

    #[zbus(signal)]
    fn monitors_changed(&self) -> zbus::Result<()>;
}

#[derive(Debug)]
pub struct MutterMonitors {
    proxy: DisplayConfigProxy<'static>,
}

impl MutterMonitors {
    pub async fn new(connection: &zbus::Connection) -> Result<Self, zbus::Error> {
        Ok(Self {
            proxy: DisplayConfigProxy::new(connection).await?,
        })
    }

    /// Forwards `MonitorsChanged` to the service until the returned task is dropped.
    pub async fn watch(&self, events: Sender<Event>) -> Result<smol::Task<()>, zbus::Error> {
        let mut stream = self.proxy.receive_monitors_changed().await?;

        Ok(smol::spawn(async move {
            while stream.next().await.is_some() {
                if events.send(Event::MonitorsChanged).await.is_err() {
                    break;
                }
            }
        }))
    }
}

impl MonitorSource for MutterMonitors {
    async fn current_monitors(&mut self) -> Result<Vec<MonitorInfo>, MonitorError> {
        let (_, physical, logical, _) = self
            .proxy
            .get_current_state()
            .await
            .map_err(|error| MonitorError::Unavailable(error.to_string()))?;

        Ok(logical
            .iter()
            .flat_map(|logical| monitors_of(logical, &physical))
            .collect())
    }
}

fn monitors_of(logical: &LogicalMonitor, physical: &[PhysicalMonitor]) -> Vec<MonitorInfo> {
    let LogicalMonitor(x, y, scale, transform, primary, specs, _) = logical;

    specs
        .iter()
        .filter_map(|spec| {
            let monitor = physical.iter().find(|candidate| candidate.0.0 == spec.0)?;
            let mode = monitor
                .1
                .iter()
                .find(|mode| {
                    mode.6
                        .get("is-current")
                        .is_some_and(|flag| bool::try_from(flag.clone()).unwrap_or(false))
                })
                .or_else(|| monitor.1.first())?;
            let (mut width, mut height) = (
                logical_pixels(mode.1, *scale),
                logical_pixels(mode.2, *scale),
            );

            // Transforms 1, 3, 5 and 7 rotate by 90 or 270 degrees.
            if transform % 2 == 1 {
                std::mem::swap(&mut width, &mut height);
            }

            Some(MonitorInfo {
                identity: MonitorIdentity {
                    connector: ConnectorName::new(spec.0.clone()),
                    vendor: spec.1.clone(),
                    product: spec.2.clone(),
                    serial: spec.3.clone(),
                },
                logical: Rectangle {
                    x: *x,
                    y: *y,
                    width,
                    height,
                },
                scale: *scale,
                is_primary: *primary,
            })
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rounded positive pixel count always fits"
)]
fn logical_pixels(physical: i32, scale: f64) -> u32 {
    (f64::from(physical.max(0)) / scale.max(0.01)).round() as u32
}
