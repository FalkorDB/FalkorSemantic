//! FalkorSemantic Storage
//!
//! This crate provides storage functionality for RDF data,
//! including IRI dictionary encoding and namespace persistence.

mod cache;
mod dictionary;
mod index;
mod namespace_store;
mod statistics;

pub use cache::{CacheStats, CachedPlan, LruCache, NamespaceCache, QueryPlanCache};
pub use dictionary::{IriDictionary, IriId, UNKNOWN_IRI_ID};
pub use index::{
    rdf_predicates, IndexHint, IndexManager, LocalNameIndex, NamespaceIndex, PredicateIndex,
    TypeIndex,
};
pub use namespace_store::{NamespaceMapping, NamespaceStore};
pub use statistics::{SelectivityHistogram, Statistics, StatisticsCollector, StatisticsSummary};

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
