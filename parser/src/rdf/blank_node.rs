//! Blank Node implementation
//!
//! Blank nodes are anonymous resources in RDF graphs.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique blank node IDs
static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A blank node (anonymous resource)
///
/// Blank nodes are identified by a label that is unique within a scope.
/// They don't have a global identity like IRIs do.
#[derive(Debug, Clone, Eq)]
pub struct BlankNode {
    /// The blank node label (without the "_:" prefix)
    label: String,
}

impl BlankNode {
    /// Create a blank node with a specific label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Generate a new unique blank node
    pub fn generate() -> Self {
        let id = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self {
            label: format!("b{}", id),
        }
    }

    /// Get the blank node label (without "_:" prefix)
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl PartialEq for BlankNode {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Hash for BlankNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.label.hash(state);
    }
}

impl fmt::Display for BlankNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_:{}", self.label)
    }
}

/// A scoped blank node generator
///
/// Generates blank nodes that are unique within a specific scope (e.g., a document).
/// This ensures that blank node labels from different documents don't collide.
#[derive(Debug)]
pub struct BlankNodeScope {
    /// Prefix for this scope
    prefix: String,
    /// Counter for this scope
    counter: AtomicU64,
}

impl BlankNodeScope {
    /// Create a new blank node scope with a prefix
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            counter: AtomicU64::new(0),
        }
    }

    /// Create a scope with a generated unique prefix
    pub fn generate() -> Self {
        let scope_id = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self::new(format!("g{}", scope_id))
    }

    /// Generate a new blank node within this scope
    pub fn next(&self) -> BlankNode {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        BlankNode::new(format!("{}_{}", self.prefix, id))
    }

    /// Map an external blank node label to a scoped blank node
    ///
    /// This is useful when parsing RDF documents to ensure blank nodes
    /// from different documents don't collide.
    pub fn map(&self, external_label: &str) -> BlankNode {
        BlankNode::new(format!("{}_{}", self.prefix, external_label))
    }
}

impl Default for BlankNodeScope {
    fn default() -> Self {
        Self::generate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blank_node_creation() {
        let bn = BlankNode::new("node1");
        assert_eq!(bn.label(), "node1");
        assert_eq!(format!("{}", bn), "_:node1");
    }

    #[test]
    fn test_blank_node_generate() {
        let bn1 = BlankNode::generate();
        let bn2 = BlankNode::generate();
        assert_ne!(bn1.label(), bn2.label());
    }

    #[test]
    fn test_blank_node_equality() {
        let bn1 = BlankNode::new("same");
        let bn2 = BlankNode::new("same");
        let bn3 = BlankNode::new("different");

        assert_eq!(bn1, bn2);
        assert_ne!(bn1, bn3);
    }

    #[test]
    fn test_blank_node_scope() {
        let scope = BlankNodeScope::new("doc1");
        let bn1 = scope.next();
        let bn2 = scope.next();

        assert!(bn1.label().starts_with("doc1_"));
        assert!(bn2.label().starts_with("doc1_"));
        assert_ne!(bn1.label(), bn2.label());
    }

    #[test]
    fn test_blank_node_scope_map() {
        let scope = BlankNodeScope::new("doc1");
        let mapped = scope.map("external");

        assert_eq!(mapped.label(), "doc1_external");
    }

    #[test]
    fn test_scopes_dont_collide() {
        let scope1 = BlankNodeScope::new("a");
        let scope2 = BlankNodeScope::new("b");

        let bn1 = scope1.next();
        let bn2 = scope2.next();

        assert_ne!(bn1.label(), bn2.label());
    }
}
