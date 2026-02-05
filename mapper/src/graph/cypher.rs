//! Cypher Query Generator
//!
//! Generates Cypher statements for inserting RDF data into FalkorDB.

use falkorsemantic_parser::rdf::{Literal, Object, Quad, Subject, Triple};

use super::schema::{
    escape_cypher_string, rdf_predicates, sanitize_identifier, Edge, LiteralNode, PropertyValue,
    ResourceNode,
};
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

                // For blank nodes with a type, use the type as the primary label
                // For resources, keep using Resource as the base label
                let node_label = if is_blank { &label } else { "Resource" };

                // Use consistent MERGE pattern with other node creations
                let statement = if is_blank {
                    // For blank nodes, use the type label directly without adding it again via SET
                    format!(
                        "{} (n:{} {{uri: '{}'}}) SET n.isBlank = {}",
                        self.operation(),
                        node_label,
                        escape_cypher_string(&subject_uri),
                        is_blank
                    )
                } else {
                    // For resources, add the type as an additional label
                    format!(
                        "{} (n:{} {{uri: '{}'}}) SET n:{}, n.isBlank = {}",
                        self.operation(),
                        node_label,
                        escape_cypher_string(&subject_uri),
                        label,
                        is_blank
                    )
                };
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
        let prop_name = sanitize_identifier(predicate.local_name());

        // For numeric and boolean types, use unquoted values; otherwise quote as string
        let is_numeric_or_bool = literal.as_integer().is_some()
            || literal.as_float().is_some()
            || literal.as_bool().is_some();

        let value_repr = if is_numeric_or_bool {
            literal.value().to_string()
        } else {
            format!("'{}'", escape_cypher_string(literal.value()))
        };

        // Store literal as a property on the subject node instead of creating a separate node
        format!("SET {}.{} = {}", subject_var, prop_name, value_repr)
    }

    /// Generate a batch of statements for multiple triples
    ///
    /// This method optimizes by grouping triples by subject and generating
    /// a single combined query per subject, reducing the number of queries.
    pub fn generate_batch(&self, triples: &[Triple]) -> Result<Vec<String>, MapperError> {
        use std::collections::HashMap;

        // Group triples by subject URI
        let mut subjects: HashMap<String, SubjectData> = HashMap::new();

        for triple in triples {
            let subject_uri = self.subject_uri(&triple.subject);
            let is_blank = matches!(triple.subject, Subject::BlankNode(_));

            let data = subjects
                .entry(subject_uri.clone())
                .or_insert_with(|| SubjectData {
                    uri: subject_uri.clone(),
                    is_blank,
                    labels: Vec::new(),
                    properties: Vec::new(),
                    relationships: Vec::new(),
                });

            // Categorize the triple
            if triple.predicate.as_str() == rdf_predicates::RDF_TYPE {
                // rdf:type -> add label
                if let Object::Iri(type_iri) = &triple.object {
                    let label = sanitize_identifier(type_iri.local_name());
                    if !data.labels.contains(&label) {
                        data.labels.push(label);
                    }
                }
            } else {
                match &triple.object {
                    Object::Literal(lit) => {
                        // Literal -> add property
                        let prop_name = sanitize_identifier(triple.predicate.local_name());
                        let is_numeric_or_bool = lit.as_integer().is_some()
                            || lit.as_float().is_some()
                            || lit.as_bool().is_some();
                        let value_repr = if is_numeric_or_bool {
                            lit.value().to_string()
                        } else {
                            format!("'{}'", escape_cypher_string(lit.value()))
                        };
                        data.properties.push((prop_name, value_repr));
                    }
                    Object::Iri(iri) => {
                        // IRI -> add relationship
                        data.relationships.push(RelationshipData {
                            predicate: triple.predicate.as_str().to_string(),
                            local_name: triple.predicate.local_name().to_string(),
                            target_uri: iri.as_str().to_string(),
                            target_is_blank: false,
                        });
                    }
                    Object::BlankNode(bn) => {
                        // Blank node -> add relationship
                        data.relationships.push(RelationshipData {
                            predicate: triple.predicate.as_str().to_string(),
                            local_name: triple.predicate.local_name().to_string(),
                            target_uri: format!("_:{}", bn.label()),
                            target_is_blank: true,
                        });
                    }
                }
            }
        }

        // Generate one query per subject
        let mut statements = Vec::new();

        for data in subjects.values() {
            let query = self.generate_subject_query(data);
            statements.push(query);
        }

        Ok(statements)
    }

    /// Generate a combined Cypher query for a subject with all its data
    fn generate_subject_query(&self, data: &SubjectData) -> String {
        let mut parts = Vec::new();

        // Determine the primary node label
        let (node_label, additional_labels) = if data.is_blank && !data.labels.is_empty() {
            // For blank nodes with types, use the first type as the primary label
            // and the rest as additional labels
            let primary = &data.labels[0];
            let additional = &data.labels[1..];
            (primary.as_str(), additional)
        } else if data.is_blank {
            // Blank node with no type - use BlankNode
            ("BlankNode", &data.labels[..])
        } else {
            // Resource node - use Resource as primary, all types as additional
            ("Resource", &data.labels[..])
        };

        // Build the SET clause with labels and properties
        let mut set_parts = Vec::new();

        // Add additional type labels (for resources, this is all labels; for blank nodes, this is labels after the first)
        for label in additional_labels {
            set_parts.push(format!("s:{}", label));
        }

        // Add isBlank property
        set_parts.push(format!("s.isBlank = {}", data.is_blank));

        // Add literal properties
        for (prop_name, value_repr) in &data.properties {
            set_parts.push(format!("s.{} = {}", prop_name, value_repr));
        }

        // Build the main MERGE + SET statement
        let main_stmt = format!(
            "{} (s:{} {{uri: '{}'}}) SET {}",
            self.operation(),
            node_label,
            escape_cypher_string(&data.uri),
            set_parts.join(", ")
        );
        parts.push(main_stmt);

        // Add relationships
        for (idx, rel) in data.relationships.iter().enumerate() {
            let target_var = format!("o{}", idx);
            let target_label = if rel.target_is_blank {
                "BlankNode"
            } else {
                "Resource"
            };
            let edge_type = sanitize_identifier(&rel.local_name);

            // MERGE target node
            parts.push(format!(
                "{} ({}:{} {{uri: '{}'}}) SET {}.isBlank = {}",
                self.operation(),
                target_var,
                target_label,
                escape_cypher_string(&rel.target_uri),
                target_var,
                rel.target_is_blank
            ));

            // MERGE relationship
            parts.push(format!(
                "{} (s)-[:{}{{predicate: '{}'}}]->({})",
                self.operation(),
                edge_type,
                escape_cypher_string(&rel.predicate),
                target_var
            ));
        }

        parts.join("\n")
    }
}

/// Helper struct to collect data about a subject
#[derive(Debug)]
struct SubjectData {
    uri: String,
    is_blank: bool,
    labels: Vec<String>,
    properties: Vec<(String, String)>, // (property_name, value_repr)
    relationships: Vec<RelationshipData>,
}

/// Helper struct for relationship data
#[derive(Debug)]
struct RelationshipData {
    predicate: String,
    local_name: String,
    target_uri: String,
    target_is_blank: bool,
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
                // Store literal as a property on the subject node
                let prop_key = sanitize_identifier(triple.predicate.local_name());
                let prop_value = PropertyValue::new(
                    lit.value().to_string(),
                    lit.datatype().as_str().to_string(),
                    lit.language().map(|s| s.to_string()),
                );
                if let Some(node) = self.resources.get_mut(&subject_uri) {
                    node.add_property(prop_key, prop_value);
                }
                // Also keep the literal in the literals collection for backwards compatibility
                self.literals.push(LiteralNode::new(
                    lit.value().to_string(),
                    lit.datatype().as_str().to_string(),
                    lit.language().map(|s| s.to_string()),
                ));
                // Return early - no edge needed since it's stored as a property
                return;
            }
        };

        // Add edge (only for non-literal objects)
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
        // Literal is now stored as a property, not as a separate Literal node
        assert!(stmt.contains("SET s.name = 'Alice'"));
        // Subject MERGE only, literal is now a SET property
        assert_eq!(stmt.matches("MERGE").count(), 1);
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
    fn test_blank_node_with_rdf_type() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();
        let blank_node = BlankNode::new("b1");
        let triple = Triple {
            subject: Subject::BlankNode(blank_node),
            predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            object: Object::Iri(test_iri("http://www.w3.org/2006/vcard/ns#Address")),
        };

        let statements = gen.generate_triple(&triple).unwrap();
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];

        // The blank node should be created with Address as the primary label
        // not BlankNode, to match the semantic meaning
        assert!(
            stmt.contains(":Address"),
            "Statement should contain :Address label. Statement: {}",
            stmt
        );

        // The MERGE should use Address label, not BlankNode
        // This is the key fix: blank nodes with a type should use that type as the primary label
        assert!(
            stmt.contains("MERGE (n:Address"),
            "Statement should MERGE with Address label, not BlankNode. Statement: {}",
            stmt
        );

        // Blank nodes should still be identifiable as blank via the isBlank property
        assert!(
            stmt.contains("isBlank = true"),
            "Statement should mark node as blank. Statement: {}",
            stmt
        );
    }

    #[test]
    fn test_blank_node_batch_with_type() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();
        let blank_node = BlankNode::new("b1");

        // Create multiple triples for the same blank node
        let triples = vec![
            // Type triple
            Triple {
                subject: Subject::BlankNode(blank_node.clone()),
                predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                object: Object::Iri(test_iri("http://www.w3.org/2006/vcard/ns#Address")),
            },
            // Property triple
            Triple {
                subject: Subject::BlankNode(blank_node.clone()),
                predicate: test_iri("http://www.w3.org/2006/vcard/ns#street-address"),
                object: Object::Literal(Literal::new("123 Main St")),
            },
        ];

        let statements = gen.generate_batch(&triples).unwrap();
        assert_eq!(statements.len(), 1); // Should combine into one statement

        let stmt = &statements[0];
        println!("Batch statement: {}", stmt);

        // The blank node should use Address as the primary label
        assert!(
            stmt.contains("MERGE (s:Address"),
            "Batch statement should use Address as primary label. Statement: {}",
            stmt
        );

        // Should have the property (sanitized to use underscore)
        assert!(
            stmt.contains("street_address") || stmt.contains("street-address"),
            "Should contain property"
        );
        assert!(
            stmt.contains("123 Main St"),
            "Should contain property value"
        );

        // Should mark as blank
        assert!(stmt.contains("isBlank = true"), "Should mark as blank");
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
