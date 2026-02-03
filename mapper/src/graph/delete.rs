//! DELETE Cypher Statement Generator
//!
//! Generates Cypher DELETE statements for removing RDF data from FalkorDB.

use falkorsemantic_parser::rdf::{
    GraphName, GraphScope, Object, QuadPattern, Subject, Triple, TriplePattern,
};

use super::schema::{escape_cypher_string, sanitize_identifier};
use crate::MapperError;

/// Options for DELETE operations
#[derive(Debug, Clone, Default)]
pub struct DeleteOptions {
    /// Whether to delete orphaned literal nodes after edge deletion
    pub cascade_literals: bool,
    /// Whether to delete orphaned blank nodes after edge deletion
    pub cascade_blank_nodes: bool,
    /// Whether to use DETACH DELETE (removes relationships too)
    pub detach: bool,
}

impl DeleteOptions {
    /// Create new delete options with defaults (no cascading)
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable cascading deletion of orphaned literals
    pub fn with_cascade_literals(mut self) -> Self {
        self.cascade_literals = true;
        self
    }

    /// Enable cascading deletion of orphaned blank nodes
    pub fn with_cascade_blank_nodes(mut self) -> Self {
        self.cascade_blank_nodes = true;
        self
    }

    /// Enable full cascading (literals and blank nodes)
    pub fn with_cascade_all(mut self) -> Self {
        self.cascade_literals = true;
        self.cascade_blank_nodes = true;
        self
    }

    /// Enable DETACH DELETE for nodes
    pub fn with_detach(mut self) -> Self {
        self.detach = true;
        self
    }
}

/// Generates Cypher DELETE statements for RDF data
#[derive(Debug, Default)]
pub struct DeleteGenerator {
    /// Delete options
    options: DeleteOptions,
}

impl DeleteGenerator {
    /// Create a new delete generator with default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a delete generator with specific options
    pub fn with_options(options: DeleteOptions) -> Self {
        Self { options }
    }

    /// Set delete options
    pub fn set_options(&mut self, options: DeleteOptions) {
        self.options = options;
    }

    /// Get the current delete options
    pub fn options(&self) -> &DeleteOptions {
        &self.options
    }

    /// Generate Cypher to delete a specific triple
    pub fn generate_delete_triple(&self, triple: &Triple) -> Result<Vec<String>, MapperError> {
        let pattern = TriplePattern::from_triple(triple);
        self.generate_delete_pattern(&pattern, GraphScope::Default)
    }

    /// Generate Cypher to delete triples matching a pattern
    pub fn generate_delete_pattern(
        &self,
        pattern: &TriplePattern,
        graph_scope: GraphScope,
    ) -> Result<Vec<String>, MapperError> {
        let mut statements = Vec::new();

        // Build MATCH clause
        let match_clause = self.build_match_clause(pattern, &graph_scope)?;

        // Build DELETE clause
        let delete_clause = self.build_delete_clause(pattern);

        statements.push(format!("{}\n{}", match_clause, delete_clause));

        // Add cascading cleanup if enabled
        if self.options.cascade_literals {
            statements.push(self.generate_orphan_literal_cleanup());
        }
        if self.options.cascade_blank_nodes {
            statements.push(self.generate_orphan_blank_node_cleanup());
        }

        Ok(statements)
    }

    /// Generate Cypher to delete quads matching a pattern
    pub fn generate_delete_quad_pattern(
        &self,
        pattern: &QuadPattern,
    ) -> Result<Vec<String>, MapperError> {
        self.generate_delete_pattern(&pattern.pattern, pattern.graph_scope())
    }

    /// Generate Cypher to delete all triples with a given subject
    pub fn generate_delete_subject(&self, subject: &Subject) -> Result<Vec<String>, MapperError> {
        let pattern = TriplePattern::with_subject(subject.clone());
        self.generate_delete_pattern(&pattern, GraphScope::Default)
    }

    /// Generate Cypher to delete all triples with a given predicate
    pub fn generate_delete_predicate(
        &self,
        predicate: &falkorsemantic_parser::rdf::Predicate,
    ) -> Result<Vec<String>, MapperError> {
        let pattern = TriplePattern::with_predicate(predicate.clone());
        self.generate_delete_pattern(&pattern, GraphScope::Default)
    }

    fn build_match_clause(
        &self,
        pattern: &TriplePattern,
        graph_scope: &GraphScope,
    ) -> Result<String, MapperError> {
        let mut conditions = Vec::new();

        // Subject matching
        let subject_match = match &pattern.subject {
            Some(Subject::Iri(iri)) => {
                format!("(s {{uri: '{}'}})", escape_cypher_string(iri.as_str()))
            }
            Some(Subject::BlankNode(bn)) => {
                format!(
                    "(s:BlankNode {{uri: '_:{}'}})",
                    escape_cypher_string(bn.label())
                )
            }
            None => "(s)".to_string(),
        };

        // Predicate matching - edge pattern
        let edge_match = match &pattern.predicate {
            Some(pred) => {
                let edge_type = sanitize_identifier(pred.local_name());
                format!(
                    "-[r:{}{{predicate: '{}'}}]->",
                    edge_type,
                    escape_cypher_string(pred.as_str())
                )
            }
            None => "-[r]->".to_string(),
        };

        // Object matching
        let object_match = match &pattern.object {
            Some(Object::Iri(iri)) => {
                format!("(o {{uri: '{}'}})", escape_cypher_string(iri.as_str()))
            }
            Some(Object::BlankNode(bn)) => {
                format!(
                    "(o:BlankNode {{uri: '_:{}'}})",
                    escape_cypher_string(bn.label())
                )
            }
            Some(Object::Literal(lit)) => {
                let value_repr = format!("'{}'", escape_cypher_string(lit.value()));
                format!("(o:Literal {{value: {}}})", value_repr)
            }
            None => "(o)".to_string(),
        };

        // Graph scope filtering
        match graph_scope {
            GraphScope::Default => {
                // Default graph - no graph property on relationship
                conditions.push("r.graph IS NULL".to_string());
            }
            GraphScope::Named(graph_name) => {
                let graph_uri = match graph_name {
                    GraphName::Iri(iri) => iri.as_str().to_string(),
                    GraphName::BlankNode(bn) => format!("_:{}", bn.label()),
                };
                conditions.push(format!("r.graph = '{}'", escape_cypher_string(&graph_uri)));
            }
            GraphScope::All => {
                // No graph filtering - match all graphs
            }
        }

        let pattern_str = format!("{}{}{}", subject_match, edge_match, object_match);

        if conditions.is_empty() {
            Ok(format!("MATCH {}", pattern_str))
        } else {
            Ok(format!(
                "MATCH {}\nWHERE {}",
                pattern_str,
                conditions.join(" AND ")
            ))
        }
    }

    fn build_delete_clause(&self, pattern: &TriplePattern) -> String {
        // For pattern deletion, we delete the relationship
        // If it's an all-wildcard pattern (delete everything), use DETACH DELETE on nodes
        if pattern.is_all_wildcard() && self.options.detach {
            "DETACH DELETE s, o".to_string()
        } else {
            // Normal case: delete the relationship
            "DELETE r".to_string()
        }
    }

    /// Generate Cypher to clean up orphaned Literal nodes
    pub fn generate_orphan_literal_cleanup(&self) -> String {
        "MATCH (l:Literal) WHERE NOT ()-[]->(l) DELETE l".to_string()
    }

    /// Generate Cypher to clean up orphaned BlankNode nodes
    pub fn generate_orphan_blank_node_cleanup(&self) -> String {
        "MATCH (b:BlankNode) WHERE NOT ()-[]->(b) AND NOT (b)-[]->() DELETE b".to_string()
    }

    /// Generate batch delete statements for multiple patterns
    pub fn generate_batch_delete(
        &self,
        patterns: &[TriplePattern],
        graph_scope: GraphScope,
    ) -> Result<Vec<String>, MapperError> {
        let mut all_statements = Vec::new();

        for pattern in patterns {
            all_statements.extend(self.generate_delete_pattern(pattern, graph_scope.clone())?);
        }

        // Deduplicate cleanup statements if cascading is enabled
        if self.options.cascade_literals || self.options.cascade_blank_nodes {
            all_statements.dedup();
        }

        Ok(all_statements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falkorsemantic_parser::rdf::{Iri, Literal};

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_delete_options_default() {
        let opts = DeleteOptions::new();
        assert!(!opts.cascade_literals);
        assert!(!opts.cascade_blank_nodes);
        assert!(!opts.detach);
    }

    #[test]
    fn test_delete_options_cascade() {
        let opts = DeleteOptions::new().with_cascade_all();
        assert!(opts.cascade_literals);
        assert!(opts.cascade_blank_nodes);
    }

    #[test]
    fn test_delete_specific_triple() {
        let gen = DeleteGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://example.org/knows"),
            test_iri("http://example.org/Bob"),
        );

        let statements = gen.generate_delete_triple(&triple).unwrap();
        assert!(!statements.is_empty());
        assert!(statements[0].contains("MATCH"));
        assert!(statements[0].contains("DELETE"));
        assert!(statements[0].contains("Alice"));
        assert!(statements[0].contains("knows"));
    }

    #[test]
    fn test_delete_pattern_with_wildcard_subject() {
        let gen = DeleteGenerator::new();
        let pattern = TriplePattern::new()
            .predicate(test_iri("http://example.org/knows"))
            .object(test_iri("http://example.org/Bob"));

        let statements = gen
            .generate_delete_pattern(&pattern, GraphScope::Default)
            .unwrap();

        assert!(statements[0].contains("(s)"));
        assert!(statements[0].contains("knows"));
        assert!(statements[0].contains("Bob"));
    }

    #[test]
    fn test_delete_pattern_all_wildcards() {
        let gen = DeleteGenerator::with_options(DeleteOptions::new().with_detach());
        let pattern = TriplePattern::new();

        let statements = gen
            .generate_delete_pattern(&pattern, GraphScope::All)
            .unwrap();

        assert!(statements[0].contains("MATCH (s)-[r]->(o)"));
        assert!(statements[0].contains("DETACH DELETE"));
    }

    #[test]
    fn test_delete_with_literal_object() {
        let gen = DeleteGenerator::new();
        let pattern = TriplePattern::new()
            .subject(test_iri("http://example.org/Alice"))
            .predicate(test_iri("http://example.org/name"))
            .object(Literal::new("Alice"));

        let statements = gen
            .generate_delete_pattern(&pattern, GraphScope::Default)
            .unwrap();

        assert!(statements[0].contains("Literal"));
        assert!(statements[0].contains("Alice"));
    }

    #[test]
    fn test_delete_with_cascade() {
        let gen = DeleteGenerator::with_options(DeleteOptions::new().with_cascade_all());
        let pattern = TriplePattern::with_subject(test_iri("http://example.org/Alice"));

        let statements = gen
            .generate_delete_pattern(&pattern, GraphScope::Default)
            .unwrap();

        assert!(statements.len() == 3); // main delete + 2 cleanup queries
        assert!(statements[1].contains("Literal"));
        assert!(statements[2].contains("BlankNode"));
    }

    #[test]
    fn test_delete_named_graph_scope() {
        let gen = DeleteGenerator::new();
        let pattern = TriplePattern::new();
        let graph = GraphScope::Named(GraphName::Iri(test_iri("http://example.org/graph1")));

        let statements = gen.generate_delete_pattern(&pattern, graph).unwrap();

        assert!(statements[0].contains("r.graph = 'http://example.org/graph1'"));
    }

    #[test]
    fn test_orphan_cleanup_queries() {
        let gen = DeleteGenerator::new();

        let literal_cleanup = gen.generate_orphan_literal_cleanup();
        assert!(literal_cleanup.contains("Literal"));
        assert!(literal_cleanup.contains("NOT ()-[]->(l)"));

        let blank_cleanup = gen.generate_orphan_blank_node_cleanup();
        assert!(blank_cleanup.contains("BlankNode"));
    }

    #[test]
    fn test_quad_pattern_delete() {
        let gen = DeleteGenerator::new();
        let quad_pattern = QuadPattern::in_graph(
            TriplePattern::with_subject(test_iri("http://example.org/s")),
            test_iri("http://example.org/graph"),
        );

        let statements = gen.generate_delete_quad_pattern(&quad_pattern).unwrap();

        assert!(statements[0].contains("http://example.org/s"));
        assert!(statements[0].contains("r.graph = 'http://example.org/graph'"));
    }

    #[test]
    fn test_batch_delete() {
        let gen = DeleteGenerator::new();
        let patterns = vec![
            TriplePattern::with_subject(test_iri("http://example.org/s1")),
            TriplePattern::with_subject(test_iri("http://example.org/s2")),
        ];

        let statements = gen
            .generate_batch_delete(&patterns, GraphScope::Default)
            .unwrap();

        assert!(statements.len() >= 2);
    }
}
