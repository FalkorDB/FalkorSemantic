//! FalkorSemantic Storage
//!
//! This crate provides storage functionality for RDF data,
//! including IRI dictionary encoding and namespace persistence.

mod dictionary;
mod namespace_store;

pub use dictionary::{IriDictionary, IriId, UNKNOWN_IRI_ID};
pub use namespace_store::{NamespaceMapping, NamespaceStore};

use thiserror::Error;

/// Storage error types
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result type for storage operations
pub type Result<T> = std::result::Result<T, StorageError>;
