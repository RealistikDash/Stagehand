use std::path::PathBuf;

use thiserror::Error;

const APP_DIR: &str = "wpe-gnome";

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("neither $HOME nor $XDG_CONFIG_HOME is set")]
    NoHome,
}

/// Every directory the program touches, resolved once at startup from the environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub data_dir: PathBuf,
    /// Where `just install` puts helper programs such as linux-wallpaperengine.
    pub libexec_dir: PathBuf,
    pub bin_dir: PathBuf,
}

impl Paths {
    pub fn from_environment() -> Result<Self, PathsError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(PathsError::NoHome)?;
        let config_home = directory_or("XDG_CONFIG_HOME", || home.join(".config"));
        let cache_home = directory_or("XDG_CACHE_HOME", || home.join(".cache"));
        let data_home = directory_or("XDG_DATA_HOME", || home.join(".local/share"));
        let runtime_dir = directory_or("XDG_RUNTIME_DIR", || cache_home.clone());

        Ok(Self {
            config_file: config_home.join(APP_DIR).join("config.json"),
            cache_dir: cache_home.join(APP_DIR),
            runtime_dir: runtime_dir.join(APP_DIR),
            data_dir: data_home.join(APP_DIR),
            libexec_dir: home.join(".local/libexec").join(APP_DIR),
            bin_dir: home.join(".local/bin"),
            home,
        })
    }
}

fn directory_or(variable: &str, fallback: impl FnOnce() -> PathBuf) -> PathBuf {
    std::env::var_os(variable).map_or_else(fallback, PathBuf::from)
}
