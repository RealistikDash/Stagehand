//! Session conditions that feed the pause rules: screen lock or blank, idleness, battery.

use async_channel::Sender;
use futures_lite::StreamExt;
use stagehand_core::service::Event;

const PRESENCE_STATUS_IDLE: u32 = 3;

#[zbus::proxy(
    interface = "org.gnome.ScreenSaver",
    default_service = "org.gnome.ScreenSaver",
    default_path = "/org/gnome/ScreenSaver"
)]
trait ScreenSaver {
    fn get_active(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn active_changed(&self, active: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.gnome.SessionManager.Presence",
    default_service = "org.gnome.SessionManager",
    default_path = "/org/gnome/SessionManager/Presence"
)]
trait Presence {
    #[zbus(property, name = "status")]
    fn presence_status(&self) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn status_changed(&self, status: u32) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
}

/// Starts the watchers and reports the initial state; each watcher is one task.
pub async fn watch(
    session: &zbus::Connection,
    system: Option<&zbus::Connection>,
    events: Sender<Event>,
) -> Result<Vec<smol::Task<()>>, zbus::Error> {
    let mut tasks = Vec::new();
    let screensaver = ScreenSaverProxy::builder(session)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let presence = PresenceProxy::builder(session)
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    if let Ok(active) = screensaver.get_active().await {
        let _ = events.send(Event::ScreenLocked(active)).await;
    }

    if let Ok(status) = presence.presence_status().await {
        let _ = events
            .send(Event::SessionIdle(status == PRESENCE_STATUS_IDLE))
            .await;
    }

    let mut lock_stream = screensaver.receive_active_changed().await?;
    tasks.push(smol::spawn({
        let events = events.clone();

        async move {
            while let Some(signal) = lock_stream.next().await {
                let Ok(args) = signal.args() else {
                    continue;
                };

                if events.send(Event::ScreenLocked(args.active)).await.is_err() {
                    break;
                }
            }
        }
    }));

    let mut idle_stream = presence.receive_status_changed().await?;
    tasks.push(smol::spawn({
        let events = events.clone();

        async move {
            while let Some(signal) = idle_stream.next().await {
                let Ok(args) = signal.args() else {
                    continue;
                };
                let idle = args.status == PRESENCE_STATUS_IDLE;

                if events.send(Event::SessionIdle(idle)).await.is_err() {
                    break;
                }
            }
        }
    }));

    if let Some(system) = system {
        let upower = UPowerProxy::new(system).await?;

        if let Ok(on_battery) = upower.on_battery().await {
            let _ = events.send(Event::OnBattery(on_battery)).await;
        }

        let mut battery_stream = upower.receive_on_battery_changed().await;
        tasks.push(smol::spawn(async move {
            while let Some(change) = battery_stream.next().await {
                let Ok(on_battery) = change.get().await else {
                    continue;
                };

                if events.send(Event::OnBattery(on_battery)).await.is_err() {
                    break;
                }
            }
        }));
    }

    Ok(tasks)
}
