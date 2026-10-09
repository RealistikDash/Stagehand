use std::collections::BTreeSet;
use std::num::NonZeroU32;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fps(NonZeroU32);

impl Fps {
    pub const MAX: u32 = 240;

    #[must_use]
    pub fn new(value: u32) -> Option<Self> {
        NonZeroU32::new(value.min(Self::MAX)).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl Default for Fps {
    fn default() -> Self {
        Self(NonZeroU32::new(30).unwrap_or(NonZeroU32::MIN))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingMode {
    #[default]
    Fill,
    Fit,
    Stretch,
    Centre,
}

impl ScalingMode {
    pub const ALL: [Self; 4] = [Self::Fill, Self::Fit, Self::Stretch, Self::Centre];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Fit => "Fit",
            Self::Stretch => "Stretch",
            Self::Centre => "Centre",
        }
    }
}

/// Playback volume in percent, `0..=100`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Volume(u8);

impl Volume {
    #[must_use]
    pub fn new(percent: u8) -> Self {
        Self(percent.min(100))
    }

    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }

    #[must_use]
    pub fn fraction(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self(50)
    }
}

/// How much the wallpaper is darkened, in percent, `0..=90`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DimLevel(u8);

impl DimLevel {
    pub const MAX_PERCENT: u8 = 90;

    #[must_use]
    pub fn new(percent: u8) -> Self {
        Self(percent.min(Self::MAX_PERCENT))
    }

    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }

    #[must_use]
    pub fn fraction(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub muted: bool,
    pub volume: Volume,
    pub mute_when_other_app_plays: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            muted: true,
            volume: Volume::default(),
            mute_when_other_app_plays: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseTrigger {
    Fullscreen,
    Maximised,
    Lock,
    Idle,
    Battery,
}

impl PauseTrigger {
    pub const ALL: [Self; 5] = [
        Self::Fullscreen,
        Self::Maximised,
        Self::Lock,
        Self::Idle,
        Self::Battery,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fullscreen => "A fullscreen window covers the monitor",
            Self::Maximised => "A maximised window covers the monitor",
            Self::Lock => "The screen is locked or blanked",
            Self::Idle => "The session is idle",
            Self::Battery => "Running on battery power",
        }
    }

    #[must_use]
    pub fn default_set() -> BTreeSet<Self> {
        BTreeSet::from([Self::Fullscreen, Self::Maximised, Self::Lock, Self::Idle])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OledSettings {
    pub dim: DimLevel,
    /// Seconds a paused wallpaper may keep its frozen frame on screen before the clone is hidden
    /// and the still image shows instead. `None` keeps the frozen frame indefinitely.
    pub paused_still_timeout_seconds: Option<NonZeroU32>,
}

impl Default for OledSettings {
    fn default() -> Self {
        Self {
            dim: DimLevel::default(),
            paused_still_timeout_seconds: NonZeroU32::new(300),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub fps: Fps,
    pub scaling: ScalingMode,
    pub audio: AudioSettings,
    pub pause_on: BTreeSet<PauseTrigger>,
    pub oled: OledSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fps: Fps::default(),
            scaling: ScalingMode::default(),
            audio: AudioSettings::default(),
            pause_on: PauseTrigger::default_set(),
            oled: OledSettings::default(),
        }
    }
}
