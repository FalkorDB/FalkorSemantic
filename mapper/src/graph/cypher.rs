//! Cypher Query Generator
//!
//! Generates Cypher statements for inserting RDF data into `FalkorDB`.

use falkorsemantic_parser::rdf::{Literal, Object, Quad, Subject, Triple};

use super::schema::{
    escape_cypher_string, rdf_predicates, sanitize_identifier, Edge, LiteralNode, PropertyValue,
    ResourceNode,
};
use crate::MapperError;

/// Type alias for a detected RDF collection array
/// Format: (subject_uri, property_name, values)
type CollectionArray = (String, String, Vec<String>);

/// Type alias for collection detection result
type CollectionDetectionResult = (Vec<CollectionArray>, Vec<Triple>);

/// XSD date datatype IRI
const XSD_DATE: &str = "http://www.w3.org/2001/XMLSchema#date";

/// Generates Cypher statements for RDF triples
#[derive(Debug, Default)]
pub struct CypherGenerator {
    /// Whether to use MERGE instead of CREATE for idempotent operations
    use_merge: bool,
}

impl CypherGenerator {
    /// Create a new Cypher generator
    #[must_use]
    pub const fn new() -> Self {
        Self { use_merge: true }
    }

    /// Create a generator that uses CREATE instead of MERGE
    #[must_use]
    pub const fn with_create() -> Self {
        Self { use_merge: false }
    }

    /// Set whether to use MERGE (true) or CREATE (false)
    pub fn set_use_merge(&mut self, use_merge: bool) {
        self.use_merge = use_merge;
    }

    const fn operation(&self) -> &str {
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

                // Use consistent base label for MERGE to prevent duplicate nodes
                // For blank nodes, always use BlankNode as primary to ensure same URI matches same node
                // For resources, use Resource as primary
                let node_label = if is_blank { "BlankNode" } else { "Resource" };

                // Add the type as an additional label via SET
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
                .map(|s| format!("// Graph: {graph_uri}\n{s}"))
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

        // Check if this is a date type
        let is_date = if let Some(datatype) = literal.explicit_datatype() {
            datatype.as_str() == XSD_DATE && literal.as_date().is_some()
        } else {
            false
        };

        // Check if explicit xsd:date datatype was declared (even if invalid)
        let has_date_datatype = literal
            .explicit_datatype()
            .is_some_and(|dt| dt.as_str() == XSD_DATE);

        // For numeric and boolean types, use unquoted values; otherwise quote as string
        // Skip numeric check if xsd:date datatype was declared to maintain type contract
        let is_numeric_or_bool = !has_date_datatype
            && (literal.as_integer().is_some()
                || literal.as_float().is_some()
                || literal.as_bool().is_some());

        let value_repr = if is_date {
            // Use FalkorDB's date() function for xsd:date types
            format!("date('{}')", escape_cypher_string(literal.value()))
        } else if is_numeric_or_bool {
            literal.value().to_string()
        } else {
            format!("'{}'", escape_cypher_string(literal.value()))
        };

        // Store literal as a property on the subject node instead of creating a separate node
        format!("SET {subject_var}.{prop_name} = {value_repr}")
    }

    /// Detect and extract RDF collections that contain only simple values.
    ///
    /// Returns (collection_arrays, remaining_triples) where:
    /// - collection_arrays: Vec<(subject_uri, property_name, values)>
    /// - remaining_triples: Vec<Triple> without collection-related triples
    ///
    /// RDF collections are represented as:
    /// - subject predicate _:head
    /// - _:head rdf:first value1
    /// - _:head rdf:rest _:node2
    /// - _:node2 rdf:first value2
    /// - _:node2 rdf:rest rdf:nil
    ///
    /// These are converted to array properties if all values are literals or IRIs.
    fn detect_and_extract_collections(
        &self,
        triples: &[Triple],
    ) -> Result<CollectionDetectionResult, MapperError> {
        use std::collections::{HashMap, HashSet};

        // Map blank nodes to their rdf:first and rdf:rest values
        let mut first_map: HashMap<String, &Object> = HashMap::new();
        let mut rest_map: HashMap<String, &Object> = HashMap::new();
        let mut collection_heads: HashMap<String, (String, String)> = HashMap::new(); // bn_id -> (subject_uri, sanitized_property_name)
        let mut used_blank_nodes: HashSet<String> = HashSet::new();

        // First pass: identify collection structure
        for triple in triples {
            let subject_uri = self.subject_uri(&triple.subject);

            if triple.predicate.as_str() == rdf_predicates::RDF_FIRST {
                // This is a rdf:first triple
                if let Subject::BlankNode(bn) = &triple.subject {
                    first_map.insert(format!("_:{}", bn.label()), &triple.object);
                }
            } else if triple.predicate.as_str() == rdf_predicates::RDF_REST {
                // This is a rdf:rest triple
                if let Subject::BlankNode(bn) = &triple.subject {
                    rest_map.insert(format!("_:{}", bn.label()), &triple.object);
                }
            } else if let Object::BlankNode(bn) = &triple.object {
                // This could be a collection head (subject predicate _:bn)
                let bn_id = format!("_:{}", bn.label());
                // Check if this blank node is part of a collection
                // (it will be the head if it has rdf:first)
                collection_heads.insert(
                    bn_id.clone(),
                    (
                        subject_uri,
                        sanitize_identifier(triple.predicate.local_name()),
                    ),
                );
            }
        }

        // Second pass: build collections
        // Filter collection_heads to only include entries that have rdf:first predicates
        let mut collections: Vec<(String, String, Vec<String>)> = Vec::new();

        for (head_bn_id, (subject_uri, prop_name)) in collection_heads
            .iter()
            .filter(|(bn_id, _)| first_map.contains_key(*bn_id))
        {
            // Try to build the collection
            let mut current_bn = head_bn_id.clone();
            let mut values = Vec::new();
            let mut collection_blank_nodes = HashSet::new();
            let mut is_simple_collection = true;

            loop {
                collection_blank_nodes.insert(current_bn.clone());

                // Get the rdf:first value
                if let Some(first_obj) = first_map.get(&current_bn) {
                    match first_obj {
                        Object::Literal(lit) => {
                            // Simple literal value
                            let is_numeric_or_bool = lit.as_integer().is_some()
                                || lit.as_float().is_some()
                                || lit.as_bool().is_some();
                            let value_repr = if is_numeric_or_bool {
                                lit.value().to_string()
                            } else {
                                format!("'{}'", escape_cypher_string(lit.value()))
                            };
                            values.push(value_repr);
                        }
                        Object::Iri(iri) => {
                            // Simple IRI value - store as string
                            values.push(format!("'{}'", escape_cypher_string(iri.as_str())));
                        }
                        Object::BlankNode(_) => {
                            // Nested blank node - not a simple collection
                            is_simple_collection = false;
                            break;
                        }
                    }
                } else {
                    // Missing rdf:first - malformed collection
                    is_simple_collection = false;
                    break;
                }

                // Get the rdf:rest value
                if let Some(rest_obj) = rest_map.get(&current_bn) {
                    match rest_obj {
                        Object::Iri(iri) if iri.as_str() == rdf_predicates::RDF_NIL => {
                            // End of collection
                            break;
                        }
                        Object::BlankNode(bn) => {
                            // Continue to next node
                            current_bn = format!("_:{}", bn.label());
                        }
                        _ => {
                            // Unexpected rdf:rest value
                            is_simple_collection = false;
                            break;
                        }
                    }
                } else {
                    // Missing rdf:rest - malformed collection
                    is_simple_collection = false;
                    break;
                }
            }

            if is_simple_collection && !values.is_empty() {
                collections.push((subject_uri.clone(), prop_name.clone(), values));
                // Mark all blank nodes as used
                for bn in collection_blank_nodes {
                    used_blank_nodes.insert(bn);
                }
            }
        }

        // Third pass: filter out collection-related triples
        let remaining_triples: Vec<Triple> = triples
            .iter()
            .filter(|triple| {
                // Remove triples that are part of detected collections
                if let Subject::BlankNode(bn) = &triple.subject {
                    let bn_id = format!("_:{}", bn.label());
                    if used_blank_nodes.contains(&bn_id) {
                        // This is a collection node, remove all its triples
                        return false;
                    }
                }

                // Remove triples that point to collection heads
                if let Object::BlankNode(bn) = &triple.object {
                    let bn_id = format!("_:{}", bn.label());
                    if used_blank_nodes.contains(&bn_id) {
                        // Check if this is a collection head reference
                        if let Some((coll_subj, _)) = collection_heads.get(&bn_id) {
                            let subject_uri = self.subject_uri(&triple.subject);
                            if coll_subj == &subject_uri {
                                // This is the triple that points to the collection head
                                return false;
                            }
                        }
                    }
                }

                true
            })
            .cloned()
            .collect();

        Ok((collections, remaining_triples))
    }

    /// Generate a batch of statements for multiple triples
    ///
    /// This method optimizes by grouping triples by subject and generating
    /// a single combined query per subject, reducing the number of queries.
    pub fn generate_batch(&self, triples: &[Triple]) -> Result<Vec<String>, MapperError> {
        use std::collections::HashMap;

        // First pass: detect and process RDF collections
        let (collection_arrays, remaining_triples) =
            self.detect_and_extract_collections(triples)?;

        // Group triples by subject URI
        let mut subjects: HashMap<String, SubjectData> = HashMap::new();

        for triple in remaining_triples.iter() {
            let subject_uri = self.subject_uri(&triple.subject);
            let is_blank = matches!(triple.subject, Subject::BlankNode(_));

            let data = subjects
                .entry(subject_uri.clone())
                .or_insert_with(|| SubjectData {
                    uri: subject_uri.clone(),
                    is_blank,
                    labels: Vec::new(),
                    properties: Vec::new(),
                    array_properties: Vec::new(),
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

                        // Check if this is a date type
                        let is_date = if let Some(datatype) = lit.explicit_datatype() {
                            datatype.as_str() == XSD_DATE && lit.as_date().is_some()
                        } else {
                            false
                        };

                        // Check if explicit xsd:date datatype was declared (even if invalid)
                        let has_date_datatype = lit
                            .explicit_datatype()
                            .is_some_and(|dt| dt.as_str() == XSD_DATE);

                        // Skip numeric check if xsd:date datatype was declared
                        let is_numeric_or_bool = !has_date_datatype
                            && (lit.as_integer().is_some()
                                || lit.as_float().is_some()
                                || lit.as_bool().is_some());
                        let value_repr = if is_date {
                            // Use FalkorDB's date() function for xsd:date types
                            format!("date('{}')", escape_cypher_string(lit.value()))
                        } else if is_numeric_or_bool {
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

        // Add detected array properties to subjects
        for (subject_uri, prop_name, values) in collection_arrays {
            let is_blank = subject_uri.starts_with("_:");
            let data = subjects
                .entry(subject_uri.clone())
                .or_insert_with(|| SubjectData {
                    uri: subject_uri.clone(),
                    is_blank,
                    labels: Vec::new(),
                    properties: Vec::new(),
                    array_properties: Vec::new(),
                    relationships: Vec::new(),
                });
            data.array_properties.push((prop_name, values));
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
        // Use consistent base label for MERGE to prevent duplicate nodes when types are in different batches
        // For blank nodes: always use BlankNode as primary
        // For resources: always use Resource as primary
        let node_label = if data.is_blank {
            "BlankNode"
        } else {
            "Resource"
        };

        // Build the SET clause with labels and properties
        let mut set_parts = Vec::new();

        // Add all type labels (sorted for deterministic output)
        let mut sorted_labels = data.labels.clone();
        sorted_labels.sort();
        for label in &sorted_labels {
            set_parts.push(format!("s:{label}"));
        }

        // Add isBlank property
        set_parts.push(format!("s.isBlank = {}", data.is_blank));

        // Add literal properties
        for (prop_name, value_repr) in &data.properties {
            set_parts.push(format!("s.{prop_name} = {value_repr}"));
        }

        // Add array properties
        for (prop_name, values) in &data.array_properties {
            let array_repr = format!("[{}]", values.join(", "));
            set_parts.push(format!("s.{} = {}", prop_name, array_repr));
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
            let target_var = format!("o{idx}");
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
    array_properties: Vec<(String, Vec<String>)>, // (property_name, [values])
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
    #[must_use]
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
                    lit.language().map(std::string::ToString::to_string),
                );
                if let Some(node) = self.resources.get_mut(&subject_uri) {
                    node.add_property(prop_key, prop_value);
                }
                // Also keep the literal in the literals collection for backwards compatibility
                self.literals.push(LiteralNode::new(
                    lit.value().to_string(),
                    lit.datatype().as_str().to_string(),
                    lit.language().map(std::string::ToString::to_string),
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

        // The blank node should use BlankNode as primary label for MERGE (to prevent duplicates)
        // and Address as an additional label via SET
        assert!(
            stmt.contains("MERGE (n:BlankNode"),
            "Statement should MERGE with BlankNode label to prevent duplicates. Statement: {}",
            stmt
        );

        assert!(
            stmt.contains("SET n:Address"),
            "Statement should SET Address as additional label. Statement: {}",
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

        // The blank node should use BlankNode as primary label for MERGE (to prevent duplicates)
        // and Address as an additional label via SET
        assert!(
            stmt.contains("MERGE (s:BlankNode"),
            "Batch statement should use BlankNode as primary label to prevent duplicates. Statement: {}",
            stmt
        );

        assert!(
            stmt.contains("s:Address"),
            "Batch statement should add Address as additional label. Statement: {}",
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
    fn test_blank_node_with_multiple_types_deterministic() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();
        let blank_node = BlankNode::new("b1");

        // Create triples with multiple types in different orders
        // The primary label should always be the alphabetically first one
        let triples = vec![
            // Type triple - ZebraType (should not be primary)
            Triple {
                subject: Subject::BlankNode(blank_node.clone()),
                predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                object: Object::Iri(test_iri("http://example.org/ZebraType")),
            },
            // Type triple - AppleType (should be primary - alphabetically first)
            Triple {
                subject: Subject::BlankNode(blank_node.clone()),
                predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                object: Object::Iri(test_iri("http://example.org/AppleType")),
            },
            // Type triple - MangoType (should be additional)
            Triple {
                subject: Subject::BlankNode(blank_node.clone()),
                predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                object: Object::Iri(test_iri("http://example.org/MangoType")),
            },
        ];

        let statements = gen.generate_batch(&triples).unwrap();
        assert_eq!(statements.len(), 1);

        let stmt = &statements[0];
        println!("Multiple types statement: {}", stmt);

        // BlankNode should be the primary label for MERGE (to prevent duplicates)
        assert!(
            stmt.contains("MERGE (s:BlankNode"),
            "Should use BlankNode as primary label to prevent duplicates. Statement: {}",
            stmt
        );

        // All types should be added as additional labels (sorted alphabetically)
        assert!(
            stmt.contains("s:AppleType"),
            "Should add AppleType as additional label"
        );
        assert!(
            stmt.contains("s:MangoType"),
            "Should add MangoType as additional label"
        );
        assert!(
            stmt.contains("s:ZebraType"),
            "Should add ZebraType as additional label"
        );
    }

    #[test]
    fn test_blank_node_multiple_batches_no_duplicates() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();
        let blank_node = BlankNode::new("b1");

        // Batch 1: Blank node with Address type
        let batch1 = vec![Triple {
            subject: Subject::BlankNode(blank_node.clone()),
            predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            object: Object::Iri(test_iri("http://www.w3.org/2006/vcard/ns#Address")),
        }];

        // Batch 2: Same blank node with Location type
        let batch2 = vec![Triple {
            subject: Subject::BlankNode(blank_node.clone()),
            predicate: test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            object: Object::Iri(test_iri("http://www.w3.org/2006/vcard/ns#Location")),
        }];

        let stmt1 = gen.generate_batch(&batch1).unwrap();
        let stmt2 = gen.generate_batch(&batch2).unwrap();

        println!("Batch 1 statement: {}", stmt1[0]);
        println!("Batch 2 statement: {}", stmt2[0]);

        // Both should use BlankNode as primary label, ensuring they MERGE to the same node
        assert!(
            stmt1[0].contains("MERGE (s:BlankNode {uri: '_:b1'})"),
            "Batch 1 should use BlankNode as primary label"
        );
        assert!(
            stmt2[0].contains("MERGE (s:BlankNode {uri: '_:b1'})"),
            "Batch 2 should use BlankNode as primary label"
        );

        // The labels should be added via SET
        assert!(
            stmt1[0].contains("s:Address"),
            "Batch 1 should add Address label"
        );
        assert!(
            stmt2[0].contains("s:Location"),
            "Batch 2 should add Location label"
        );
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

    #[test]
    fn test_simple_collection_with_literals() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();

        // Build a simple collection: alice favoriteCourses ("Databases" "AI" "DistributedSystems")
        // This generates:
        // alice favoriteCourses _:b1
        // _:b1 rdf:first "Databases"
        // _:b1 rdf:rest _:b2
        // _:b2 rdf:first "AI"
        // _:b2 rdf:rest _:b3
        // _:b3 rdf:first "DistributedSystems"
        // _:b3 rdf:rest rdf:nil

        let bn1 = BlankNode::new("b1");
        let bn2 = BlankNode::new("b2");
        let bn3 = BlankNode::new("b3");

        let triples = vec![
            Triple::new(
                test_iri("http://example.org/alice"),
                test_iri("http://example.org/favoriteCourses"),
                bn1.clone(),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                Literal::new("Databases"),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                bn2.clone(),
            ),
            Triple::new(
                bn2.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                Literal::new("AI"),
            ),
            Triple::new(
                bn2.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                bn3.clone(),
            ),
            Triple::new(
                bn3.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                Literal::new("DistributedSystems"),
            ),
            Triple::new(
                bn3.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
            ),
        ];

        let statements = gen.generate_batch(&triples).unwrap();

        // Should generate one statement for alice with an array property
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];

        // Check that it contains alice
        assert!(stmt.contains("alice"), "Statement should contain alice");

        // Check that it contains the favoriteCourses property
        assert!(
            stmt.contains("favoriteCourses"),
            "Statement should contain favoriteCourses"
        );

        // Check that it contains the expected course values in order,
        // without relying on an exact array string representation
        assert!(
            stmt.contains("Databases"),
            "Statement should contain value 'Databases': {}",
            stmt
        );
        assert!(
            stmt.contains("AI"),
            "Statement should contain value 'AI': {}",
            stmt
        );
        assert!(
            stmt.contains("DistributedSystems"),
            "Statement should contain value 'DistributedSystems': {}",
            stmt
        );

        let pos_db = stmt
            .find("Databases")
            .expect("Statement should contain value 'Databases'");
        let pos_ai = stmt
            .find("AI")
            .expect("Statement should contain value 'AI'");
        let pos_ds = stmt
            .find("DistributedSystems")
            .expect("Statement should contain value 'DistributedSystems'");
        assert!(
            pos_db < pos_ai && pos_ai < pos_ds,
            "Course values should appear in order: Databases, AI, DistributedSystems. Statement: {}",
            stmt
        );

        // Should NOT contain blank node relationships
        assert!(!stmt.contains("BlankNode"), "Should not create blank nodes");
        assert!(!stmt.contains("rdf:first"), "Should not have rdf:first");
        assert!(!stmt.contains("rdf:rest"), "Should not have rdf:rest");
    }

    #[test]
    fn test_simple_collection_with_iris() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();

        // Build a collection with IRIs: alice knows (bob charlie david)
        let bn1 = BlankNode::new("b1");
        let bn2 = BlankNode::new("b2");
        let bn3 = BlankNode::new("b3");

        let triples = vec![
            Triple::new(
                test_iri("http://example.org/alice"),
                test_iri("http://example.org/knows"),
                bn1.clone(),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                test_iri("http://example.org/bob"),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                bn2.clone(),
            ),
            Triple::new(
                bn2.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                test_iri("http://example.org/charlie"),
            ),
            Triple::new(
                bn2.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                bn3.clone(),
            ),
            Triple::new(
                bn3.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                test_iri("http://example.org/david"),
            ),
            Triple::new(
                bn3.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
            ),
        ];

        let statements = gen.generate_batch(&triples).unwrap();

        // Should generate one statement for alice with an array property
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];

        // Check that knows is a property
        assert!(
            stmt.contains("knows"),
            "Statement should contain knows property"
        );

        // Check that all expected IRIs appear, in order, within an array-like context
        assert!(
            stmt.contains("http://example.org/bob"),
            "Statement should contain bob IRI: {}",
            stmt
        );
        assert!(
            stmt.contains("http://example.org/charlie"),
            "Statement should contain charlie IRI: {}",
            stmt
        );
        assert!(
            stmt.contains("http://example.org/david"),
            "Statement should contain david IRI: {}",
            stmt
        );

        // Ensure the IRIs appear in the correct order
        let bob_pos = stmt
            .find("http://example.org/bob")
            .expect("bob IRI not found");
        let charlie_pos = stmt
            .find("http://example.org/charlie")
            .expect("charlie IRI not found");
        let david_pos = stmt
            .find("http://example.org/david")
            .expect("david IRI not found");
        assert!(
            bob_pos < charlie_pos && charlie_pos < david_pos,
            "IRIs should appear in order [bob, charlie, david]: {}",
            stmt
        );
    }

    #[test]
    fn test_empty_collection() {
        let gen = CypherGenerator::new();

        // Empty collection: alice favoriteCourses ()
        // This generates: alice favoriteCourses rdf:nil
        let triples = vec![Triple::new(
            test_iri("http://example.org/alice"),
            test_iri("http://example.org/favoriteCourses"),
            test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
        )];

        let statements = gen.generate_batch(&triples).unwrap();

        // Empty collections are handled as regular relationships (not arrays)
        // since they point directly to rdf:nil without any blank nodes
        // This creates alice node and rdf:nil node with a relationship
        assert!(
            statements.len() >= 1,
            "Should generate at least one statement"
        );

        // Check that alice is created
        let combined = statements.join("\n");
        assert!(
            combined.contains("alice") || combined.contains("example.org/alice"),
            "Should contain alice: {}",
            combined
        );
    }

    #[test]
    fn test_nested_collection_not_converted() {
        use falkorsemantic_parser::rdf::BlankNode;

        let gen = CypherGenerator::new();

        // Nested collection (not simple): alice data (_:nested)
        // _:b1 rdf:first _:nested (this is a blank node, not a literal/IRI)
        // _:b1 rdf:rest rdf:nil

        let bn1 = BlankNode::new("b1");
        let bn_nested = BlankNode::new("nested");

        let triples = vec![
            Triple::new(
                test_iri("http://example.org/alice"),
                test_iri("http://example.org/data"),
                bn1.clone(),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#first"),
                bn_nested.clone(),
            ),
            Triple::new(
                bn1.clone(),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest"),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil"),
            ),
        ];

        let statements = gen.generate_batch(&triples).unwrap();

        // Should NOT convert to array since it contains a blank node
        // Should create multiple statements for the collection structure
        assert!(
            statements.len() > 1,
            "Nested collections should not be converted to arrays"
        );

        // The output should contain blank node references
        let combined = statements.join("\n");
        assert!(
            combined.contains("BlankNode") || combined.contains("_:"),
            "Should preserve blank node structure for nested collections"
        );
    }

    #[test]
    fn test_cypher_date_literal() {
        use falkorsemantic_parser::rdf::xsd;

        let gen = CypherGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/university"),
            test_iri("http://example.org/established"),
            Literal::with_datatype("1995-10-01", xsd::date()),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];
        // Should use date() function for xsd:date type
        assert!(
            stmt.contains("SET s.established = date('1995-10-01')"),
            "Statement should contain date() function, got: {}",
            stmt
        );
    }

    #[test]
    fn test_cypher_integer_literal() {
        use falkorsemantic_parser::rdf::xsd;

        let gen = CypherGenerator::new();
        let triple = Triple::new(
            test_iri("http://example.org/university"),
            test_iri("http://example.org/ranking"),
            Literal::with_datatype("42", xsd::integer()),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];
        // Integer should be unquoted
        assert!(
            stmt.contains("SET s.ranking = 42"),
            "Statement should contain unquoted integer, got: {}",
            stmt
        );
    }

    #[test]
    fn test_cypher_batch_with_date() {
        use falkorsemantic_parser::rdf::xsd;

        let gen = CypherGenerator::new();

        // Example from the issue: university with establishment date
        let triples = vec![
            // ex:university a foaf:Organization
            Triple::new(
                test_iri("http://example.org/university"),
                test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                test_iri("http://xmlns.com/foaf/0.1/Organization"),
            ),
            // ex:university foaf:name "Tech University"
            Triple::new(
                test_iri("http://example.org/university"),
                test_iri("http://xmlns.com/foaf/0.1/name"),
                Literal::new("Tech University"),
            ),
            // ex:university ex:established "1995-10-01"^^xsd:date
            Triple::new(
                test_iri("http://example.org/university"),
                test_iri("http://example.org/established"),
                Literal::with_datatype("1995-10-01", xsd::date()),
            ),
            // ex:university ex:ranking "42"^^xsd:int
            Triple::new(
                test_iri("http://example.org/university"),
                test_iri("http://example.org/ranking"),
                Literal::with_datatype("42", xsd::integer()),
            ),
        ];

        let statements = gen.generate_batch(&triples).unwrap();
        assert_eq!(
            statements.len(),
            1,
            "Should generate one statement for the subject"
        );

        let stmt = &statements[0];

        // Verify it contains the Organization label
        assert!(
            stmt.contains(":Organization"),
            "Should have Organization label"
        );

        // Verify date is stored with date() function
        assert!(
            stmt.contains("date('1995-10-01')"),
            "Date should use date() function, got: {}",
            stmt
        );

        // Verify integer is unquoted - check for both parts separately
        assert!(
            stmt.contains(".ranking") && stmt.contains("42"),
            "Integer property should be present, got: {}",
            stmt
        );
        // Ensure 42 is not quoted
        assert!(
            !stmt.contains("'42'"),
            "Integer should not be quoted, got: {}",
            stmt
        );

        // Verify string is quoted
        assert!(
            stmt.contains("'Tech University'"),
            "String should be quoted, got: {}",
            stmt
        );
    }

    #[test]
    fn test_invalid_xsd_date_as_string() {
        use falkorsemantic_parser::rdf::xsd;

        let gen = CypherGenerator::new();

        // Test case: literal with xsd:date datatype but invalid date value
        // Should be treated as string, not as unquoted integer
        let triple = Triple::new(
            test_iri("http://example.org/entity"),
            test_iri("http://example.org/prop"),
            Literal::with_datatype("42", xsd::date()),
        );

        let statements = gen.generate_triple(&triple).unwrap();
        assert_eq!(statements.len(), 1);
        let stmt = &statements[0];

        // Should be quoted as string since xsd:date datatype contract must be maintained
        assert!(
            stmt.contains("SET s.prop = '42'"),
            "Invalid xsd:date should be quoted as string, got: {}",
            stmt
        );
        // Should NOT be unquoted integer
        assert!(
            !stmt.contains("SET s.prop = 42"),
            "Invalid xsd:date should not be treated as integer, got: {}",
            stmt
        );
    }
}
