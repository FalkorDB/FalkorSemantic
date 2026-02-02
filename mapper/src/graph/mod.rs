//! Graph Mapping Module
//!
//! Provides functionality for mapping RDF triples to FalkorDB graph structures.

mod cypher;
mod schema;

pub use cypher::{CypherGenerator, GraphBuilder};
pub use schema::{
    rdf_predicates, sanitize_identifier, Edge, GraphElement, LiteralNode, NodeType, ResourceNode,
};
