//! Reads JSON command lines from stdin on a thread and hands them to the GTK main loop.

use std::io::BufRead;

use async_channel::Receiver;
use async_channel::Sender;
use wpe_gnome_core::protocol::RendererCommand;

#[derive(Debug)]
pub enum StdinEvent {
    Command(RendererCommand),
    Malformed(String),
    Closed,
}

pub fn start() -> Receiver<StdinEvent> {
    let (sender, receiver) = async_channel::unbounded();

    std::thread::Builder::new()
        .name("stdin".to_owned())
        .spawn(move || read_loop(&sender))
        .map(|_| ())
        .unwrap_or_else(|error| {
            tracing::error!(error = %error, "Could not start stdin reader.");
        });

    receiver
}

fn read_loop(sender: &Sender<StdinEvent>) {
    let stdin = std::io::stdin();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };

        if line.trim().is_empty() {
            continue;
        }

        let event = match serde_json::from_str::<RendererCommand>(&line) {
            Ok(command) => StdinEvent::Command(command),
            Err(error) => StdinEvent::Malformed(error.to_string()),
        };

        if sender.send_blocking(event).is_err() {
            return;
        }
    }

    // NOTE: A closed receiver means the main loop has already gone.
    let _ = sender.send_blocking(StdinEvent::Closed);
}
