//! Whether another application is playing audio, read from the sound server's sink inputs.
//! The dependency on `pactl` lives here only.

use std::time::Duration;

use async_channel::Sender;
use futures_lite::AsyncBufReadExt;
use futures_lite::FutureExt;
use futures_lite::StreamExt;
use smol::process::Command;
use smol::process::Stdio;
use stagehand_core::service::Event;
use thiserror::Error;

// Our own renderers and helpers that never count as "another application".
const OWN_BINARIES: [&str; 5] = [
    "stagehand-renderer",
    "linux-wallpaperengine",
    "WebKitWebProcess",
    "WebKitNetworkProcess",
    "sd_dummy",
];
// Sink-input events arrive in bursts; one listing per burst is enough.
const SETTLE: Duration = Duration::from_millis(300);

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("could not start pactl: {0}")]
    Spawn(std::io::Error),
}

pub struct AudioWatcher {
    _task: smol::Task<()>,
}

impl std::fmt::Debug for AudioWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioWatcher").finish_non_exhaustive()
    }
}

impl AudioWatcher {
    pub fn start(events: Sender<Event>) -> Result<Self, AudioError> {
        let mut subscription = Command::new("pactl")
            .arg("subscribe")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(AudioError::Spawn)?;
        let stdout = subscription
            .stdout
            .take()
            .ok_or_else(|| AudioError::Spawn(std::io::Error::other("pactl has no stdout")))?;

        Ok(Self {
            _task: smol::spawn(async move {
                // The child must outlive the loop; it is killed when the task is dropped.
                let _subscription = subscription;
                watch(stdout, events).await;
            }),
        })
    }
}

async fn watch(stdout: smol::process::ChildStdout, events: Sender<Event>) {
    let mut lines = smol::io::BufReader::new(stdout).lines();
    let mut playing = other_application_playing().await;

    if events
        .send(Event::OtherAudioPlaying(playing))
        .await
        .is_err()
    {
        return;
    }

    while let Some(Ok(line)) = lines.next().await {
        if !line.contains("sink-input") {
            continue;
        }

        // Drain the rest of the burst.
        loop {
            let quiet = async {
                smol::Timer::after(SETTLE).await;

                true
            };
            let more = async { !matches!(lines.next().await, Some(Ok(_))) };

            if quiet.or(more).await {
                break;
            }
        }

        let now_playing = other_application_playing().await;

        if now_playing != playing {
            playing = now_playing;
            tracing::debug!(playing, "Other application audio state changed.");

            if events
                .send(Event::OtherAudioPlaying(playing))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

async fn other_application_playing() -> bool {
    let output = Command::new("pactl")
        .args(["-f", "json", "list", "sink-inputs"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await;
    let Ok(output) = output else {
        return false;
    };
    let Ok(inputs) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) else {
        return false;
    };

    inputs.iter().any(is_foreign_and_playing)
}

fn is_foreign_and_playing(input: &serde_json::Value) -> bool {
    let corked = input
        .get("corked")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let muted = input
        .get("mute")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let binary = input
        .get("properties")
        .and_then(|properties| properties.get("application.process.binary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    !corked && !muted && !OWN_BINARIES.iter().any(|own| binary.starts_with(own))
}
