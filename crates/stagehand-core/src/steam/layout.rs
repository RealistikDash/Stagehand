use std::path::Path;
use std::path::PathBuf;

use crate::steam::vdf::parse_vdf;

const WALLPAPER_ENGINE_APP_ID: &str = "431960";
const STEAM_ROOT_CANDIDATES: [&str; 4] = [
    ".local/share/Steam",
    ".steam/steam",
    ".steam/root",
    ".var/app/com.valvesoftware.Steam/.local/share/Steam",
];

/// The Steam directories relevant to Wallpaper Engine, resolved from `libraryfolders.vdf`.
/// Every path is checked for existence at resolution time, so an unmounted library simply
/// drops out until the next scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SteamLayout {
    libraries: Vec<PathBuf>,
}

impl SteamLayout {
    #[must_use]
    pub fn discover(home: &Path, extra_roots: &[PathBuf]) -> Self {
        let mut libraries: Vec<PathBuf> = Vec::new();
        let roots = STEAM_ROOT_CANDIDATES
            .iter()
            .map(|candidate| home.join(candidate))
            .chain(extra_roots.iter().cloned());

        for root in roots {
            let Ok(root) = root.canonicalize() else {
                continue;
            };

            push_unique(&mut libraries, root.clone());

            for library in libraries_listed_in(&root) {
                push_unique(&mut libraries, library);
            }
        }

        Self { libraries }
    }

    #[must_use]
    pub fn from_libraries(libraries: Vec<PathBuf>) -> Self {
        Self { libraries }
    }

    #[must_use]
    pub fn libraries(&self) -> &[PathBuf] {
        &self.libraries
    }

    /// The first library that has Wallpaper Engine installed.
    #[must_use]
    pub fn install_dir(&self) -> Option<PathBuf> {
        self.libraries
            .iter()
            .map(|library| library.join("steamapps/common/wallpaper_engine"))
            .find(|dir| dir.join("assets").is_dir())
    }

    #[must_use]
    pub fn assets_dir(&self) -> Option<PathBuf> {
        self.install_dir().map(|dir| dir.join("assets"))
    }

    #[must_use]
    pub fn default_projects_dir(&self) -> Option<PathBuf> {
        self.install_dir()
            .map(|dir| dir.join("projects/defaultprojects"))
            .filter(|dir| dir.is_dir())
    }

    #[must_use]
    pub fn my_projects_dir(&self) -> Option<PathBuf> {
        self.install_dir()
            .map(|dir| dir.join("projects/myprojects"))
            .filter(|dir| dir.is_dir())
    }

    /// Every existing `workshop/content/431960` directory across the libraries.
    #[must_use]
    pub fn workshop_dirs(&self) -> Vec<PathBuf> {
        self.libraries
            .iter()
            .map(|library| {
                library
                    .join("steamapps/workshop/content")
                    .join(WALLPAPER_ENGINE_APP_ID)
            })
            .filter(|dir| dir.is_dir())
            .collect()
    }
}

fn libraries_listed_in(root: &Path) -> Vec<PathBuf> {
    let vdf_path = root.join("steamapps/libraryfolders.vdf");
    let Ok(text) = std::fs::read_to_string(&vdf_path) else {
        return Vec::new();
    };
    let document = match parse_vdf(&text) {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(path = %vdf_path.display(), error = %error, "Skipped unreadable library list.");

            return Vec::new();
        }
    };
    let Some(folders) = document.get("libraryfolders") else {
        return Vec::new();
    };

    folders
        .entries()
        .iter()
        .filter_map(|(_, folder)| folder.get("path").and_then(|path| path.as_str()))
        .filter_map(|path| PathBuf::from(path).canonicalize().ok())
        .collect()
}

fn push_unique(libraries: &mut Vec<PathBuf>, library: PathBuf) {
    if !libraries.contains(&library) {
        libraries.push(library);
    }
}
