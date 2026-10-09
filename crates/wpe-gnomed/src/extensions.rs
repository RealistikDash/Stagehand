//! First-run setup: enable the Shell extension if it is installed but off, so a fresh package
//! install works after the next login without a visit to the Extensions app.

use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

// From GNOME Shell's ExtensionState enum.
const STATE_ACTIVE: f64 = 1.0;
const STATE_INACTIVE: f64 = 2.0;

#[zbus::proxy(
    interface = "org.gnome.Shell.Extensions",
    default_service = "org.gnome.Shell.Extensions",
    default_path = "/org/gnome/Shell/Extensions"
)]
trait ShellExtensions {
    fn get_extension_info(&self, uuid: &str) -> zbus::Result<HashMap<String, OwnedValue>>;
    fn enable_extension(&self, uuid: &str) -> zbus::Result<bool>;

    #[zbus(property)]
    fn user_extensions_enabled(&self) -> zbus::Result<bool>;
}

/// Returns whether the extension is active after the call.
pub async fn ensure_enabled(
    connection: &zbus::Connection,
    uuid: &str,
) -> Result<bool, zbus::Error> {
    let proxy = ShellExtensionsProxy::new(connection).await?;
    let info = proxy.get_extension_info(uuid).await?;
    let state = info
        .get("state")
        .and_then(|value| f64::try_from(value.clone()).ok());

    match state {
        Some(state) if state == STATE_ACTIVE => Ok(true),
        Some(state) if state == STATE_INACTIVE => {
            tracing::info!(uuid, "Enabling the Shell extension.");

            proxy.enable_extension(uuid).await
        }
        Some(state) => {
            tracing::warn!(uuid, state, "Shell extension is in an unexpected state.");

            Ok(false)
        }
        None => {
            tracing::warn!(uuid, "Shell extension is not installed.");

            Ok(false)
        }
    }
}
