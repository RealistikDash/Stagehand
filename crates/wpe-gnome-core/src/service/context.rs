use std::path::PathBuf;

use async_channel::Sender;

use crate::config::ConfigRepository;
use crate::ports::BackgroundPinner;
use crate::ports::Clock;
use crate::ports::MonitorSource;
use crate::ports::RendererLauncher;
use crate::ports::StillImageSink;
use crate::service::events::Notification;
use crate::steam::WorkshopRepository;

/// Everything the service needs, injected by the entry point (real systems) or a test (fakes).
#[derive(Debug)]
pub struct Context<M, P, L, S, C>
where
    M: MonitorSource,
    P: BackgroundPinner,
    L: RendererLauncher,
    S: StillImageSink,
    C: Clock,
{
    pub monitors: M,
    pub pinner: P,
    pub launcher: L,
    pub still: S,
    pub clock: C,
    pub config_repository: ConfigRepository,
    pub workshop: WorkshopRepository,
    pub cache_dir: PathBuf,
    pub notifications: Sender<Notification>,
}
