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

/// Schema for a resource node (URI or blank node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceNode {
    /// The node's unique identifier (IRI string or blank node label)
    pub uri: String,
    /// Whether this is a blank node
    pub is_blank: bool,
    /// Labels assigned to this node (from rdf:type)
    pub labels: Vec<String>,
}

impl ResourceNode {
    /// Create a new resource node from an IRI
    pub fn from_iri(uri: String) -> Self {
        Self {
            uri,
            is_blank: false,
            labels: Vec::new(),
        }
    }

    /// Create a new blank node
    pub fn blank(label: String) -> Self {
        Self {
            uri: label,
            is_blank: true,
            labels: Vec::new(),
        }
    }

    /// Add a label to this node
    pub fn add_label(&mut self, label: String) {
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }

    /// Get the primary label for Cypher (first label or "Resource")
    pub fn primary_label(&self) -> &str {
        self.labels.first().map(|s| s.as_str()).unwrap_or("Resource")
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
        let edge = Edge::new(
            "http://example.org/knows".to_string(),
            "knows".to_string(),
        );
        assert_eq!(edge.edge_type(), "knows");
    }

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("Person"), "Person");
        assert_eq!(sanitize_identifier("has-name"), "has_name");
        assert_eq!(sanitize_identifier("123abc"), "_123abc");
        assert_eq!(sanitize_identifier("hello world"), "hello_world");
    }
}
