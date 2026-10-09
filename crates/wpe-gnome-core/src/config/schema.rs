use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::model::Assignments;
use crate::model::PropertyValue;
use crate::model::Settings;
use crate::model::WallpaperId;

pub type PropertyValues = BTreeMap<String, PropertyValue>;

/// The `org.gnome.desktop.background` keys we overwrite, kept so they can be restored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundKeys {
    pub picture_uri: String,
    pub picture_uri_dark: String,
    pub picture_options: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub assignments: Assignments,
    /// User-chosen property values per wallpaper; absent keys use the project defaults.
    pub property_values: BTreeMap<WallpaperId, PropertyValues>,
    /// The user's background keys before the first apply, restored on uninstall.
    pub original_background: Option<BackgroundKeys>,
    /// Steam roots to scan in addition to the autodetected ones.
    pub extra_steam_roots: Vec<PathBuf>,
}
