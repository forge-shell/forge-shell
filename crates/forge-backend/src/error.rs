#![allow(dead_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("command not found on PATH: '{command}'")]
    CommandNotFound { command: String },

    #[error("unsupported HIR construct: {reason}")]
    Unsupported { reason: String },

    #[error("IO error during path resolution: {0}")]
    Io(#[from] std::io::Error),
}
