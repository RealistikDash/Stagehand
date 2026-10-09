use std::path::Path;
use std::path::PathBuf;

use crate::model::BrokenProject;
use crate::model::LibraryItem;
use crate::model::WallpaperId;
use crate::model::WorkshopId;
use crate::project;
use crate::steam::layout::SteamLayout;

const PROJECT_FILE: &str = "project.json";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    pub items: Vec<LibraryItem>,
    pub assets_dir: Option<PathBuf>,
}

impl Library {
    #[must_use]
    pub fn find(&self, id: &WallpaperId) -> Option<&LibraryItem> {
        self.items.iter().find(|item| item.id() == id)
    }
}

/// Reads wallpapers from every Steam library plus the projects shipped with Wallpaper Engine.
/// Steam's directories are treated as read-only input.
#[derive(Clone, Debug)]
pub struct WorkshopRepository {
    layout: SteamLayout,
}

impl WorkshopRepository {
    #[must_use]
    pub fn new(layout: SteamLayout) -> Self {
        Self { layout }
    }

    #[must_use]
    pub fn layout(&self) -> &SteamLayout {
        &self.layout
    }

    /// Directories a watcher should observe to notice subscription changes.
    #[must_use]
    pub fn watch_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = self.layout.workshop_dirs();
        dirs.extend(self.layout.my_projects_dir());

        dirs
    }

    #[must_use]
    pub fn scan(&self) -> Library {
        let mut items = Vec::new();

        for dir in self.layout.workshop_dirs() {
            items.extend(scan_directory(&dir, |name| {
                name.parse::<u64>()
                    .ok()
                    .map(WorkshopId::new)
                    .map(WallpaperId::Workshop)
            }));
        }

        if let Some(dir) = self.layout.default_projects_dir() {
            items.extend(scan_directory(&dir, |name| {
                Some(WallpaperId::Default(name.to_owned()))
            }));
        }

        if let Some(dir) = self.layout.my_projects_dir() {
            items.extend(scan_directory(&dir, |name| {
                Some(WallpaperId::Local(name.to_owned()))
            }));
        }

        items.sort_by_key(|item| item.title().to_lowercase());
        items.dedup_by(|a, b| a.id() == b.id());

        Library {
            items,
            assets_dir: self.layout.assets_dir(),
        }
    }
}

fn scan_directory(dir: &Path, id_for: impl Fn(&str) -> Option<WallpaperId>) -> Vec<LibraryItem> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(path = %dir.display(), error = %error, "Could not list wallpaper directory.");

            return Vec::new();
        }
    };
    let mut items = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();

        if !path.is_dir() || !path.join(PROJECT_FILE).is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(id) = id_for(name) else {
            continue;
        };

        items.push(load_item(id, &path));
    }

    items
}

fn load_item(id: WallpaperId, directory: &Path) -> LibraryItem {
    match project::parse_project(id.clone(), directory) {
        Ok(wallpaper) => LibraryItem::Wallpaper(wallpaper),
        Err(error) => {
            tracing::warn!(path = %directory.display(), error = %error, "Skipped malformed project.");

            LibraryItem::Broken(BrokenProject {
                id,
                directory: directory.to_path_buf(),
                error: error.to_string(),
            })
        }
    }
}
