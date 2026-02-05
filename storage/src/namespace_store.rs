//! Namespace Storage
//!
//! Persists namespace prefix registries.

use std::collections::HashMap;

use falkorsemantic_parser::rdf::NamespaceRegistry;
use serde::{Deserialize, Serialize};

use crate::StorageError;

/// Serializable namespace mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceMapping {
    /// Prefix to namespace mappings
    pub prefixes: HashMap<String, String>,
}

impl NamespaceMapping {
    /// Create from a `NamespaceRegistry`
    #[must_use] 
    pub fn from_registry(registry: &NamespaceRegistry) -> Self {
        let prefixes = registry
            .prefixes()
            .map(|(p, n)| (p.to_string(), n.to_string()))
            .collect();
        Self { prefixes }
    }

    /// Convert to a `NamespaceRegistry`
    #[must_use] 
    pub fn to_registry(&self) -> NamespaceRegistry {
        let mut registry = NamespaceRegistry::new();
        for (prefix, namespace) in &self.prefixes {
            registry.add(prefix.clone(), namespace.clone());
        }
        registry
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, StorageError> {
        serde_json::to_string(self).map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self, StorageError> {
        serde_json::from_str(json).map_err(|e| StorageError::SerializationError(e.to_string()))
    }
}

/// In-memory namespace storage per graph
#[derive(Debug, Default)]
pub struct NamespaceStore {
    /// Graph name to namespace mappings
    graphs: HashMap<String, NamespaceMapping>,
}

impl NamespaceStore {
    /// Create a new namespace store
    #[must_use] 
    pub fn new() -> Self {
        Self {
            graphs: HashMap::new(),
        }
    }

    /// Store namespaces for a graph
    pub fn set(&mut self, graph_name: &str, registry: &NamespaceRegistry) {
        let mapping = NamespaceMapping::from_registry(registry);
        self.graphs.insert(graph_name.to_string(), mapping);
    }

    /// Get namespaces for a graph
    #[must_use] 
    pub fn get(&self, graph_name: &str) -> Option<NamespaceRegistry> {
        self.graphs.get(graph_name).map(NamespaceMapping::to_registry)
    }

    /// Remove namespaces for a graph
    pub fn remove(&mut self, graph_name: &str) -> Option<NamespaceRegistry> {
        self.graphs.remove(graph_name).map(|m| m.to_registry())
    }

    /// Check if a graph has stored namespaces
    #[must_use] 
    pub fn contains(&self, graph_name: &str) -> bool {
        self.graphs.contains_key(graph_name)
    }

    /// Get all graph names
    pub fn graph_names(&self) -> impl Iterator<Item = &str> {
        self.graphs.keys().map(std::string::String::as_str)
    }

    /// Clear all stored namespaces
    pub fn clear(&mut self) {
        self.graphs.clear();
    }

    /// Export all namespaces as JSON
    pub fn export(&self) -> Result<String, StorageError> {
        serde_json::to_string(&self.graphs)
            .map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    /// Import namespaces from JSON
    pub fn import(&mut self, json: &str) -> Result<(), StorageError> {
        let graphs: HashMap<String, NamespaceMapping> = serde_json::from_str(json)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;
        self.graphs = graphs;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_mapping_roundtrip() {
        let mut registry = NamespaceRegistry::new();
        registry.add("ex", "http://example.org/");
        registry.add("test", "http://test.org/");

        let mapping = NamespaceMapping::from_registry(&registry);
        let restored = mapping.to_registry();

        assert_eq!(restored.get_namespace("ex"), Some("http://example.org/"));
        assert_eq!(restored.get_namespace("test"), Some("http://test.org/"));
    }

    #[test]
    fn test_namespace_store() {
        let mut store = NamespaceStore::new();

        let mut registry = NamespaceRegistry::new();
        registry.add("ex", "http://example.org/");

        store.set("graph1", &registry);

        let retrieved = store.get("graph1");
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().get_namespace("ex"),
            Some("http://example.org/")
        );
    }

    #[test]
    fn test_namespace_json_serialization() {
        let mut registry = NamespaceRegistry::new();
        registry.add("ex", "http://example.org/");

        let mapping = NamespaceMapping::from_registry(&registry);
        let json = mapping.to_json().unwrap();
        let restored = NamespaceMapping::from_json(&json).unwrap();

        assert_eq!(
            restored.to_registry().get_namespace("ex"),
            Some("http://example.org/")
        );
    }
}
