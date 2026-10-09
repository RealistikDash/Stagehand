use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

use crate::config::schema::Config;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid configuration: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

#[derive(Clone, Debug)]
pub struct ConfigRepository {
    path: PathBuf,
}

impl ConfigRepository {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A missing file yields the default configuration; a malformed one is an error so that a
    /// typo never silently wipes the user's assignments.
    pub fn load(&self) -> Result<Config, ConfigError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Config::default());
            }
            Err(source) => {
                return Err(ConfigError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };

        serde_json::from_str(&text).map_err(|source| ConfigError::Parse {
            path: self.path.clone(),
            source,
        })
    }

    /// Writes atomically: the new content goes to a sibling file that is renamed over the old one.
    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        let write_error = |source| ConfigError::Write {
            path: self.path.clone(),
            source,
        };

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(write_error)?;
        }

        let text = serde_json::to_string_pretty(config).map_err(|source| ConfigError::Parse {
            path: self.path.clone(),
            source,
        })?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(write_error)?;
        std::fs::rename(&temporary, &self.path).map_err(write_error)
    }
}
