use serde::Deserialize;
use serde::Serialize;

/// An RGB colour with components in `0.0..=1.0`, the form Wallpaper Engine uses in `project.json`
/// (`"0.89 0 0.27"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Colour {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl Colour {
    #[must_use]
    pub fn parse_wallpaper_engine(text: &str) -> Option<Self> {
        let mut parts = text.split_whitespace().map(|part| part.parse::<f32>().ok());
        let red = parts.next()??;
        let green = parts.next()??;
        let blue = parts.next()??;

        Some(Self { red, green, blue })
    }

    /// Formats as `r g b` with components in `0..=1`, the form scripts and renderers expect.
    #[must_use]
    pub fn to_wallpaper_engine(self) -> String {
        format!("{} {} {}", self.red, self.green, self.blue)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Bool(bool),
    Number(f64),
    Colour(Colour),
    Text(String),
}

impl PropertyValue {
    /// The textual form linux-wallpaperengine accepts after `--set-property name=`.
    #[must_use]
    pub fn to_argument(&self) -> String {
        match self {
            Self::Bool(value) => if *value { "1" } else { "0" }.to_owned(),
            Self::Number(value) => value.to_string(),
            Self::Colour(colour) => colour.to_wallpaper_engine(),
            Self::Text(text) => text.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComboOption {
    pub label: String,
    pub value: PropertyValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropertyKind {
    Bool,
    Slider {
        min: f64,
        max: f64,
        step: Option<f64>,
        precision: Option<u8>,
    },
    Colour,
    Combo {
        options: Vec<ComboOption>,
    },
    TextInput,
    /// A static heading or paragraph shown between controls.
    Label,
    File,
    Directory,
    SceneTexture,
    Unsupported {
        raw_type: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    /// The `text` field, which may be a localisation key such as `ui_browse_properties_scheme_color`.
    pub label: String,
    pub kind: PropertyKind,
    pub default: Option<PropertyValue>,
    pub order: i64,
    /// A JavaScript-like expression over other properties (`HEADER_ADVANCED.value`) that gates
    /// the control. Not evaluated; shown as a hint.
    pub condition: Option<String>,
}

impl Property {
    #[must_use]
    pub fn is_editable(&self) -> bool {
        matches!(
            self.kind,
            PropertyKind::Bool
                | PropertyKind::Slider { .. }
                | PropertyKind::Colour
                | PropertyKind::Combo { .. }
                | PropertyKind::TextInput
        )
    }
}
