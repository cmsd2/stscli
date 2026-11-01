use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StsCliError {
    #[error("{0}")]
    Message(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("cannot read {}: {source}", path.display())]
    Ini {
        path: PathBuf,
        #[source]
        source: ini::Error,
    },

    #[error("JSON encoder error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("STS {operation} failed: {message}")]
    Sts {
        operation: &'static str,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, StsCliError>;
