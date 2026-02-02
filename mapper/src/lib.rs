//! FalkorSemantic Mapper
//!
//! This crate provides mapping functionality for transforming semantic data
//! (RDF triples/quads) to FalkorDB graph structures (nodes, edges, Cypher).

pub mod graph;

use thiserror::Error;

/// Mapper error types
#[derive(Debug, Error)]
pub enum MapperError {
    #[error("Mapping error: {0}")]
    MappingError(String),
    #[error("Invalid transformation: {0}")]
    InvalidTransformation(String),
}

/// Result type for mapper operations
pub type Result<T> = std::result::Result<T, MapperError>;

/// Mapper for converting semantic data to graph structures
pub struct Mapper {
    cypher_gen: graph::CypherGenerator,
}

impl Mapper {
    /// Create a new mapper instance
    pub fn new() -> Self {
        Self {
            cypher_gen: graph::CypherGenerator::new(),
        }
    }

    /// Map a triple to Cypher statements
    pub fn map_triple(
        &self,
        triple: &falkorsemantic_parser::rdf::Triple,
    ) -> Result<Vec<String>> {
        self.cypher_gen.generate_triple(triple)
    }

    /// Map a quad to Cypher statements
    pub fn map_quad(
        &self,
        quad: &falkorsemantic_parser::rdf::Quad,
    ) -> Result<Vec<String>> {
        self.cypher_gen.generate_quad(quad)
    }

    /// Map multiple triples to Cypher statements
    pub fn map_triples(
        &self,
        triples: &[falkorsemantic_parser::rdf::Triple],
    ) -> Result<Vec<String>> {
        self.cypher_gen.generate_batch(triples)
    }

    /// Get a reference to the Cypher generator for advanced usage
    pub fn cypher_generator(&self) -> &graph::CypherGenerator {
        &self.cypher_gen
    }
}

impl Default for Mapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falkorsemantic_parser::rdf::{Iri, Literal, Triple};

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_mapper_creation() {
        let _mapper = Mapper::new();
    }

    #[test]
    fn test_mapper_triple() {
        let mapper = Mapper::new();
        let triple = Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            Literal::new("value"),
        );
        let result = mapper.map_triple(&triple);
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
