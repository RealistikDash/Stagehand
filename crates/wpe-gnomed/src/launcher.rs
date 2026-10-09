//! Renderer processes. Every renderer starts behind a gate (`sh -c 'read _ && exec "$@"'`), so its
//! PID is known and pinned before the real program maps a window. Each renderer gets its own
//! process group, which is what pause (`SIGSTOP`) and termination signal.

use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_channel::Sender;
use futures_lite::AsyncWriteExt;
use rustix::process::Pid;
use rustix::process::Signal;
use smol::process::ChildStdin;
use smol::process::Command;
use wpe_gnome_core::model::ProcessId;
use wpe_gnome_core::model::ScalingMode;
use wpe_gnome_core::ports::LaunchKind;
use wpe_gnome_core::ports::LaunchPlan;
use wpe_gnome_core::ports::RendererError;
use wpe_gnome_core::ports::RendererHandle;
use wpe_gnome_core::ports::RendererLauncher;
use wpe_gnome_core::ports::SceneLaunch;
use wpe_gnome_core::protocol::GATE_LINE;
use wpe_gnome_core::protocol::RendererCommand;
use wpe_gnome_core::service::Event;

const GATE_SCRIPT: &str = "read _ && exec \"$@\"";
const KILL_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub struct ProcessLauncher {
    renderer_binary: PathBuf,
    scene_binary: PathBuf,
    events: Sender<Event>,
}

impl ProcessLauncher {
    pub fn new(renderer_binary: PathBuf, scene_binary: PathBuf, events: Sender<Event>) -> Self {
        Self {
            renderer_binary,
            scene_binary,
            events,
        }
    }

    fn argv(&self, plan: &LaunchPlan) -> Vec<OsString> {
        match &plan.kind {
            LaunchKind::Scene(scene) => scene_argv(&self.scene_binary, scene, plan),
            LaunchKind::Gtk(arguments) => {
                let json = serde_json::to_string(arguments).unwrap_or_default();

                vec![
                    self.renderer_binary.clone().into(),
                    "--arguments".into(),
                    json.into(),
                ]
            }
        }
    }
}

fn scene_argv(binary: &PathBuf, scene: &SceneLaunch, plan: &LaunchPlan) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![
        binary.clone().into(),
        "--assets-dir".into(),
        scene.assets_dir.clone().into(),
        "--window".into(),
        format!("0x0x{}x{}", plan.size.width, plan.size.height).into(),
        "--fps".into(),
        scene.fps.get().to_string().into(),
        "--scaling".into(),
        scaling_argument(scene.scaling).into(),
        "--no-fullscreen-pause".into(),
    ];

    if scene.muted {
        argv.push("--silent".into());
    } else {
        argv.push("--volume".into());
        argv.push(scene.volume.percent().to_string().into());
    }

    if !scene.mute_when_other_app_plays {
        argv.push("--noautomute".into());
    }

    for (key, value) in &scene.properties {
        argv.push("--set-property".into());
        argv.push(format!("{key}={value}").into());
    }

    argv.push(scene.directory.clone().into());

    argv
}

fn scaling_argument(scaling: ScalingMode) -> &'static str {
    match scaling {
        ScalingMode::Fill => "fill",
        ScalingMode::Fit => "fit",
        ScalingMode::Stretch => "stretch",
        ScalingMode::Centre => "default",
    }
}

impl RendererLauncher for ProcessLauncher {
    type Handle = ProcessHandle;

    async fn launch(&mut self, plan: &LaunchPlan) -> Result<ProcessHandle, RendererError> {
        let argv = self.argv(plan);
        let mut command = std::process::Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(GATE_SCRIPT)
            .arg("wpe-gate")
            .args(&argv)
            .process_group(0);
        // NOTE: The stdio configuration must be set on the async command; the conversion from
        // the std command keeps the process group but forgets the pipes.
        let mut child = Command::from(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(RendererError::Spawn)?;
        let pid = ProcessId::new(child.id());
        let stdin = child.stdin.take().ok_or(RendererError::NotRunning)?;
        let events = self.events.clone();
        let waiter = smol::spawn(async move {
            let success = child.status().await.is_ok_and(|status| status.success());
            // NOTE: The service shutting down before its renderers exit is not an error.
            let _ = events.send(Event::RendererExited { pid, success }).await;
        });

        tracing::debug!(pid = %pid, argv = ?argv, "Spawned gated renderer.");

        Ok(ProcessHandle {
            pid,
            stdin: Some(stdin),
            _waiter: waiter,
        })
    }
}

pub struct ProcessHandle {
    pid: ProcessId,
    stdin: Option<ChildStdin>,
    _waiter: smol::Task<()>,
}

impl std::fmt::Debug for ProcessHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessHandle")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl ProcessHandle {
    fn group(&self) -> Option<Pid> {
        Pid::from_raw(i32::try_from(self.pid.get()).ok()?)
    }

    fn signal_group(&self, signal: Signal) -> Result<(), RendererError> {
        let group = self.group().ok_or(RendererError::NotRunning)?;

        rustix::process::kill_process_group(group, signal)
            .map_err(|error| RendererError::Signal(error.to_string()))
    }

    async fn write_line(&mut self, line: &str) -> Result<(), RendererError> {
        let stdin = self.stdin.as_mut().ok_or(RendererError::NotRunning)?;
        stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(RendererError::Pipe)?;
        stdin.flush().await.map_err(RendererError::Pipe)
    }
}

impl RendererHandle for ProcessHandle {
    fn pid(&self) -> ProcessId {
        self.pid
    }

    async fn open_gate(&mut self) -> Result<(), RendererError> {
        self.write_line(GATE_LINE).await
    }

    async fn send(&mut self, command: &RendererCommand) -> Result<(), RendererError> {
        let line = serde_json::to_string(command)
            .map_err(|error| RendererError::Pipe(std::io::Error::other(error)))?;

        self.write_line(&line).await
    }

    fn freeze(&mut self) -> Result<(), RendererError> {
        self.signal_group(Signal::STOP)
    }

    fn thaw(&mut self) -> Result<(), RendererError> {
        self.signal_group(Signal::CONT)
    }

    /// Closes stdin (our renderer exits on EOF), sends `SIGTERM` to the group, and escalates to
    /// `SIGKILL` after a grace period.
    async fn terminate(&mut self) -> Result<(), RendererError> {
        self.stdin = None;
        let _ = self.signal_group(Signal::CONT);
        let result = self.signal_group(Signal::TERM);
        let group = self.group();

        // NOTE: Detached on purpose: the handle is dropped right after terminate(), and the kill
        // must outlive it. The task is bounded by the grace period and only sends one signal.
        smol::spawn(async move {
            smol::Timer::after(KILL_GRACE).await;

            if let Some(group) = group {
                // ESRCH here means the group already exited, which is the normal case.
                let _ = rustix::process::kill_process_group(group, Signal::KILL);
            }
        })
        .detach();

        result
    }
}
