//! The `org.gnome.desktop.background` keys, through GSettings.
//!
//! dconf writes are synchronous calls to the dconf service. They run on a blocking thread with a
//! timeout so that a wedged settings service can never stall the service loop.

use std::time::Duration;

use futures_lite::FutureExt;
use gio::prelude::*;
use wpe_gnome_core::config::BackgroundKeys;
use wpe_gnome_core::ports::StillError;
use wpe_gnome_core::ports::StillImageSink;

const SCHEMA: &str = "org.gnome.desktop.background";
const PICTURE_URI: &str = "picture-uri";
const PICTURE_URI_DARK: &str = "picture-uri-dark";
const PICTURE_OPTIONS: &str = "picture-options";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct GSettingsBackground;

fn open() -> Result<gio::Settings, StillError> {
    let source = gio::SettingsSchemaSource::default()
        .ok_or_else(|| StillError::Read("no GSettings schema source".to_owned()))?;
    let schema = source
        .lookup(SCHEMA, true)
        .ok_or_else(|| StillError::Read(format!("schema {SCHEMA} is not installed")))?;

    Ok(gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None))
}

async fn with_timeout<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, StillError> + Send + 'static,
    describe: fn(String) -> StillError,
) -> Result<T, StillError> {
    let timeout = async {
        smol::Timer::after(TIMEOUT).await;

        Err(describe("the settings service did not answer in time".to_owned()))
    };

    smol::unblock(work).or(timeout).await
}

impl StillImageSink for GSettingsBackground {
    async fn read_keys(&mut self) -> Result<BackgroundKeys, StillError> {
        with_timeout(
            || {
                let settings = open()?;

                Ok(BackgroundKeys {
                    picture_uri: settings.string(PICTURE_URI).to_string(),
                    picture_uri_dark: settings.string(PICTURE_URI_DARK).to_string(),
                    picture_options: settings.string(PICTURE_OPTIONS).to_string(),
                })
            },
            StillError::Read,
        )
        .await
    }

    async fn write_keys(&mut self, keys: &BackgroundKeys) -> Result<(), StillError> {
        let keys = keys.clone();

        with_timeout(
            move || {
                let settings = open()?;
                let write = |key: &str, value: &str| {
                    settings
                        .set_string(key, value)
                        .map_err(|error| StillError::Write(format!("{key}: {error}")))
                };

                write(PICTURE_URI, &keys.picture_uri)?;
                write(PICTURE_URI_DARK, &keys.picture_uri_dark)?;
                write(PICTURE_OPTIONS, &keys.picture_options)?;
                gio::Settings::sync();

                Ok(())
            },
            StillError::Write,
        )
        .await
    }
}
