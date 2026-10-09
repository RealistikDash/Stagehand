use std::io::Write;

use thiserror::Error;
use wpe_gnome_core::config::ConfigRepository;
use wpe_gnome_core::config::Paths;
use wpe_gnome_core::config::PathsError;
use wpe_gnome_core::model::LibraryItem;
use wpe_gnome_core::model::PropertyKind;
use wpe_gnome_core::model::PropertyValue;
use wpe_gnome_core::model::Support;
use wpe_gnome_core::model::UnsupportedReason;
use wpe_gnome_core::model::WallpaperId;
use wpe_gnome_core::model::WallpaperIdError;
use wpe_gnome_core::steam::Library;
use wpe_gnome_core::steam::SteamLayout;
use wpe_gnome_core::steam::WorkshopRepository;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    Config(#[from] wpe_gnome_core::config::ConfigError),
    #[error(transparent)]
    Id(#[from] WallpaperIdError),
    #[error("no wallpaper with id `{0}`")]
    NotFound(WallpaperId),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn repository() -> Result<WorkshopRepository, CommandError> {
    let paths = Paths::from_environment()?;
    let config = ConfigRepository::new(&paths.config_file).load()?;
    let layout = SteamLayout::discover(&paths.home, &config.extra_steam_roots);

    Ok(WorkshopRepository::new(layout))
}

fn load_library() -> Result<Library, CommandError> {
    Ok(repository()?.scan())
}

pub fn list(json: bool, kind: Option<&str>) -> Result<(), CommandError> {
    let library = load_library()?;
    let items: Vec<&LibraryItem> = library
        .items
        .iter()
        .filter(|item| match (item, kind) {
            (_, None) => true,
            (LibraryItem::Wallpaper(wallpaper), Some(kind)) => {
                wallpaper.kind.label().eq_ignore_ascii_case(kind)
            }
            (LibraryItem::Broken(_), Some(_)) => false,
        })
        .collect();
    let mut out = std::io::stdout().lock();

    if json {
        serde_json::to_writer_pretty(&mut out, &items)?;
        writeln!(out)?;

        return Ok(());
    }

    writeln!(
        out,
        "ID                           TYPE         SUPPORT      TITLE"
    )?;

    for item in &items {
        match item {
            LibraryItem::Wallpaper(wallpaper) => {
                let support = match &wallpaper.support {
                    Support::Supported => "yes".to_owned(),
                    Support::Unsupported(reason) => format!("no: {}", describe(reason)),
                };

                writeln!(
                    out,
                    "{:<28} {:<12} {:<12} {}",
                    wallpaper.id,
                    wallpaper.kind.label(),
                    support,
                    wallpaper.title
                )?;
            }
            LibraryItem::Broken(broken) => {
                writeln!(
                    out,
                    "{:<28} broken       no           {}",
                    broken.id, broken.error
                )?;
            }
        }
    }

    writeln!(out, "\n{} items", items.len())?;

    Ok(())
}

pub fn show(id: &str) -> Result<(), CommandError> {
    let id: WallpaperId = id.parse()?;
    let library = load_library()?;
    let item = library
        .find(&id)
        .ok_or_else(|| CommandError::NotFound(id.clone()))?;
    let mut out = std::io::stdout().lock();

    match item {
        LibraryItem::Broken(broken) => {
            writeln!(out, "{} is broken: {}", broken.id, broken.error)?;
        }
        LibraryItem::Wallpaper(wallpaper) => {
            writeln!(out, "Title:     {}", wallpaper.title)?;
            writeln!(out, "Type:      {}", wallpaper.kind.label())?;
            writeln!(out, "Directory: {}", wallpaper.directory.display())?;

            if let Some(entry) = &wallpaper.entry_file {
                writeln!(out, "Entry:     {}", entry.display())?;
            }

            if let Some(url) = &wallpaper.workshop_url {
                writeln!(out, "Workshop:  {url}")?;
            }

            writeln!(out, "Tags:      {}", wallpaper.tags.join(", "))?;
            writeln!(out, "Properties ({}):", wallpaper.properties.len())?;

            for property in &wallpaper.properties {
                let default = property
                    .default
                    .as_ref()
                    .map(describe_value)
                    .unwrap_or_default();

                writeln!(
                    out,
                    "  {:<32} {:<12} {:<20} {}",
                    property.key,
                    kind_name(&property.kind),
                    default,
                    property.label
                )?;
            }
        }
    }

    Ok(())
}

pub fn paths() -> Result<(), CommandError> {
    let repository = repository()?;
    let layout = repository.layout();
    let mut out = std::io::stdout().lock();

    writeln!(out, "Steam libraries:")?;

    for library in layout.libraries() {
        writeln!(out, "  {}", library.display())?;
    }

    writeln!(
        out,
        "Wallpaper Engine: {}",
        layout
            .install_dir()
            .map_or("not found".to_owned(), |dir| dir.display().to_string())
    )?;
    writeln!(
        out,
        "Assets:           {}",
        layout
            .assets_dir()
            .map_or("not found".to_owned(), |dir| dir.display().to_string())
    )?;
    writeln!(out, "Workshop:")?;

    for dir in layout.workshop_dirs() {
        writeln!(out, "  {}", dir.display())?;
    }

    Ok(())
}

fn describe(reason: &UnsupportedReason) -> String {
    match reason {
        UnsupportedReason::ApplicationWallpaper => "application".to_owned(),
        UnsupportedReason::MissingKind => "no type".to_owned(),
        UnsupportedReason::UnknownKind(kind) => format!("type {kind}"),
        UnsupportedReason::MissingEntryFile(path) => format!("missing {}", path.display()),
    }
}

fn describe_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Bool(flag) => flag.to_string(),
        PropertyValue::Number(number) => number.to_string(),
        PropertyValue::Colour(colour) => colour.to_wallpaper_engine(),
        PropertyValue::Text(text) => text.clone(),
    }
}

fn kind_name(kind: &PropertyKind) -> &str {
    match kind {
        PropertyKind::Bool => "bool",
        PropertyKind::Slider { .. } => "slider",
        PropertyKind::Colour => "colour",
        PropertyKind::Combo { .. } => "combo",
        PropertyKind::TextInput => "text input",
        PropertyKind::Label => "label",
        PropertyKind::File => "file",
        PropertyKind::Directory => "directory",
        PropertyKind::SceneTexture => "texture",
        PropertyKind::Unsupported { raw_type } => raw_type,
    }
}
