//! JSON-LD Parser
//!
//! This module provides JSON-LD parsing functionality including:
//! - Document parsing from JSON strings
//! - @context resolution
//! - Expansion and compaction algorithms
//! - Basic framing support
//! - Conversion to RDF triples

mod parser;
mod error;
mod context;
mod conversion;

pub use error::JsonLdError;
pub use parser::JsonLdParser;
pub use context::ContextResolver;
pub use conversion::JsonLdToRdf;
