//! RDF Data Model
//!
//! This module provides core RDF data types including IRIs, literals,
//! blank nodes, triples, quads, and namespace management.

mod blank_node;
mod iri;
mod literal;
mod namespace;
mod triple;

pub use blank_node::{BlankNode, BlankNodeScope};
pub use iri::Iri;
pub use literal::{xsd, Literal, RDF_LANG_STRING};
pub use namespace::{well_known, NamespaceRegistry, PrefixedName};
pub use triple::{GraphName, Object, Predicate, Quad, Subject, Triple};
