//! What can go wrong with a local model.

use std::path::Path;

use thiserror::Error;

/// A model could not be installed, loaded or run.
#[derive(Debug, Error)]
pub enum Error {
    /// The model's files are not installed.
    #[error("model {0} is not installed")]
    NotInstalled(String),
    /// No model has this id.
    #[error("unknown model {0}")]
    Unknown(String),
    /// A file could not be read or written.
    #[error("{path}: {message}")]
    File {
        /// The file.
        path: String,
        /// What happened.
        message: String,
    },
    /// A download failed or did not match its pinned hash.
    #[error("download: {0}")]
    Download(String),
    /// The model's files are not what they should be.
    #[error("model: {0}")]
    Model(String),
    /// The tensor library failed.
    #[error("inference: {0}")]
    Inference(#[from] candle_core::Error),
}

impl Error {
    pub(crate) fn file(path: &Path, error: impl std::fmt::Display) -> Self {
        Error::File { path: path.display().to_string(), message: error.to_string() }
    }
}

/// Result of this crate.
pub type Result<T> = std::result::Result<T, Error>;
