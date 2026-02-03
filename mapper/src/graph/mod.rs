//! Graph Mapping Module
//!
//! Provides functionality for mapping RDF triples to FalkorDB graph structures.

mod cypher;
mod delete;
mod schema;

pub use cypher::{CypherGenerator, GraphBuilder};
pub use delete::{DeleteGenerator, DeleteOptions};
pub use schema::{
    escape_cypher_identifier, escape_cypher_string, rdf_predicates, sanitize_identifier, Edge,
    GraphElement, LiteralNode, NodeType, PropertyValue, ResourceNode,
};
