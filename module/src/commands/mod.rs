//! Command handlers for the FalkorSemantic Redis module

mod rdf_insert;
mod rdf_namespaces;

pub use rdf_insert::rdf_insert;
pub use rdf_namespaces::rdf_namespaces;
