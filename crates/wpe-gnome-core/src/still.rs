//! Still frames for consumers that can only show a picture (the lock screen, static blur).

use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StillImageError {
    #[error("could not decode {path}: {source}")]
    Decode {
        path: PathBuf,
        source: image::ImageError,
    },
    #[error("could not write {path}: {source}")]
    Encode {
        path: PathBuf,
        source: image::ImageError,
    },
    #[error("could not create {path}: {source}")]
    Directory {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Converts a preview (JPEG, PNG, WebP or the first frame of a GIF) into a PNG at `output`.
pub fn write_still(preview: &Path, output: &Path) -> Result<(), StillImageError> {
    let image = image::open(preview).map_err(|source| StillImageError::Decode {
        path: preview.to_path_buf(),
        source,
    })?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|source| StillImageError::Directory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    image
        .to_rgb8()
        .save_with_format(output, image::ImageFormat::Png)
        .map_err(|source| StillImageError::Encode {
            path: output.to_path_buf(),
            source,
        })
}
