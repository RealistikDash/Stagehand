use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::model::ids::WallpaperId;
use crate::model::monitor::MonitorKey;
use crate::model::settings::ScalingMode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperChoice {
    pub wallpaper: WallpaperId,
    /// Overrides the global scaling mode for this slot.
    pub scaling: Option<ScalingMode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentLayout {
    #[default]
    PerMonitor,
    Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Assignments {
    pub layout: AssignmentLayout,
    pub span: Option<WallpaperChoice>,
    pub monitors: BTreeMap<MonitorKey, WallpaperChoice>,
}
