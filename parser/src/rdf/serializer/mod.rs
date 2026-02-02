//! RDF Serializers
//!
//! This module provides serializers for RDF data in various formats:
//! - N-Triples: Simple line-based format for triples
//! - N-Quads: N-Triples extended with named graphs
//! - Turtle: Human-readable format with prefix support
//! - TriG: Turtle extended with named graphs
//! - JSON-LD: JSON-based linked data format

mod error;
mod nquads;
mod ntriples;
mod traits;
mod trig;
mod turtle;
mod jsonld;

pub use error::{SerializerError, SerializerResult};
pub use nquads::NQuadsSerializer;
pub use ntriples::NTriplesSerializer;
pub use traits::{QuadSerializer, TripleSerializer, GraphSerializer};
pub use trig::TriGSerializer;
pub use turtle::TurtleSerializer;
pub use jsonld::JsonLdSerializer;
