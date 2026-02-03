//! Error types for the SEC SGML parser

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid SGML structure: {0}")]
    InvalidStructure(String),

    #[error("Encoding error: unable to decode bytes")]
    EncodingError,

    #[error("UU-decode error: {0}")]
    UuDecodeError(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ParseError>;