//! Serializer error types

use std::io;
use thiserror::Error;

/// Errors that can occur during RDF serialization
#[derive(Debug, Error)]
pub enum SerializerError {
    /// I/O error during writing
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// Invalid data for serialization
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Encoding error
    #[error("Encoding error: {0}")]
    EncodingError(String),

    /// Format-specific error
    #[error("Format error: {0}")]
    FormatError(String),
}

impl SerializerError {
    /// Create an invalid data error
    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    /// Create a format error
    pub fn format_error(msg: impl Into<String>) -> Self {
        Self::FormatError(msg.into())
    }
}

/// Result type for serializer operations
pub type SerializerResult<T> = Result<T, SerializerError>;
