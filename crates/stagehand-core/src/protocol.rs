//! The line protocol between the daemon and `stagehand-renderer`. Commands are JSON objects, one
//! per line, written to the renderer's stdin. The first line of any renderer is the gate
//! (`go`); everything after it is a [`RendererCommand`].

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::config::PropertyValues;
use crate::model::Fps;
use crate::model::ScalingMode;
use crate::model::Size;
use crate::model::Volume;

/// The line written to release the start gate (see `docs/design.md` §3.4).
pub const GATE_LINE: &str = "go";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum RendererCommand {
    Pause,
    Resume,
    SetAudio { muted: bool, volume: Volume },
    SetProperties { values: PropertyValues },
    Quit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RendererContent {
    Video { file: PathBuf },
    Web { index: PathBuf },
}

/// Everything the GTK renderer needs to start, passed as one JSON argument.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RendererArguments {
    pub content: RendererContent,
    /// Logical size of the window; GTK applies the monitor scale itself.
    pub size: Size,
    pub scaling: ScalingMode,
    pub fps: Fps,
    pub muted: bool,
    pub volume: Volume,
    pub properties: PropertyValues,
    /// Shown in the window title so a stray window can be told apart in logs.
    pub label: String,
}
