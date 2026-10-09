//! "Start at login": the systemd user unit, driven through `systemctl --user`.

use std::ffi::OsStr;

use thiserror::Error;

const UNIT: &str = "wpe-gnomed.service";

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("could not run systemctl: {0}")]
    Spawn(glib::Error),
    #[error("systemctl failed: {0}")]
    Failed(String),
}

pub async fn is_enabled() -> Result<bool, AutostartError> {
    let (status, _) = run(&["is-enabled", UNIT]).await?;

    Ok(status == 0)
}

pub async fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
    let verb = if enabled { "enable" } else { "disable" };
    let (status, output) = run(&[verb, "--now", UNIT]).await?;

    if status == 0 {
        Ok(())
    } else {
        Err(AutostartError::Failed(output))
    }
}

async fn run(arguments: &[&str]) -> Result<(i32, String), AutostartError> {
    let mut argv = vec!["systemctl", "--user"];
    argv.extend_from_slice(arguments);
    let argv: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
    let process = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    )
    .map_err(AutostartError::Spawn)?;
    let (stdout, _) = process
        .communicate_utf8_future(None)
        .await
        .map_err(AutostartError::Spawn)?;

    Ok((
        process.exit_status(),
        stdout.map(|text| text.to_string()).unwrap_or_default(),
    ))
}
