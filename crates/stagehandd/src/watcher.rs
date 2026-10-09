//! Watches the workshop directories and reports a settled change to the service.

use std::path::PathBuf;
use std::time::Duration;

use async_channel::Receiver;
use async_channel::Sender;
use futures_lite::FutureExt;
use notify::RecursiveMode;
use notify::Watcher;
use stagehand_core::service::Event;
use thiserror::Error;

// Steam writes a subscription in many small steps; wait for the burst to end.
const SETTLE: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum WatchError {
    #[error("could not watch {path}: {source}")]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
    #[error("could not create watcher: {0}")]
    Create(notify::Error),
}

pub struct LibraryWatcher {
    _watcher: notify::RecommendedWatcher,
    _task: smol::Task<()>,
}

impl std::fmt::Debug for LibraryWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryWatcher").finish_non_exhaustive()
    }
}

impl LibraryWatcher {
    pub fn new(directories: &[PathBuf], events: Sender<Event>) -> Result<Self, WatchError> {
        let (raw_sender, raw_receiver) = async_channel::unbounded::<()>();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                match result {
                    // Renderers read the media files constantly; only structural changes matter.
                    Ok(event) if is_structural(event.kind) => {
                        // NOTE: A full or closed channel only means the debounce task already knows.
                        let _ = raw_sender.try_send(());
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(error = %error, "File watcher reported an error."),
                }
            })
            .map_err(WatchError::Create)?;

        for directory in directories {
            watcher
                .watch(directory, RecursiveMode::Recursive)
                .map_err(|source| WatchError::Watch {
                    path: directory.clone(),
                    source,
                })?;
            tracing::debug!(path = %directory.display(), "Watching wallpaper directory.");
        }

        Ok(Self {
            _watcher: watcher,
            _task: smol::spawn(debounce(raw_receiver, events)),
        })
    }
}

fn is_structural(kind: notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_)
            | notify::EventKind::Remove(_)
            | notify::EventKind::Modify(
                notify::event::ModifyKind::Data(_)
                    | notify::event::ModifyKind::Name(_)
                    | notify::event::ModifyKind::Any
            )
    )
}

async fn debounce(raw: Receiver<()>, events: Sender<Event>) {
    while raw.recv().await.is_ok() {
        // Drain further events until the directory has been quiet for the settle period.
        loop {
            let quiet = async {
                smol::Timer::after(SETTLE).await;

                true
            };
            let more = async { raw.recv().await.is_err() };

            if quiet.or(more).await {
                break;
            }
        }

        if events.send(Event::LibraryChanged).await.is_err() {
            return;
        }
    }
}
