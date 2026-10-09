use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use async_channel::Receiver;

use crate::config::BackgroundKeys;
use crate::config::Config;
use crate::model::ConnectorName;
use crate::model::DimLevel;
use crate::model::LibraryItem;
use crate::model::MonitorInfo;
use crate::model::PauseTrigger;
use crate::model::ProcessId;
use crate::model::SlotState;
use crate::model::SlotStatus;
use crate::model::Status;
use crate::model::Wallpaper;
use crate::model::WallpaperId;
use crate::ports::BackgroundPinner;
use crate::ports::Clock;
use crate::ports::LaunchKind;
use crate::ports::LaunchPlan;
use crate::ports::MonitorSource;
use crate::ports::RendererHandle;
use crate::ports::RendererLauncher;
use crate::ports::StillImageSink;
use crate::protocol::RendererCommand;
use crate::service::context::Context;
use crate::service::events::Event;
use crate::service::events::Notification;
use crate::service::events::Occlusion;
use crate::service::events::Request;
use crate::service::events::ServiceError;
use crate::service::plan;
use crate::service::plan::Conditions;
use crate::service::plan::DesiredSlot;
use crate::steam::Library;
use crate::still;

const MAX_START_ATTEMPTS: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(3);
const PICTURE_OPTIONS_FOR_STILL: &str = "zoom";

struct Process<H> {
    handle: H,
    plan: LaunchPlan,
    pinned: bool,
    paused: Option<Paused>,
    hidden: bool,
    terminating: bool,
}

struct Paused {
    since: Instant,
    reasons: Vec<PauseTrigger>,
}

struct Slot<H> {
    desired: DesiredSlot,
    current: Option<Process<H>>,
    pending: Option<Process<H>>,
    failure: Option<String>,
    attempts: u32,
    retry_at: Option<Instant>,
}

impl<H> Slot<H> {
    fn new(desired: DesiredSlot) -> Self {
        Self {
            desired,
            current: None,
            pending: None,
            failure: None,
            attempts: 0,
            retry_at: None,
        }
    }

    fn connectors(&self) -> &[ConnectorName] {
        &self.desired.connectors
    }
}

pub struct Service<M, P, L, S, C>
where
    M: MonitorSource,
    P: BackgroundPinner,
    L: RendererLauncher,
    S: StillImageSink,
    C: Clock,
{
    ctx: Context<M, P, L, S, C>,
    events: Receiver<Event>,
    config: Config,
    library: Library,
    monitors: Vec<MonitorInfo>,
    slots: Vec<Slot<L::Handle>>,
    conditions: Conditions,
    other_audio_playing: bool,
    shell_ready: bool,
    occlusion: BTreeMap<ConnectorName, Occlusion>,
    published_still: Option<PathBuf>,
}

impl<M, P, L, S, C> Service<M, P, L, S, C>
where
    M: MonitorSource,
    P: BackgroundPinner,
    L: RendererLauncher,
    S: StillImageSink,
    C: Clock,
{
    pub fn new(ctx: Context<M, P, L, S, C>, config: Config, events: Receiver<Event>) -> Self {
        Self {
            ctx,
            events,
            config,
            library: Library::default(),
            monitors: Vec::new(),
            slots: Vec::new(),
            conditions: Conditions::default(),
            other_audio_playing: false,
            shell_ready: false,
            occlusion: BTreeMap::new(),
            published_still: None,
        }
    }

    /// Runs until [`Event::Shutdown`] or until every sender is gone. Each event is handled in
    /// its own error scope so one failing wallpaper never stops the loop.
    pub async fn run(mut self) {
        self.rescan_library();
        self.refresh_monitors().await;
        self.shell_ready = self.ctx.pinner.is_ready().await;
        self.reconcile().await;

        while let Ok(event) = self.events.recv().await {
            if matches!(event, Event::Shutdown) {
                break;
            }

            self.handle(event).await;
        }

        self.stop_everything().await;
    }

    async fn handle(&mut self, event: Event) {
        match event {
            Event::MonitorsChanged => {
                self.refresh_monitors().await;
                self.notify(Notification::MonitorsChanged);
                self.reconcile().await;
            }
            Event::LibraryChanged => {
                self.rescan_library();
                self.notify(Notification::LibraryChanged);
                self.reconcile().await;
            }
            Event::OcclusionChanged { connector, occlusion } => {
                self.occlusion.insert(connector, occlusion);
                self.apply_pause_state().await;
            }
            Event::ScreenLocked(locked) => {
                self.conditions.locked = locked;
                self.apply_pause_state().await;
            }
            Event::SessionIdle(idle) => {
                self.conditions.idle = idle;
                self.apply_pause_state().await;
            }
            Event::OnBattery(on_battery) => {
                self.conditions.on_battery = on_battery;
                self.apply_pause_state().await;
            }
            Event::OtherAudioPlaying(playing) => {
                self.other_audio_playing = playing;
                self.apply_audio().await;
            }
            Event::ShellReady(ready) => {
                tracing::info!(ready, "Shell extension availability changed.");
                self.shell_ready = ready;

                if ready {
                    self.reconcile().await;
                } else {
                    self.stop_everything().await;
                    self.notify(Notification::StatusChanged);
                }
            }
            Event::WindowPinned(pid) => self.on_window_pinned(pid).await,
            Event::WindowLost(pid) => self.on_window_lost(pid).await,
            Event::RendererExited { pid, success } => self.on_renderer_exited(pid, success).await,
            Event::Tick => self.on_tick().await,
            Event::Request(request) => self.handle_request(request).await,
            Event::Shutdown => {}
        }
    }

    async fn handle_request(&mut self, request: Request) {
        match request {
            Request::GetLibrary(reply) => reply.send(self.library.clone()),
            Request::GetMonitors(reply) => reply.send(self.monitors.clone()),
            Request::GetStatus(reply) => reply.send(self.status()),
            Request::GetConfig(reply) => reply.send(self.config.clone()),
            Request::Rescan(reply) => {
                self.rescan_library();
                self.notify(Notification::LibraryChanged);
                self.reconcile().await;
                reply.send(());
            }
            Request::SetSettings { settings, reply } => {
                self.config.settings = settings;
                reply.send(self.save_config());
                self.reconcile().await;
                self.apply_audio().await;
                self.apply_dim().await;
                self.apply_pause_state().await;
            }
            Request::Assign { key, choice, reply } => {
                let result = self.check_wallpaper(&choice.wallpaper).and_then(|()| {
                    self.config.assignments.monitors.insert(key, choice);

                    self.save_config()
                });
                reply.send(result);
                self.reconcile().await;
            }
            Request::AssignSpan { choice, reply } => {
                let result = self.check_wallpaper(&choice.wallpaper).and_then(|()| {
                    self.config.assignments.span = Some(choice);

                    self.save_config()
                });
                reply.send(result);
                self.reconcile().await;
            }
            Request::SetLayout { layout, reply } => {
                self.config.assignments.layout = layout;
                reply.send(self.save_config());
                self.reconcile().await;
            }
            Request::Unassign { key, reply } => {
                self.config.assignments.monitors.remove(&key);
                reply.send(self.save_config());
                self.reconcile().await;
            }
            Request::SetPropertyValues { wallpaper, values, reply } => {
                if values.is_empty() {
                    self.config.property_values.remove(&wallpaper);
                } else {
                    self.config.property_values.insert(wallpaper, values);
                }

                reply.send(self.save_config());
                self.reconcile().await;
            }
            Request::Stop(reply) => {
                self.config.assignments.monitors.clear();
                self.config.assignments.span = None;
                reply.send(self.save_config());
                self.reconcile().await;
            }
            Request::RestoreBackground(reply) => reply.send(self.restore_background().await),
        }
    }

    fn check_wallpaper(&self, id: &WallpaperId) -> Result<(), ServiceError> {
        match self.library.find(id) {
            Some(LibraryItem::Wallpaper(_)) => Ok(()),
            Some(LibraryItem::Broken(broken)) => {
                Err(ServiceError::Unsupported(id.clone(), broken.error.clone()))
            }
            None => Err(ServiceError::UnknownWallpaper(id.clone())),
        }
    }

    fn save_config(&mut self) -> Result<(), ServiceError> {
        self.ctx.config_repository.save(&self.config)?;
        self.notify(Notification::ConfigChanged);

        Ok(())
    }

    fn notify(&self, notification: Notification) {
        // NOTE: A full or closed notification channel only means no client is listening.
        let _ = self.ctx.notifications.try_send(notification);
    }

    fn rescan_library(&mut self) {
        self.library = self.ctx.workshop.scan();
        tracing::info!(items = self.library.items.len(), "Scanned wallpaper library.");
    }

    async fn refresh_monitors(&mut self) {
        match self.ctx.monitors.current_monitors().await {
            Ok(monitors) => {
                tracing::info!(count = monitors.len(), "Read monitor configuration.");
                self.monitors = monitors;
            }
            Err(error) => {
                tracing::error!(error = %error, "Could not read monitor configuration.");
            }
        }
    }

    // ------------------------------------------------------------------------------------------
    // Reconciliation: make the running renderers match the configuration.
    // ------------------------------------------------------------------------------------------

    async fn reconcile(&mut self) {
        let desired = plan::desired_slots(&self.config.assignments, &self.monitors);
        let mut retained = Vec::new();

        for mut slot in std::mem::take(&mut self.slots) {
            if desired.iter().any(|wanted| wanted.connectors == slot.desired.connectors) {
                retained.push(slot);
            } else {
                tracing::info!(connectors = ?slot.connectors(), "Stopping renderer for removed slot.");
                self.stop_slot(&mut slot).await;
            }
        }

        self.slots = retained;

        for wanted in desired {
            let existing = self.slots.iter().position(|slot| slot.connectors() == wanted.connectors);
            let index = if let Some(index) = existing {
                index
            } else {
                self.slots.push(Slot::new(wanted.clone()));

                self.slots.len() - 1
            };

            self.reconcile_slot(index, wanted).await;
        }

        self.publish_still().await;
        self.notify(Notification::StatusChanged);
    }

    async fn reconcile_slot(&mut self, index: usize, wanted: DesiredSlot) {
        let desired = {
            let Some(slot) = self.slots.get_mut(index) else {
                return;
            };

            if slot.desired != wanted {
                slot.desired = wanted;
                slot.attempts = 0;
                slot.retry_at = None;
                slot.failure = None;
            }

            slot.desired.clone()
        };
        let plan = match self.plan_for(&desired) {
            Ok(plan) => plan,
            Err(error) => {
                let message = error.to_string();

                if let Some(slot) = self.slots.get_mut(index) {
                    if slot.failure.as_deref() != Some(&message) {
                        tracing::warn!(connectors = ?slot.connectors(), error = %error, "Slot cannot be shown.");
                    }

                    slot.failure = Some(message);
                }

                self.stop_slot_processes(index).await;

                return;
            }
        };
        let now = self.ctx.clock.now();
        let shell_ready = self.shell_ready;
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        let needs_restart = slot
            .pending
            .as_ref()
            .or(slot.current.as_ref())
            .is_none_or(|process| plan::requires_restart(&process.plan, &plan));

        if !needs_restart {
            if let Some(current) = slot.current.as_mut() {
                current.plan = plan;
            }

            return;
        }

        if !shell_ready {
            slot.failure = Some("the shell extension is not running".to_owned());

            return;
        }

        if slot.retry_at.is_some_and(|retry_at| now < retry_at) || slot.attempts >= MAX_START_ATTEMPTS {
            return;
        }

        self.start_process(index, plan).await;
    }

    fn plan_for(&self, desired: &DesiredSlot) -> Result<LaunchPlan, ServiceError> {
        let wallpaper = self.wallpaper(&desired.choice.wallpaper)?;
        let overrides = self.config.property_values.get(&wallpaper.id);

        plan::launch_plan(
            wallpaper,
            self.library.assets_dir.as_deref(),
            &self.config.settings,
            overrides,
            desired,
        )
    }

    fn wallpaper(&self, id: &WallpaperId) -> Result<&Wallpaper, ServiceError> {
        match self.library.find(id) {
            Some(LibraryItem::Wallpaper(wallpaper)) => Ok(wallpaper),
            Some(LibraryItem::Broken(broken)) => {
                Err(ServiceError::Unsupported(id.clone(), broken.error.clone()))
            }
            None => Err(ServiceError::UnknownWallpaper(id.clone())),
        }
    }

    async fn start_process(&mut self, index: usize, plan: LaunchPlan) {
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };

        if let Some(mut stale) = slot.pending.take() {
            terminate(&mut self.ctx.pinner, &mut stale).await;
        }

        slot.attempts += 1;
        tracing::info!(connectors = ?slot.connectors(), label = %plan.label, attempt = slot.attempts, "Starting renderer.");

        let mut handle = match self.ctx.launcher.launch(&plan).await {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(error = %error, "Could not start renderer.");
                slot.failure = Some(error.to_string());
                slot.retry_at = Some(self.ctx.clock.now() + RETRY_BASE_DELAY * slot.attempts);

                return;
            }
        };
        let pid = handle.pid();

        if let Err(error) = self.ctx.pinner.pin(pid, &plan.connectors).await {
            tracing::error!(error = %error, pid = %pid, "Could not pin renderer window.");
            let _ = handle.terminate().await;
            slot.failure = Some(error.to_string());
            slot.retry_at = Some(self.ctx.clock.now() + RETRY_BASE_DELAY * slot.attempts);

            return;
        }

        if let Err(error) = handle.open_gate().await {
            tracing::error!(error = %error, pid = %pid, "Could not release renderer gate.");
            let _ = self.ctx.pinner.unpin(pid).await;
            let _ = handle.terminate().await;
            slot.failure = Some(error.to_string());
            slot.retry_at = Some(self.ctx.clock.now() + RETRY_BASE_DELAY * slot.attempts);

            return;
        }

        slot.failure = None;
        slot.pending = Some(Process {
            handle,
            plan,
            pinned: false,
            paused: None,
            hidden: false,
            terminating: false,
        });
    }

    async fn on_window_pinned(&mut self, pid: ProcessId) {
        let Some(index) = self.slot_index_for(pid) else {
            tracing::debug!(pid = %pid, "Ignoring pin of unknown window.");

            return;
        };
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };

        if slot.pending.as_ref().is_some_and(|process| process.handle.pid() == pid) {
            let Some(mut promoted) = slot.pending.take() else {
                return;
            };
            promoted.pinned = true;

            if let Some(mut previous) = slot.current.replace(promoted) {
                terminate(&mut self.ctx.pinner, &mut previous).await;
            }

            slot.attempts = 0;
            slot.retry_at = None;
            tracing::info!(pid = %pid, connectors = ?slot.connectors(), "Renderer window pinned.");
        } else if let Some(current) = slot.current.as_mut() {
            current.pinned = true;
        }

        self.apply_dim().await;
        self.apply_audio().await;
        self.apply_pause_state().await;
        self.notify(Notification::StatusChanged);
    }

    async fn on_window_lost(&mut self, pid: ProcessId) {
        let Some(index) = self.slot_index_for(pid) else {
            return;
        };

        tracing::warn!(pid = %pid, "Renderer window disappeared; restarting.");
        self.remove_process(index, pid).await;
        self.reconcile().await;
    }

    async fn on_renderer_exited(&mut self, pid: ProcessId, success: bool) {
        let Some(index) = self.slot_index_for(pid) else {
            return;
        };
        let Some(slot) = self.slots.get(index) else {
            return;
        };
        let terminating = slot
            .current
            .as_ref()
            .into_iter()
            .chain(slot.pending.as_ref())
            .any(|process| process.handle.pid() == pid && process.terminating);

        self.remove_process(index, pid).await;

        if terminating {
            return;
        }

        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        tracing::warn!(pid = %pid, success, attempts = slot.attempts, "Renderer exited unexpectedly.");
        slot.failure = Some(if success {
            "the renderer exited".to_owned()
        } else {
            "the renderer crashed".to_owned()
        });
        slot.retry_at = Some(self.ctx.clock.now() + RETRY_BASE_DELAY * slot.attempts.max(1));
        self.notify(Notification::StatusChanged);
    }

    async fn on_tick(&mut self) {
        let now = self.ctx.clock.now();
        let due = self
            .slots
            .iter()
            .any(|slot| slot.retry_at.is_some_and(|at| now >= at) && slot.attempts < MAX_START_ATTEMPTS);

        if due {
            self.reconcile().await;
        }

        let Some(timeout) = self.config.settings.oled.paused_still_timeout_seconds else {
            return;
        };
        let timeout = Duration::from_secs(u64::from(timeout.get()));

        for slot in &mut self.slots {
            let Some(current) = slot.current.as_mut() else {
                continue;
            };
            let Some(paused) = current.paused.as_ref() else {
                continue;
            };

            if current.hidden || now.duration_since(paused.since) < timeout {
                continue;
            }

            tracing::info!(pid = %current.handle.pid(), "Hiding paused renderer after timeout.");

            if let Err(error) = self.ctx.pinner.set_visible(current.handle.pid(), false).await {
                tracing::warn!(error = %error, "Could not hide paused renderer.");

                continue;
            }

            current.hidden = true;
        }
    }

    fn slot_index_for(&self, pid: ProcessId) -> Option<usize> {
        self.slots.iter().position(|slot| {
            slot.current
                .as_ref()
                .into_iter()
                .chain(slot.pending.as_ref())
                .any(|process| process.handle.pid() == pid)
        })
    }

    async fn remove_process(&mut self, index: usize, pid: ProcessId) {
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        let taken = if slot.current.as_ref().is_some_and(|process| process.handle.pid() == pid) {
            slot.current.take()
        } else if slot.pending.as_ref().is_some_and(|process| process.handle.pid() == pid) {
            slot.pending.take()
        } else {
            None
        };

        if let Some(mut process) = taken {
            terminate(&mut self.ctx.pinner, &mut process).await;
        }
    }

    async fn stop_slot_processes(&mut self, index: usize) {
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };

        for process in slot.current.take().into_iter().chain(slot.pending.take()) {
            let mut process = process;
            terminate(&mut self.ctx.pinner, &mut process).await;
        }
    }

    async fn stop_slot(&mut self, slot: &mut Slot<L::Handle>) {
        for process in slot.current.take().into_iter().chain(slot.pending.take()) {
            let mut process = process;
            terminate(&mut self.ctx.pinner, &mut process).await;
        }
    }

    async fn stop_everything(&mut self) {
        for mut slot in std::mem::take(&mut self.slots) {
            self.stop_slot(&mut slot).await;
        }
    }

    // ------------------------------------------------------------------------------------------
    // Live state: pause, audio, dim.
    // ------------------------------------------------------------------------------------------

    async fn apply_pause_state(&mut self) {
        let now = self.ctx.clock.now();
        let settings = self.config.settings.clone();
        let mut changed = false;

        for slot in &mut self.slots {
            let reasons = plan::pause_reasons(&settings, self.conditions, &self.occlusion, &slot.desired.connectors);
            let Some(current) = slot.current.as_mut() else {
                continue;
            };

            if !current.pinned {
                continue;
            }

            match (&mut current.paused, reasons.is_empty()) {
                (Some(_), true) => {
                    tracing::info!(pid = %current.handle.pid(), "Resuming renderer.");

                    if let Err(error) = current.handle.thaw() {
                        tracing::warn!(error = %error, "Could not resume renderer.");

                        continue;
                    }

                    if is_gtk(&current.plan) {
                        send_command(current, &RendererCommand::Resume).await;
                    }

                    if current.hidden {
                        if let Err(error) = self.ctx.pinner.set_visible(current.handle.pid(), true).await {
                            tracing::warn!(error = %error, "Could not show renderer.");
                        }

                        current.hidden = false;
                    }

                    current.paused = None;
                    changed = true;
                }
                (None, false) => {
                    tracing::info!(pid = %current.handle.pid(), reasons = ?reasons, "Pausing renderer.");

                    if is_gtk(&current.plan) {
                        send_command(current, &RendererCommand::Pause).await;
                    }

                    if let Err(error) = current.handle.freeze() {
                        tracing::warn!(error = %error, "Could not pause renderer.");

                        continue;
                    }

                    current.paused = Some(Paused { since: now, reasons });
                    changed = true;
                }
                (Some(paused), false) => {
                    if paused.reasons != reasons {
                        paused.reasons = reasons;
                        changed = true;
                    }
                }
                (None, true) => {}
            }
        }

        if changed {
            self.notify(Notification::StatusChanged);
        }
    }

    async fn apply_audio(&mut self) {
        let audio = &self.config.settings.audio;
        let muted = audio.muted || (audio.mute_when_other_app_plays && self.other_audio_playing);
        let command = RendererCommand::SetAudio { muted, volume: audio.volume };

        for slot in &mut self.slots {
            for process in slot.current.iter_mut().chain(slot.pending.iter_mut()) {
                if is_gtk(&process.plan) && process.paused.is_none() {
                    send_command(process, &command).await;
                }
            }
        }
    }

    async fn apply_dim(&mut self) {
        let dim: DimLevel = self.config.settings.oled.dim;

        for slot in &self.slots {
            let Some(current) = slot.current.as_ref() else {
                continue;
            };

            if let Err(error) = self.ctx.pinner.set_dim(current.handle.pid(), dim).await {
                tracing::warn!(error = %error, "Could not set dim level.");
            }
        }
    }

    // ------------------------------------------------------------------------------------------
    // Still image for static consumers.
    // ------------------------------------------------------------------------------------------

    async fn publish_still(&mut self) {
        let primary = self
            .monitors
            .iter()
            .find(|monitor| monitor.is_primary)
            .or_else(|| self.monitors.first())
            .map(|monitor| monitor.identity.connector.clone());
        let Some(primary) = primary else {
            return;
        };
        let wallpaper = self
            .slots
            .iter()
            .find(|slot| slot.desired.connectors.contains(&primary))
            .and_then(|slot| self.wallpaper(&slot.desired.choice.wallpaper).ok());
        let Some(wallpaper) = wallpaper else {
            return;
        };
        let Some(preview) = wallpaper.preview.clone() else {
            tracing::debug!(wallpaper = %wallpaper.id, "Wallpaper has no preview; keeping the current still.");

            return;
        };
        let output = self.ctx.cache_dir.join(format!("still-{}.png", sanitise(&wallpaper.id.to_string())));

        if self.published_still.as_ref() == Some(&output) {
            return;
        }

        if let Err(error) = still::write_still(&preview, &output) {
            tracing::warn!(error = %error, "Could not write still image.");

            return;
        }

        if self.config.original_background.is_none() {
            match self.ctx.still.read_keys().await {
                Ok(keys) => {
                    self.config.original_background = Some(keys);

                    if let Err(error) = self.save_config() {
                        tracing::warn!(error = %error, "Could not record the original background.");
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "Could not read the original background.");
                }
            }
        }

        let uri = format!("file://{}", output.display());
        let keys = BackgroundKeys {
            picture_uri: uri.clone(),
            picture_uri_dark: uri,
            picture_options: PICTURE_OPTIONS_FOR_STILL.to_owned(),
        };

        match self.ctx.still.write_keys(&keys).await {
            Ok(()) => {
                tracing::info!(path = %output.display(), "Published still image.");
                self.published_still = Some(output);
            }
            Err(error) => tracing::warn!(error = %error, "Could not publish still image."),
        }
    }

    async fn restore_background(&mut self) -> Result<(), ServiceError> {
        let Some(keys) = self.config.original_background.clone() else {
            return Err(ServiceError::NothingToRestore);
        };

        self.ctx.still.write_keys(&keys).await?;
        self.config.original_background = None;
        self.published_still = None;
        self.save_config()
    }

    fn status(&self) -> Status {
        let slots = self
            .slots
            .iter()
            .map(|slot| {
                let process = slot.current.as_ref();
                let state = match (process, slot.pending.as_ref(), &slot.failure) {
                    (Some(current), _, _) => match &current.paused {
                        Some(paused) => SlotState::Paused {
                            reasons: paused.reasons.clone(),
                            hidden: current.hidden,
                        },
                        None => SlotState::Running,
                    },
                    (None, Some(_), _) => SlotState::Starting,
                    (None, None, Some(message)) => SlotState::Failed { message: message.clone() },
                    (None, None, None) => SlotState::Idle,
                };

                SlotStatus {
                    connectors: slot.desired.connectors.clone(),
                    wallpaper: Some(slot.desired.choice.wallpaper.clone()),
                    pid: process.or(slot.pending.as_ref()).map(|process| process.handle.pid()),
                    state,
                }
            })
            .collect();

        Status { shell_ready: self.shell_ready, slots }
    }
}

async fn terminate<P: BackgroundPinner, H: RendererHandle>(pinner: &mut P, process: &mut Process<H>) {
    let pid = process.handle.pid();
    process.terminating = true;

    if process.paused.is_some() {
        let _ = process.handle.thaw();
    }

    if let Err(error) = pinner.unpin(pid).await {
        tracing::debug!(error = %error, pid = %pid, "Unpin failed.");
    }

    if let Err(error) = process.handle.terminate().await {
        tracing::debug!(error = %error, pid = %pid, "Terminate failed.");
    }
}

async fn send_command<H: RendererHandle>(process: &mut Process<H>, command: &RendererCommand) {
    if let Err(error) = process.handle.send(command).await {
        tracing::warn!(error = %error, pid = %process.handle.pid(), "Could not send renderer command.");
    }
}

fn is_gtk(plan: &LaunchPlan) -> bool {
    matches!(plan.kind, LaunchKind::Gtk(_))
}

fn sanitise(text: &str) -> String {
    text.chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
        .collect()
}
