//! Cypher Query Generator
//!
//! Generates Cypher statements for inserting RDF data into FalkorDB.

use falkorsemantic_parser::rdf::{Literal, Object, Quad, Subject, Triple};

use super::schema::{rdf_predicates, sanitize_identifier, Edge, LiteralNode, ResourceNode};
use crate::MapperError;

/// Generates Cypher statements for RDF triples
#[derive(Debug, Default)]
pub struct CypherGenerator {
    /// Whether to use MERGE instead of CREATE for idempotent operations
    use_merge: bool,
}

impl CypherGenerator {
    /// Create a new Cypher generator
    pub fn new() -> Self {
        Self { use_merge: true }
    }

    /// Create a generator that uses CREATE instead of MERGE
    pub fn with_create() -> Self {
        Self { use_merge: false }
    }

    /// Set whether to use MERGE (true) or CREATE (false)
    pub fn set_use_merge(&mut self, use_merge: bool) {
        self.use_merge = use_merge;
    }

    fn operation(&self) -> &str {
        if self.use_merge {
            "MERGE"
        } else {
            "CREATE"
        }
    }

    /// Generate Cypher for a triple
    ///
    /// Returns a single combined Cypher statement that handles the entire triple,
    /// ensuring variable bindings are preserved within the query.
    pub fn generate_triple(&self, triple: &Triple) -> Result<Vec<String>, MapperError> {
        // Check if this is an rdf:type statement
        if triple.predicate.as_str() == rdf_predicates::RDF_TYPE {
            // Handle rdf:type as a label assignment
            if let Object::Iri(type_iri) = &triple.object {
                let label = sanitize_identifier(type_iri.local_name());
                let subject_uri = self.subject_uri(&triple.subject);
                let is_blank = matches!(triple.subject, Subject::BlankNode(_));
                let node_label = if is_blank { "BlankNode" } else { "Resource" };

                // Use consistent MERGE pattern with other node creations
                let statement = format!(
                    "{} (n:{} {{uri: '{}'}}) SET n:{}, n.isBlank = {}",
                    self.operation(),
                    node_label,
                    escape_cypher_string(&subject_uri),
                    label,
                    is_blank
                );
                return Ok(vec![statement]);
            }
        }

        // Build a single combined query for the triple
        let mut parts = Vec::new();

        // Create/merge subject node
        let subject_uri = self.subject_uri(&triple.subject);
        let subject_var = "s";
        parts.push(self.generate_resource_node(subject_var, &subject_uri, &triple.subject));

        // Generate based on object type
        match &triple.object {
            Object::Iri(iri) => {
                // Object is a resource
                let object_uri = iri.as_str();
                parts.push(self.generate_resource_node_from_iri("o", object_uri));
                parts.push(self.generate_edge(
                    subject_var,
                    "o",
                    triple.predicate.as_str(),
                    triple.predicate.local_name(),
                ));
            }
            Object::BlankNode(bn) => {
                // Object is a blank node
                let bn_id = format!("_:{}", bn.label());
                parts.push(self.generate_blank_node("o", &bn_id));
                parts.push(self.generate_edge(
                    subject_var,
                    "o",
                    triple.predicate.as_str(),
                    triple.predicate.local_name(),
                ));
            }
            Object::Literal(lit) => {
                // Object is a literal - store as property on edge or as literal node
                parts.push(self.generate_literal_edge(subject_var, &triple.predicate, lit));
            }
        }

        // Combine all parts into a single statement with newlines for readability
        let combined = parts.join("\n");
        Ok(vec![combined])
    }

    /// Generate Cypher for a quad (triple in a named graph)
    pub fn generate_quad(&self, quad: &Quad) -> Result<Vec<String>, MapperError> {
        let mut statements = self.generate_triple(&quad.triple)?;

        // Add graph information if present
        if let Some(graph) = &quad.graph {
            let graph_uri = match graph {
                falkorsemantic_parser::rdf::GraphName::Iri(iri) => iri.as_str().to_string(),
                falkorsemantic_parser::rdf::GraphName::BlankNode(bn) => {
                    format!("_:{}", bn.label())
                }
            };

            // Wrap statements in a graph context
            statements = statements
                .into_iter()
                .map(|s| format!("// Graph: {}\n{}", graph_uri, s))
                .collect();
        }

        Ok(statements)
    }

    fn subject_uri(&self, subject: &Subject) -> String {
        match subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::BlankNode(bn) => format!("_:{}", bn.label()),
        }
    }

    fn generate_resource_node(&self, var: &str, uri: &str, subject: &Subject) -> String {
        let is_blank = matches!(subject, Subject::BlankNode(_));
        let label = if is_blank { "BlankNode" } else { "Resource" };

        // MERGE on label + uri only, then SET isBlank for consistency
        format!(
            "{} ({}:{} {{uri: '{}'}}) SET {}.isBlank = {}",
            self.operation(),
            var,
            label,
            escape_cypher_string(uri),
            var,
            is_blank
        )
    }

    fn generate_resource_node_from_iri(&self, var: &str, uri: &str) -> String {
        // MERGE on label + uri only, then SET isBlank for consistency
        format!(
            "{} ({}:Resource {{uri: '{}'}}) SET {}.isBlank = false",
            self.operation(),
            var,
            escape_cypher_string(uri),
            var
        )
    }

    fn generate_blank_node(&self, var: &str, id: &str) -> String {
        // MERGE on label + uri only, then SET isBlank for consistency
        format!(
            "{} ({}:BlankNode {{uri: '{}'}}) SET {}.isBlank = true",
            self.operation(),
            var,
            escape_cypher_string(id),
            var
        )
    }

    fn generate_edge(
        &self,
        from_var: &str,
        to_var: &str,
        predicate: &str,
        local_name: &str,
    ) -> String {
        let edge_type = sanitize_identifier(local_name);
        format!(
            "{} ({})-[:{}{{predicate: '{}'}}]->({})",
            self.operation(),
            from_var,
            edge_type,
            escape_cypher_string(predicate),
            to_var
        )
    }

    fn generate_literal_edge(
        &self,
        subject_var: &str,
        predicate: &falkorsemantic_parser::rdf::Iri,
        literal: &Literal,
    ) -> String {
        let edge_type = sanitize_identifier(predicate.local_name());
        let datatype = literal.datatype().as_str().to_string();

        // For numeric and boolean types, use unquoted values; otherwise quote as string
        let is_numeric_or_bool = literal.as_integer().is_some()
            || literal.as_float().is_some()
            || literal.as_bool().is_some();

        let value_repr = if is_numeric_or_bool {
            literal.value().to_string()
        } else {
            format!("'{}'", escape_cypher_string(literal.value()))
        };

        let lang_part = literal
            .language()
            .map(|l| format!(", language: '{}'", l))
            .unwrap_or_default();

        format!(
            "{} ({})-[:{}{{predicate: '{}', value: {}, datatype: '{}'{}}}]->(l:Literal{{value: {}, datatype: '{}'{}}})",
            self.operation(),
            subject_var,
            edge_type,
            escape_cypher_string(predicate.as_str()),
            value_repr,
            escape_cypher_string(&datatype),
            lang_part,
            value_repr,
            escape_cypher_string(&datatype),
            lang_part
        )
    }

    /// Generate a batch of statements for multiple triples
    pub fn generate_batch(&self, triples: &[Triple]) -> Result<Vec<String>, MapperError> {
        let mut all_statements = Vec::new();
        for triple in triples {
            all_statements.extend(self.generate_triple(triple)?);
        }
        Ok(all_statements)
    }
}

/// Escape a string for use in Cypher queries
fn escape_cypher_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Builder for constructing graph elements from RDF
#[derive(Debug, Default)]
pub struct GraphBuilder {
    /// Resource nodes indexed by URI
    resources: std::collections::HashMap<String, ResourceNode>,
    /// Literal nodes
    literals: Vec<LiteralNode>,
    /// Edges
    edges: Vec<(String, String, Edge, bool)>,
}

impl GraphBuilder {
    /// Create a new graph builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a triple to the graph
    pub fn add_triple(&mut self, triple: &Triple) {
        let subject_uri = match &triple.subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::BlankNode(bn) => format!("_:{}", bn.label()),
        };

        // Ensure subject node exists
        self.resources
            .entry(subject_uri.clone())
            .or_insert_with(|| match &triple.subject {
                Subject::Iri(iri) => ResourceNode::from_iri(iri.as_str().to_string()),
                Subject::BlankNode(bn) => ResourceNode::blank(bn.label().to_string()),
            });

        // Handle rdf:type
        if triple.predicate.as_str() == rdf_predicates::RDF_TYPE {
            if let Object::Iri(type_iri) = &triple.object {
                let label = sanitize_identifier(type_iri.local_name());
                if let Some(node) = self.resources.get_mut(&subject_uri) {
                    node.add_label(label);
                }
                return;
            }
        }

        // Handle object
        let (object_id, is_literal) = match &triple.object {
            Object::Iri(iri) => {
                let uri = iri.as_str().to_string();
                self.resources
                    .entry(uri.clone())
                    .or_insert_with(|| ResourceNode::from_iri(iri.as_str().to_string()));
                (uri, false)
            }
            Object::BlankNode(bn) => {
                let id = format!("_:{}", bn.label());
                self.resources
                    .entry(id.clone())
                    .or_insert_with(|| ResourceNode::blank(bn.label().to_string()));
                (id, false)
            }
            Object::Literal(lit) => {
                let id = format!("literal_{}", self.literals.len());
                self.literals.push(LiteralNode::new(
                    lit.value().to_string(),
                    lit.datatype().as_str().to_string(),
                    lit.language().map(|s| s.to_string()),
                ));
                (id, true)
            }
        };

        // Add edge
        let edge = Edge::new(
            triple.predicate.as_str().to_string(),
            triple.predicate.local_name().to_string(),
        );
        self.edges.push((subject_uri, object_id, edge, is_literal));
    }

    /// Get all resource nodes
    pub fn resources(&self) -> impl Iterator<Item = &ResourceNode> {
        self.resources.values()
    }

    /// Get all literal nodes
    pub fn literals(&self) -> impl Iterator<Item = &LiteralNode> {
        self.literals.iter()
    }

    /// Get all edges
    pub fn edges(&self) -> impl Iterator<Item = (&str, &str, &Edge, bool)> {
        self.edges
            .iter()
            .map(|(f, t, e, l)| (f.as_str(), t.as_str(), e, *l))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falkorsemantic_parser::rdf::Iri;

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_cypher_resource_triple() {
        let gen = CypherGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://example.org/knows"),
            test_iri("http://example.org/Bob"),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        // Should return exactly one combined statement
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];
        // The combined statement should contain all three MERGE clauses
        assert!(stmt.contains("Alice"));
        assert!(stmt.contains("Bob"));
        assert!(stmt.contains("knows"));
        // Verify it contains multiple MERGE operations in one statement
        assert_eq!(stmt.matches("MERGE").count(), 3);
    }

    #[test]
    fn test_cypher_literal_triple() {
        let gen = CypherGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://example.org/name"),
            Literal::new("Alice"),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        // Should return exactly one combined statement
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];
        assert!(stmt.contains("Alice"));
        assert!(stmt.contains("Literal"));
        // Subject MERGE + literal edge MERGE
        assert_eq!(stmt.matches("MERGE").count(), 2);
    }

    #[test]
    fn test_cypher_rdf_type() {
        let gen = CypherGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            test_iri("http://example.org/Person"),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        assert!(statements.iter().any(|s| s.contains(":Person")));
    }

    #[test]
    fn test_escape_cypher_string() {
        assert_eq!(escape_cypher_string("hello"), "hello");
        assert_eq!(escape_cypher_string("it's"), "it\\'s");
        assert_eq!(escape_cypher_string("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn test_graph_builder() {
        let mut builder = GraphBuilder::new();

        let triple = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            test_iri("http://example.org/Person"),
        );
        builder.add_triple(&triple);

        let resources: Vec<_> = builder.resources().collect();
        assert_eq!(resources.len(), 1);
        assert!(resources[0].labels.contains(&"Person".to_string()));
    }
}
