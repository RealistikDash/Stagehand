use serde::Deserialize;
use serde::Serialize;

use crate::model::ids::ProcessId;
use crate::model::ids::WallpaperId;
use crate::model::monitor::ConnectorName;
use crate::model::settings::PauseTrigger;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SlotState {
    Idle,
    Starting,
    Running,
    Paused { reasons: Vec<PauseTrigger>, hidden: bool },
    Failed { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotStatus {
    pub connectors: Vec<ConnectorName>,
    pub wallpaper: Option<WallpaperId>,
    pub pid: Option<ProcessId>,
    pub state: SlotState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub shell_ready: bool,
    pub slots: Vec<SlotStatus>,
}
