//! Graph Schema Definitions
//!
//! Defines the node and edge schemas for mapping RDF to FalkorDB graphs.

use serde::{Deserialize, Serialize};

/// Node types in the graph
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    /// A resource node (represents an IRI or blank node subject/object)
    Resource,
    /// A literal value node
    Literal,
}

/// A property value that can be stored on a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyValue {
    /// The property value
    pub value: String,
    /// The datatype IRI
    pub datatype: String,
    /// The language tag (if any)
    pub language: Option<String>,
}

impl PropertyValue {
    /// Create a new property value
    pub fn new(value: String, datatype: String, language: Option<String>) -> Self {
        Self {
            value,
            datatype,
            language,
        }
    }
}

/// Schema for a resource node (URI or blank node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceNode {
    /// The node's unique identifier (IRI string or blank node label)
    pub uri: String,
    /// Whether this is a blank node
    pub is_blank: bool,
    /// Labels assigned to this node (from rdf:type)
    pub labels: Vec<String>,
    /// Properties on this node (literal values keyed by predicate local name)
    pub properties: std::collections::HashMap<String, Vec<PropertyValue>>,
}

impl ResourceNode {
    /// Create a new resource node from an IRI
    pub fn from_iri(uri: String) -> Self {
        Self {
            uri,
            is_blank: false,
            labels: Vec::new(),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Create a new blank node
    pub fn blank(label: String) -> Self {
        Self {
            uri: label,
            is_blank: true,
            labels: Vec::new(),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Add a label to this node
    pub fn add_label(&mut self, label: String) {
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }

    /// Add a property to this node
    pub fn add_property(&mut self, key: String, value: PropertyValue) {
        self.properties.entry(key).or_default().push(value);
    }

    /// Get the primary label for Cypher (first label or "Resource")
    pub fn primary_label(&self) -> &str {
        self.labels
            .first()
            .map(|s| s.as_str())
            .unwrap_or("Resource")
    }
}

/// Schema for a literal value node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteralNode {
    /// The literal value
    pub value: String,
    /// The datatype IRI
    pub datatype: String,
    /// The language tag (if any)
    pub language: Option<String>,
}

impl LiteralNode {
    /// Create a new literal node
    pub fn new(value: String, datatype: String, language: Option<String>) -> Self {
        Self {
            value,
            datatype,
            language,
        }
    }
}

/// Schema for an edge (predicate)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The predicate IRI
    pub predicate: String,
    /// The predicate local name (for edge type)
    pub local_name: String,
    /// The graph this edge belongs to (for named graphs)
    pub graph: Option<String>,
}

impl Edge {
    /// Create a new edge
    pub fn new(predicate: String, local_name: String) -> Self {
        Self {
            predicate,
            local_name,
            graph: None,
        }
    }

    /// Create a new edge in a named graph
    pub fn in_graph(predicate: String, local_name: String, graph: String) -> Self {
        Self {
            predicate,
            local_name,
            graph: Some(graph),
        }
    }

    /// Get the edge type for Cypher (local name or sanitized predicate)
    pub fn edge_type(&self) -> String {
        sanitize_identifier(&self.local_name)
    }
}

/// A complete graph element (for building the graph)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphElement {
    /// A resource node
    Resource(ResourceNode),
    /// A literal node
    Literal(LiteralNode),
    /// An edge between two nodes
    Edge {
        /// Source node URI
        from: String,
        /// Target node URI (or literal identifier)
        to: String,
        /// Edge data
        edge: Edge,
        /// Whether the target is a literal
        to_is_literal: bool,
    },
}

/// Well-known RDF predicates
pub mod rdf_predicates {
    /// rdf:type predicate
    pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    /// rdf:first predicate (used in RDF collections)
    pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
    /// rdf:rest predicate (used in RDF collections)
    pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
    /// rdf:nil IRI (used to terminate RDF collections)
    pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
}

/// Escape a string for safe use in Cypher queries.
///
/// This function escapes all characters that could be used for Cypher injection
/// attacks or cause parsing errors.
///
/// # Escaped characters:
/// - Backslash (`\`) → `\\`
/// - Single quote (`'`) → `\'`
/// - Double quote (`"`) → `\"`
/// - Newline → `\n`
/// - Carriage return → `\r`
/// - Tab → `\t`
/// - Null byte → (removed)
/// - Other control characters (0x00-0x1F, 0x7F) → removed
///
/// # Example
/// ```
/// use falkorsemantic_mapper::graph::escape_cypher_string;
/// assert_eq!(escape_cypher_string("it's"), "it\\'s");
/// assert_eq!(escape_cypher_string("line\nbreak"), "line\\nbreak");
/// ```
pub fn escape_cypher_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '\'' => result.push_str("\\'"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // Remove null bytes and other control characters (security)
            '\0' => {}
            c if c.is_control() => {}
            c => result.push(c),
        }
    }
    result
}

/// Escape a string and wrap it in backticks for use as a Cypher identifier (label, type).
///
/// This is safer than using raw identifiers for user-provided data.
///
/// # Example
/// ```
/// use falkorsemantic_mapper::graph::escape_cypher_identifier;
/// assert_eq!(escape_cypher_identifier("My Label"), "`My Label`");
/// ```
pub fn escape_cypher_identifier(s: &str) -> String {
    // Backticks in identifiers are escaped by doubling them
    let escaped = s.replace('`', "``");
    format!("`{}`", escaped)
}

/// Sanitize a string for use as a Cypher identifier
///
/// Replaces invalid characters with underscores and ensures it starts with a letter.
pub fn sanitize_identifier(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    // Ensure first character is a letter
    if let Some(&first) = chars.peek() {
        if !first.is_ascii_alphabetic() && first != '_' {
            result.push('_');
        }
    }

    for ch in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }

    if result.is_empty() {
        result.push_str("_unnamed");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_node() {
        let mut node = ResourceNode::from_iri("http://example.org/Person/1".to_string());
        assert!(!node.is_blank);
        assert_eq!(node.primary_label(), "Resource");

        node.add_label("Person".to_string());
        assert_eq!(node.primary_label(), "Person");
    }

    #[test]
    fn test_blank_node() {
        let node = ResourceNode::blank("b0".to_string());
        assert!(node.is_blank);
    }

    #[test]
    fn test_edge() {
        let edge = Edge::new("http://example.org/knows".to_string(), "knows".to_string());
        assert_eq!(edge.edge_type(), "knows");
    }

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("Person"), "Person");
        assert_eq!(sanitize_identifier("has-name"), "has_name");
        assert_eq!(sanitize_identifier("123abc"), "_123abc");
        assert_eq!(sanitize_identifier("hello world"), "hello_world");
    }

    #[test]
    fn test_escape_cypher_string_basic() {
        assert_eq!(escape_cypher_string("hello"), "hello");
        assert_eq!(escape_cypher_string("it's"), "it\\'s");
        assert_eq!(escape_cypher_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_escape_cypher_string_whitespace() {
        assert_eq!(escape_cypher_string("line\nbreak"), "line\\nbreak");
        assert_eq!(escape_cypher_string("with\ttab"), "with\\ttab");
        assert_eq!(escape_cypher_string("with\rreturn"), "with\\rreturn");
    }

    #[test]
    fn test_escape_cypher_string_backslash() {
        assert_eq!(escape_cypher_string("path\\to\\file"), "path\\\\to\\\\file");
        assert_eq!(escape_cypher_string("\\' injection"), "\\\\\\' injection");
    }

    #[test]
    fn test_escape_cypher_string_null_byte() {
        assert_eq!(escape_cypher_string("before\0after"), "beforeafter");
    }

    #[test]
    fn test_escape_cypher_string_control_chars() {
        // Control characters should be removed
        assert_eq!(escape_cypher_string("test\x01\x02\x03"), "test");
        assert_eq!(escape_cypher_string("\x7fDEL"), "DEL");
    }

    #[test]
    fn test_escape_cypher_string_unicode() {
        // Unicode should be preserved
        assert_eq!(escape_cypher_string("日本語"), "日本語");
        assert_eq!(escape_cypher_string("emoji 🎉"), "emoji 🎉");
    }

    #[test]
    fn test_escape_cypher_identifier() {
        assert_eq!(escape_cypher_identifier("Person"), "`Person`");
        assert_eq!(escape_cypher_identifier("My Label"), "`My Label`");
        assert_eq!(escape_cypher_identifier("has`tick"), "`has``tick`");
    }
}
