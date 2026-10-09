use std::path::Path;
use std::path::PathBuf;

use serde_json::Map;
use serde_json::Value;
use thiserror::Error;

use crate::model::Colour;
use crate::model::ComboOption;
use crate::model::Property;
use crate::model::PropertyKind;
use crate::model::PropertyValue;
use crate::model::Support;
use crate::model::UnsupportedReason;
use crate::model::Wallpaper;
use crate::model::WallpaperId;
use crate::model::WallpaperKind;

const PROJECT_FILE: &str = "project.json";
const WORKSHOP_URL_PREFIX: &str = "https://steamcommunity.com/sharedfiles/filedetails/?id=";

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{path} is not a JSON object")]
    NotAnObject { path: PathBuf },
}

/// Reads `<directory>/project.json`. Real files are inconsistent: unknown fields are ignored,
/// missing fields fall back to sensible values, and only unreadable or non-object files fail.
pub fn parse_project(id: WallpaperId, directory: &Path) -> Result<Wallpaper, ProjectError> {
    let path = directory.join(PROJECT_FILE);
    let text = std::fs::read_to_string(&path).map_err(|source| ProjectError::Read {
        path: path.clone(),
        source,
    })?;

    parse_project_text(id, directory, &text)
}

pub fn parse_project_text(
    id: WallpaperId,
    directory: &Path,
    text: &str,
) -> Result<Wallpaper, ProjectError> {
    let path = directory.join(PROJECT_FILE);
    let text = text.trim_start_matches('\u{feff}');
    let value: Value = serde_json::from_str(text).map_err(|source| ProjectError::Json {
        path: path.clone(),
        source,
    })?;
    let Value::Object(object) = value else {
        return Err(ProjectError::NotAnObject { path });
    };

    Ok(build_wallpaper(id, directory, &object))
}

fn build_wallpaper(id: WallpaperId, directory: &Path, object: &Map<String, Value>) -> Wallpaper {
    let kind = parse_kind(object.get("type"));
    let entry_file = string_field(object, "file").map(|name| resolve_entry(directory, name, &kind));
    let entry_exists = entry_file.as_deref().is_some_and(Path::exists);
    let support = support_for(&kind, entry_file.as_deref(), entry_exists);
    let general = object.get("general").and_then(Value::as_object);
    let properties = general
        .and_then(|general| general.get("properties"))
        .and_then(Value::as_object)
        .map(parse_properties)
        .unwrap_or_default();
    let supports_audio_processing = general
        .and_then(|general| general.get("supportsaudioprocessing"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let title = string_field(object, "title")
        .filter(|title| !title.trim().is_empty())
        .map_or_else(|| fallback_title(directory), str::to_owned);
    let workshop_url = string_field(object, "workshopurl")
        .map(str::to_owned)
        .or_else(|| match &id {
            WallpaperId::Workshop(workshop) => Some(format!("{WORKSHOP_URL_PREFIX}{workshop}")),
            WallpaperId::Default(_) | WallpaperId::Local(_) => None,
        });

    Wallpaper {
        id,
        title,
        kind,
        directory: directory.to_path_buf(),
        entry_file: entry_file.filter(|_| entry_exists),
        preview: string_field(object, "preview")
            .map(|name| directory.join(name))
            .filter(|preview| preview.exists()),
        description: string_field(object, "description")
            .unwrap_or_default()
            .to_owned(),
        tags: object
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        content_rating: string_field(object, "contentrating").map(str::to_owned),
        workshop_url,
        support,
        properties,
        supports_audio_processing,
    }
}

/// Workshop scenes ship packed: `project.json` names `scene.json` but the directory holds
/// `scene.pkg`, which linux-wallpaperengine reads directly.
fn resolve_entry(directory: &Path, name: &str, kind: &WallpaperKind) -> PathBuf {
    let plain = directory.join(name);

    if plain.exists() || *kind != WallpaperKind::Scene {
        return plain;
    }

    let packed = plain.with_extension("pkg");

    if packed.exists() { packed } else { plain }
}

fn parse_kind(value: Option<&Value>) -> WallpaperKind {
    let Some(raw) = value.and_then(Value::as_str) else {
        return WallpaperKind::Unknown(String::new());
    };

    match raw.to_ascii_lowercase().as_str() {
        "scene" => WallpaperKind::Scene,
        "video" => WallpaperKind::Video,
        "web" => WallpaperKind::Web,
        "application" => WallpaperKind::Application,
        _ => WallpaperKind::Unknown(raw.to_owned()),
    }
}

fn support_for(kind: &WallpaperKind, entry_file: Option<&Path>, entry_exists: bool) -> Support {
    match kind {
        WallpaperKind::Application => Support::Unsupported(UnsupportedReason::ApplicationWallpaper),
        WallpaperKind::Unknown(raw) if raw.is_empty() => {
            Support::Unsupported(UnsupportedReason::MissingKind)
        }
        WallpaperKind::Unknown(raw) => {
            Support::Unsupported(UnsupportedReason::UnknownKind(raw.clone()))
        }
        WallpaperKind::Scene | WallpaperKind::Video | WallpaperKind::Web => match entry_file {
            Some(_) if entry_exists => Support::Supported,
            Some(path) => {
                Support::Unsupported(UnsupportedReason::MissingEntryFile(path.to_path_buf()))
            }
            None => Support::Unsupported(UnsupportedReason::MissingEntryFile(PathBuf::new())),
        },
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "saturating float-to-integer conversion is the intent"
)]
fn saturating_u8(number: f64) -> u8 {
    number.round().clamp(0.0, f64::from(u8::MAX)) as u8
}

fn parse_properties(properties: &Map<String, Value>) -> Vec<Property> {
    let mut parsed: Vec<Property> = properties
        .iter()
        .filter_map(|(key, value)| value.as_object().map(|object| parse_property(key, object)))
        .collect();
    parsed.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.key.cmp(&b.key)));

    parsed
}

fn parse_property(key: &str, object: &Map<String, Value>) -> Property {
    let raw_type = string_field(object, "type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let value = object.get("value");
    let (kind, default) = match raw_type.as_str() {
        "bool" => (
            PropertyKind::Bool,
            value.and_then(parse_bool).map(PropertyValue::Bool),
        ),
        "slider" => (
            PropertyKind::Slider {
                min: number_field(object, "min").unwrap_or(0.0),
                max: number_field(object, "max").unwrap_or(100.0),
                step: number_field(object, "step"),
                precision: number_field(object, "precision").map(saturating_u8),
            },
            value.and_then(parse_number).map(PropertyValue::Number),
        ),
        "color" => (
            PropertyKind::Colour,
            value
                .and_then(Value::as_str)
                .and_then(Colour::parse_wallpaper_engine)
                .map(PropertyValue::Colour),
        ),
        "combo" => (
            PropertyKind::Combo {
                options: parse_combo_options(object.get("options")),
            },
            value.and_then(parse_scalar),
        ),
        "textinput" => (
            PropertyKind::TextInput,
            value
                .and_then(Value::as_str)
                .map(|text| PropertyValue::Text(text.to_owned())),
        ),
        "text" => (PropertyKind::Label, None),
        "file" => (
            PropertyKind::File,
            value
                .and_then(Value::as_str)
                .map(|text| PropertyValue::Text(text.to_owned())),
        ),
        "directory" => (PropertyKind::Directory, None),
        "scenetexture" => (
            PropertyKind::SceneTexture,
            value
                .and_then(Value::as_str)
                .map(|text| PropertyValue::Text(text.to_owned())),
        ),
        _ => (
            PropertyKind::Unsupported {
                raw_type: raw_type.clone(),
            },
            value.and_then(parse_scalar),
        ),
    };

    Property {
        key: key.to_owned(),
        label: string_field(object, "text").unwrap_or(key).to_owned(),
        kind,
        default,
        order: object
            .get("order")
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX),
        condition: string_field(object, "condition").map(str::to_owned),
    }
}

fn parse_combo_options(value: Option<&Value>) -> Vec<ComboOption> {
    value
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(Value::as_object)
                .filter_map(|option| {
                    let value = option.get("value").and_then(parse_scalar)?;
                    let label = string_field(option, "label")
                        .map_or_else(|| value.to_argument(), str::to_owned);

                    Some(ComboOption { label, value })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_scalar(value: &Value) -> Option<PropertyValue> {
    match value {
        Value::Bool(flag) => Some(PropertyValue::Bool(*flag)),
        Value::Number(number) => number.as_f64().map(PropertyValue::Number),
        Value::String(text) => Some(PropertyValue::Text(text.clone())),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn parse_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_f64().map(|number| number != 0.0),
        Value::String(text) => match text.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn parse_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn number_field(object: &Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key).and_then(parse_number)
}

fn fallback_title(directory: &Path) -> String {
    directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_owned()
}
