use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

const WORKSHOP_PREFIX: &str = "workshop:";
const DEFAULT_PREFIX: &str = "default:";
const LOCAL_PREFIX: &str = "local:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkshopId(u64);

impl WorkshopId {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WorkshopId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessId(u32);

impl ProcessId {
    #[must_use]
    pub const fn new(pid: u32) -> Self {
        Self(pid)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WallpaperIdError {
    #[error("wallpaper id `{0}` has no known prefix")]
    UnknownPrefix(String),
    #[error("workshop id `{0}` is not a number")]
    NotANumber(String),
    #[error("wallpaper id `{0}` has an empty name")]
    EmptyName(String),
}

/// Where a wallpaper lives. Serialised as `workshop:<id>`, `default:<dir>` or `local:<dir>`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum WallpaperId {
    Workshop(WorkshopId),
    /// A project shipped with Wallpaper Engine under `projects/defaultprojects`.
    Default(String),
    /// A project under `projects/myprojects`.
    Local(String),
}

impl fmt::Display for WallpaperId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workshop(id) => write!(f, "{WORKSHOP_PREFIX}{id}"),
            Self::Default(name) => write!(f, "{DEFAULT_PREFIX}{name}"),
            Self::Local(name) => write!(f, "{LOCAL_PREFIX}{name}"),
        }
    }
}

impl FromStr for WallpaperId {
    type Err = WallpaperIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = text.strip_prefix(WORKSHOP_PREFIX) {
            let id = rest
                .parse::<u64>()
                .map_err(|_| WallpaperIdError::NotANumber(rest.to_owned()))?;

            return Ok(Self::Workshop(WorkshopId(id)));
        }

        if let Some(rest) = text.strip_prefix(DEFAULT_PREFIX) {
            return named(rest, text).map(Self::Default);
        }

        if let Some(rest) = text.strip_prefix(LOCAL_PREFIX) {
            return named(rest, text).map(Self::Local);
        }

        Err(WallpaperIdError::UnknownPrefix(text.to_owned()))
    }
}

impl From<WallpaperId> for String {
    fn from(id: WallpaperId) -> Self {
        id.to_string()
    }
}

impl TryFrom<String> for WallpaperId {
    type Error = WallpaperIdError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

fn named(rest: &str, whole: &str) -> Result<String, WallpaperIdError> {
    if rest.is_empty() {
        return Err(WallpaperIdError::EmptyName(whole.to_owned()));
    }

    Ok(rest.to_owned())
}
