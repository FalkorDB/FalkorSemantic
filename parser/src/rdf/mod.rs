//! RDF Data Model
//!
//! This module provides core RDF data types including IRIs, literals,
//! blank nodes, triples, quads, and namespace management.

mod blank_node;
mod iri;
pub mod jsonld;
mod literal;
mod namespace;
pub mod serializer;
mod triple;

pub use blank_node::{BlankNode, BlankNodeScope};
pub use iri::Iri;
pub use jsonld::{ContextResolver, JsonLdError, JsonLdParser, JsonLdToRdf};
pub use literal::{xsd, Literal, RDF_LANG_STRING};
pub use namespace::{well_known, NamespaceRegistry, PrefixedName};
pub use serializer::{
    GraphSerializer, JsonLdSerializer, NQuadsSerializer, NTriplesSerializer, QuadSerializer,
    SerializerError, SerializerResult, TriGSerializer, TripleSerializer, TurtleSerializer,
};
pub use triple::{
    GraphName, GraphScope, Object, Predicate, Quad, QuadPattern, Subject, Triple, TriplePattern,
};
