//! JSON-LD Error types

use thiserror::Error;

/// Errors that can occur during JSON-LD processing
#[derive(Debug, Error)]
pub enum JsonLdError {
    /// JSON parsing error
    #[error("JSON parse error: {0}")]
    JsonParseError(String),

    /// Context processing error
    #[error("Context error: {0}")]
    ContextError(String),

    /// Expansion error
    #[error("Expansion error: {0}")]
    ExpansionError(String),

    /// Compaction error
    #[error("Compaction error: {0}")]
    CompactionError(String),

    /// Framing error
    #[error("Framing error: {0}")]
    FramingError(String),

    /// RDF conversion error
    #[error("RDF conversion error: {0}")]
    RdfConversionError(String),

    /// Invalid IRI
    #[error("Invalid IRI: {0}")]
    InvalidIri(String),

    /// Invalid document structure
    #[error("Invalid document structure: {0}")]
    InvalidDocument(String),

    /// Remote context loading not supported
    #[error("Remote context loading not supported: {0}")]
    RemoteContextNotSupported(String),
}

impl JsonLdError {
    /// Create a JSON parse error
    pub fn json_parse(msg: impl Into<String>) -> Self {
        JsonLdError::JsonParseError(msg.into())
    }

    /// Create a context error
    pub fn context(msg: impl Into<String>) -> Self {
        JsonLdError::ContextError(msg.into())
    }

    /// Create an expansion error
    pub fn expansion(msg: impl Into<String>) -> Self {
        JsonLdError::ExpansionError(msg.into())
    }

    /// Create a compaction error
    pub fn compaction(msg: impl Into<String>) -> Self {
        JsonLdError::CompactionError(msg.into())
    }

    /// Create a framing error
    pub fn framing(msg: impl Into<String>) -> Self {
        JsonLdError::FramingError(msg.into())
    }

    /// Create an RDF conversion error
    pub fn rdf_conversion(msg: impl Into<String>) -> Self {
        JsonLdError::RdfConversionError(msg.into())
    }
}

/// Result type for JSON-LD operations
pub type JsonLdResult<T> = Result<T, JsonLdError>;
