//! Namespace Prefix Map
//!
//! Stores and manages namespace prefixes from SPARQL query prologues.

use std::collections::HashMap;

/// Map of namespace prefixes to IRIs
#[derive(Debug, Clone, Default)]
pub struct PrefixMap {
    prefixes: HashMap<String, String>,
    base: Option<String>,
}

impl PrefixMap {
    /// Create a new empty prefix map
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a prefix map with standard prefixes pre-populated
    pub fn with_common_prefixes() -> Self {
        let mut map = Self::new();
        map.add("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#");
        map.add("rdfs", "http://www.w3.org/2000/01/rdf-schema#");
        map.add("xsd", "http://www.w3.org/2001/XMLSchema#");
        map.add("owl", "http://www.w3.org/2002/07/owl#");
        map.add("foaf", "http://xmlns.com/foaf/0.1/");
        map.add("dc", "http://purl.org/dc/elements/1.1/");
        map.add("dcterms", "http://purl.org/dc/terms/");
        map.add("skos", "http://www.w3.org/2004/02/skos/core#");
        map
    }

    /// Set the base IRI
    pub fn set_base(&mut self, base: impl Into<String>) {
        self.base = Some(base.into());
    }

    /// Get the base IRI
    pub fn base(&self) -> Option<&str> {
        self.base.as_deref()
    }

    /// Add a prefix mapping
    pub fn add(&mut self, prefix: impl Into<String>, iri: impl Into<String>) {
        self.prefixes.insert(prefix.into(), iri.into());
    }

    /// Get the IRI for a prefix
    pub fn get(&self, prefix: &str) -> Option<&str> {
        self.prefixes.get(prefix).map(|s| s.as_str())
    }

    /// Check if a prefix exists
    pub fn contains(&self, prefix: &str) -> bool {
        self.prefixes.contains_key(prefix)
    }

    /// Expand a prefixed name to a full IRI
    ///
    /// Returns None if the prefix is not defined.
    pub fn expand(&self, prefixed_name: &str) -> Option<String> {
        if let Some((prefix, local)) = prefixed_name.split_once(':') {
            if let Some(ns) = self.prefixes.get(prefix) {
                return Some(format!("{}{}", ns, local));
            }
        }
        None
    }

    /// Compact a full IRI to a prefixed name if possible
    pub fn compact(&self, iri: &str) -> Option<String> {
        for (prefix, ns) in &self.prefixes {
            if let Some(local) = iri.strip_prefix(ns.as_str()) {
                // Validate that the local part is a valid local name
                if is_valid_local_name(local) {
                    return Some(format!("{}:{}", prefix, local));
                }
            }
        }
        None
    }

    /// Get all prefix mappings
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.prefixes.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of prefixes
    pub fn len(&self) -> usize {
        self.prefixes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    /// Merge another prefix map into this one
    ///
    /// Existing prefixes are not overwritten.
    pub fn merge(&mut self, other: &PrefixMap) {
        for (prefix, iri) in &other.prefixes {
            if !self.prefixes.contains_key(prefix) {
                self.prefixes.insert(prefix.clone(), iri.clone());
            }
        }
    }

    /// Extract prefixes from a spargebra Query
    pub fn from_spargebra_query(query: &spargebra::Query) -> Self {
        let map = Self::new();

        // spargebra doesn't directly expose prefixes in parsed form,
        // but we can extract them from serialization if needed
        // For now, this is a placeholder for future enhancement

        // Extract base IRI if present (would need query inspection)
        let _ = query; // Acknowledge parameter

        map
    }
}

/// Check if a string is a valid local name in a prefixed name
fn is_valid_local_name(s: &str) -> bool {
    if s.is_empty() {
        return true; // Empty local part is valid (e.g., "rdf:")
    }

    // Basic validation - local names shouldn't contain certain characters
    !s.contains('<')
        && !s.contains('>')
        && !s.contains('"')
        && !s.contains(' ')
        && !s.contains('{')
        && !s.contains('}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_prefix_map() {
        let map = PrefixMap::new();
        assert!(map.is_empty());
    }

    #[test]
    fn test_add_and_get() {
        let mut map = PrefixMap::new();
        map.add("ex", "http://example.org/");

        assert_eq!(map.get("ex"), Some("http://example.org/"));
        assert_eq!(map.get("unknown"), None);
    }

    #[test]
    fn test_expand() {
        let mut map = PrefixMap::new();
        map.add("foaf", "http://xmlns.com/foaf/0.1/");

        assert_eq!(
            map.expand("foaf:Person"),
            Some("http://xmlns.com/foaf/0.1/Person".to_string())
        );
        assert_eq!(map.expand("unknown:Thing"), None);
    }

    #[test]
    fn test_compact() {
        let mut map = PrefixMap::new();
        map.add("foaf", "http://xmlns.com/foaf/0.1/");

        assert_eq!(
            map.compact("http://xmlns.com/foaf/0.1/Person"),
            Some("foaf:Person".to_string())
        );
        assert_eq!(map.compact("http://unknown.org/Thing"), None);
    }

    #[test]
    fn test_common_prefixes() {
        let map = PrefixMap::with_common_prefixes();

        assert!(map.contains("rdf"));
        assert!(map.contains("rdfs"));
        assert!(map.contains("xsd"));
        assert!(map.contains("owl"));
    }

    #[test]
    fn test_base_iri() {
        let mut map = PrefixMap::new();
        assert_eq!(map.base(), None);

        map.set_base("http://example.org/");
        assert_eq!(map.base(), Some("http://example.org/"));
    }

    #[test]
    fn test_merge() {
        let mut map1 = PrefixMap::new();
        map1.add("ex", "http://example.org/");

        let mut map2 = PrefixMap::new();
        map2.add("foaf", "http://xmlns.com/foaf/0.1/");
        map2.add("ex", "http://other.org/"); // Should not overwrite

        map1.merge(&map2);

        assert_eq!(map1.get("ex"), Some("http://example.org/"));
        assert_eq!(map1.get("foaf"), Some("http://xmlns.com/foaf/0.1/"));
    }

    #[test]
    fn test_iter() {
        let mut map = PrefixMap::new();
        map.add("a", "http://a.org/");
        map.add("b", "http://b.org/");

        let entries: Vec<_> = map.iter().collect();
        assert_eq!(entries.len(), 2);
    }
}
