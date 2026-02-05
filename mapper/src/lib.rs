//! `FalkorSemantic` Mapper
//!
//! This crate provides mapping functionality for transforming semantic data
//! (RDF triples/quads) to `FalkorDB` graph structures (nodes, edges, Cypher).

pub mod graph;
pub mod query;

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
    delete_gen: graph::DeleteGenerator,
}

impl Mapper {
    /// Create a new mapper instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            cypher_gen: graph::CypherGenerator::new(),
            delete_gen: graph::DeleteGenerator::new(),
        }
    }

    /// Create a mapper with custom delete options
    #[must_use]
    pub fn with_delete_options(options: graph::DeleteOptions) -> Self {
        Self {
            cypher_gen: graph::CypherGenerator::new(),
            delete_gen: graph::DeleteGenerator::with_options(options),
        }
    }

    /// Map a triple to Cypher statements
    pub fn map_triple(&self, triple: &falkorsemantic_parser::rdf::Triple) -> Result<Vec<String>> {
        self.cypher_gen.generate_triple(triple)
    }

    /// Map a quad to Cypher statements
    pub fn map_quad(&self, quad: &falkorsemantic_parser::rdf::Quad) -> Result<Vec<String>> {
        self.cypher_gen.generate_quad(quad)
    }

    /// Map multiple triples to Cypher statements
    pub fn map_triples(
        &self,
        triples: &[falkorsemantic_parser::rdf::Triple],
    ) -> Result<Vec<String>> {
        self.cypher_gen.generate_batch(triples)
    }

    /// Generate DELETE Cypher for a specific triple
    pub fn delete_triple(
        &self,
        triple: &falkorsemantic_parser::rdf::Triple,
    ) -> Result<Vec<String>> {
        self.delete_gen.generate_delete_triple(triple)
    }

    /// Generate DELETE Cypher for a triple pattern
    pub fn delete_pattern(
        &self,
        pattern: &falkorsemantic_parser::rdf::TriplePattern,
        graph_scope: falkorsemantic_parser::rdf::GraphScope,
    ) -> Result<Vec<String>> {
        self.delete_gen
            .generate_delete_pattern(pattern, graph_scope)
    }

    /// Generate DELETE Cypher for a quad pattern
    pub fn delete_quad_pattern(
        &self,
        pattern: &falkorsemantic_parser::rdf::QuadPattern,
    ) -> Result<Vec<String>> {
        self.delete_gen.generate_delete_quad_pattern(pattern)
    }

    /// Generate DELETE Cypher for multiple patterns
    pub fn delete_patterns(
        &self,
        patterns: &[falkorsemantic_parser::rdf::TriplePattern],
        graph_scope: falkorsemantic_parser::rdf::GraphScope,
    ) -> Result<Vec<String>> {
        self.delete_gen.generate_batch_delete(patterns, graph_scope)
    }

    /// Get a reference to the Cypher generator for advanced usage
    #[must_use]
    pub const fn cypher_generator(&self) -> &graph::CypherGenerator {
        &self.cypher_gen
    }

    /// Get a reference to the delete generator for advanced usage
    #[must_use]
    pub const fn delete_generator(&self) -> &graph::DeleteGenerator {
        &self.delete_gen
    }

    /// Get a mutable reference to the delete generator
    pub fn delete_generator_mut(&mut self) -> &mut graph::DeleteGenerator {
        &mut self.delete_gen
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
    use falkorsemantic_parser::rdf::{GraphScope, Iri, Literal, Triple, TriplePattern};

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

    #[test]
    fn test_mapper_delete_triple() {
        let mapper = Mapper::new();
        let triple = Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            Literal::new("value"),
        );
        let result = mapper.delete_triple(&triple);
        assert!(result.is_ok());
        let statements = result.unwrap();
        assert!(!statements.is_empty());
        assert!(statements[0].contains("DELETE"));
    }

    #[test]
    fn test_mapper_delete_pattern() {
        let mapper = Mapper::new();
        let pattern = TriplePattern::with_subject(test_iri("http://example.org/s"));
        let result = mapper.delete_pattern(&pattern, GraphScope::Default);
        assert!(result.is_ok());
        let statements = result.unwrap();
        assert!(statements[0].contains("MATCH"));
        assert!(statements[0].contains("DELETE"));
    }

    #[test]
    fn test_mapper_with_delete_options() {
        let mapper = Mapper::with_delete_options(graph::DeleteOptions::new().with_cascade_all());
        let pattern = TriplePattern::with_subject(test_iri("http://example.org/s"));
        let result = mapper.delete_pattern(&pattern, GraphScope::Default);
        assert!(result.is_ok());
        let statements = result.unwrap();
        // Should include cleanup queries
        assert!(statements.len() >= 2);
    }
}
