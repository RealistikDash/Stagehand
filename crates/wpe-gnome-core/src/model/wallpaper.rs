use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::model::ids::WallpaperId;
use crate::model::property::Property;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperKind {
    Scene,
    Video,
    Web,
    Application,
    Unknown(String),
}

impl WallpaperKind {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Scene => "Scene",
            Self::Video => "Video",
            Self::Web => "Web",
            Self::Application => "Application",
            Self::Unknown(raw) if raw.is_empty() => "Unknown",
            Self::Unknown(raw) => raw.as_str(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedReason {
    /// `application` wallpapers are Windows executables.
    ApplicationWallpaper,
    MissingKind,
    UnknownKind(String),
    MissingEntryFile(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported(UnsupportedReason),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wallpaper {
    pub id: WallpaperId,
    pub title: String,
    pub kind: WallpaperKind,
    pub directory: PathBuf,
    /// Absolute path of the entry file (`scene.json`, the video, or `index.html`), when it exists.
    pub entry_file: Option<PathBuf>,
    pub preview: Option<PathBuf>,
    pub description: String,
    pub tags: Vec<String>,
    pub content_rating: Option<String>,
    pub workshop_url: Option<String>,
    pub support: Support,
    pub properties: Vec<Property>,
    pub supports_audio_processing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokenProject {
    pub id: WallpaperId,
    pub directory: PathBuf,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[expect(
    clippy::large_enum_variant,
    reason = "a library holds tens of items; boxing buys nothing"
)]
pub enum LibraryItem {
    Wallpaper(Wallpaper),
    Broken(BrokenProject),
}

impl LibraryItem {
    #[must_use]
    pub fn id(&self) -> &WallpaperId {
        match self {
            Self::Wallpaper(wallpaper) => &wallpaper.id,
            Self::Broken(broken) => &broken.id,
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        match self {
            Self::Wallpaper(wallpaper) => &wallpaper.title,
            Self::Broken(broken) => broken
                .directory
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("(broken)"),
        }
    }
}
