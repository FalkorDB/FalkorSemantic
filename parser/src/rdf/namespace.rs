//! Namespace prefix management
//!
//! Provides functionality for managing RDF namespace prefixes,
//! including expansion and contraction of IRIs.

use std::collections::HashMap;
use std::fmt;

use super::Iri;
use crate::ParserError;

/// Well-known RDF namespace prefixes
pub mod well_known {
    /// RDF namespace
    pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    /// RDFS namespace
    pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
    /// XSD namespace
    pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
    /// OWL namespace
    pub const OWL: &str = "http://www.w3.org/2002/07/owl#";
    /// SKOS namespace
    pub const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
    /// DC namespace (Dublin Core elements)
    pub const DC: &str = "http://purl.org/dc/elements/1.1/";
    /// DCT namespace (Dublin Core terms)
    pub const DCT: &str = "http://purl.org/dc/terms/";
    /// FOAF namespace
    pub const FOAF: &str = "http://xmlns.com/foaf/0.1/";
}

/// A prefixed name (prefix:localName)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrefixedName {
    /// The prefix (without the colon)
    pub prefix: String,
    /// The local name
    pub local_name: String,
}

impl PrefixedName {
    /// Create a new prefixed name
    pub fn new(prefix: impl Into<String>, local_name: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            local_name: local_name.into(),
        }
    }

    /// Parse a prefixed name from a string (e.g., "rdf:type")
    pub fn parse(s: &str) -> Result<Self, ParserError> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(ParserError::InvalidInput(format!(
                "Invalid prefixed name: {s}"
            )));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

impl fmt::Display for PrefixedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.prefix, self.local_name)
    }
}

/// Namespace prefix registry
///
/// Maps prefixes to namespace IRIs and vice versa.
#[derive(Debug, Clone)]
pub struct NamespaceRegistry {
    /// Prefix to namespace mapping
    prefix_to_ns: HashMap<String, String>,
    /// Namespace to prefix mapping (for contraction)
    ns_to_prefix: HashMap<String, String>,
}

impl NamespaceRegistry {
    /// Create an empty namespace registry
    #[must_use] 
    pub fn new() -> Self {
        Self {
            prefix_to_ns: HashMap::new(),
            ns_to_prefix: HashMap::new(),
        }
    }

    /// Create a registry with standard prefixes (rdf, rdfs, xsd, owl)
    #[must_use] 
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.add("rdf", well_known::RDF);
        registry.add("rdfs", well_known::RDFS);
        registry.add("xsd", well_known::XSD);
        registry.add("owl", well_known::OWL);
        registry
    }

    /// Create a registry with extended defaults (includes common vocabularies)
    #[must_use] 
    pub fn with_extended_defaults() -> Self {
        let mut registry = Self::with_defaults();
        registry.add("skos", well_known::SKOS);
        registry.add("dc", well_known::DC);
        registry.add("dct", well_known::DCT);
        registry.add("foaf", well_known::FOAF);
        registry
    }

    /// Add a prefix-namespace mapping
    pub fn add(&mut self, prefix: impl Into<String>, namespace: impl Into<String>) {
        let prefix = prefix.into();
        let namespace = namespace.into();

        // Remove old namespace mapping if prefix is being reassigned
        if let Some(old_ns) = self.prefix_to_ns.get(&prefix) {
            self.ns_to_prefix.remove(old_ns);
        }

        self.ns_to_prefix.insert(namespace.clone(), prefix.clone());
        self.prefix_to_ns.insert(prefix, namespace);
    }

    /// Remove a prefix mapping
    pub fn remove(&mut self, prefix: &str) -> Option<String> {
        if let Some(ns) = self.prefix_to_ns.remove(prefix) {
            self.ns_to_prefix.remove(&ns);
            Some(ns)
        } else {
            None
        }
    }

    /// Get the namespace for a prefix
    #[must_use] 
    pub fn get_namespace(&self, prefix: &str) -> Option<&str> {
        self.prefix_to_ns.get(prefix).map(std::string::String::as_str)
    }

    /// Get the prefix for a namespace
    #[must_use] 
    pub fn get_prefix(&self, namespace: &str) -> Option<&str> {
        self.ns_to_prefix.get(namespace).map(std::string::String::as_str)
    }

    /// Check if a prefix is registered
    #[must_use] 
    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.prefix_to_ns.contains_key(prefix)
    }

    /// Expand a prefixed name to a full IRI
    pub fn expand(&self, prefixed: &PrefixedName) -> Result<Iri, ParserError> {
        let namespace = self.get_namespace(&prefixed.prefix).ok_or_else(|| {
            ParserError::InvalidInput(format!("Unknown prefix: {}", prefixed.prefix))
        })?;
        Iri::new(format!("{}{}", namespace, prefixed.local_name))
    }

    /// Expand a prefixed name string (e.g., "rdf:type") to a full IRI
    pub fn expand_str(&self, prefixed: &str) -> Result<Iri, ParserError> {
        let pname = PrefixedName::parse(prefixed)?;
        self.expand(&pname)
    }

    /// Contract an IRI to a prefixed name, if possible
    #[must_use] 
    pub fn contract(&self, iri: &Iri) -> Option<PrefixedName> {
        let iri_str = iri.as_str();

        // Try to find a matching namespace
        for (ns, prefix) in &self.ns_to_prefix {
            if iri_str.starts_with(ns) {
                let local_name = &iri_str[ns.len()..];
                // Only contract if the local name is valid (no slashes or hashes)
                if !local_name.contains('/') && !local_name.contains('#') {
                    return Some(PrefixedName::new(prefix.clone(), local_name));
                }
            }
        }
        None
    }

    /// Get all registered prefixes
    pub fn prefixes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.prefix_to_ns
            .iter()
            .map(|(p, n)| (p.as_str(), n.as_str()))
    }

    /// Get the number of registered prefixes
    #[must_use] 
    pub fn len(&self) -> usize {
        self.prefix_to_ns.len()
    }

    /// Check if the registry is empty
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.prefix_to_ns.is_empty()
    }

    /// Merge another registry into this one
    ///
    /// If there are conflicts, the other registry's mappings take precedence.
    pub fn merge(&mut self, other: &Self) {
        for (prefix, namespace) in &other.prefix_to_ns {
            self.add(prefix.clone(), namespace.clone());
        }
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl fmt::Display for NamespaceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (prefix, namespace) in &self.prefix_to_ns {
            writeln!(f, "@prefix {prefix}: <{namespace}> .")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefixed_name_parse() {
        let pn = PrefixedName::parse("rdf:type").unwrap();
        assert_eq!(pn.prefix, "rdf");
        assert_eq!(pn.local_name, "type");
        assert_eq!(format!("{}", pn), "rdf:type");
    }

    #[test]
    fn test_prefixed_name_empty_local() {
        let pn = PrefixedName::parse("ex:").unwrap();
        assert_eq!(pn.prefix, "ex");
        assert_eq!(pn.local_name, "");
    }

    #[test]
    fn test_registry_defaults() {
        let registry = NamespaceRegistry::with_defaults();
        assert_eq!(registry.get_namespace("rdf"), Some(well_known::RDF));
        assert_eq!(registry.get_namespace("rdfs"), Some(well_known::RDFS));
        assert_eq!(registry.get_namespace("xsd"), Some(well_known::XSD));
        assert_eq!(registry.get_namespace("owl"), Some(well_known::OWL));
    }

    #[test]
    fn test_registry_expand() {
        let registry = NamespaceRegistry::with_defaults();
        let iri = registry.expand_str("rdf:type").unwrap();
        assert_eq!(
            iri.as_str(),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
        );
    }

    #[test]
    fn test_registry_contract() {
        let registry = NamespaceRegistry::with_defaults();
        let iri = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").unwrap();
        let contracted = registry.contract(&iri);
        assert!(contracted.is_some());
        let pn = contracted.unwrap();
        assert_eq!(pn.prefix, "rdf");
        assert_eq!(pn.local_name, "type");
    }

    #[test]
    fn test_registry_add_remove() {
        let mut registry = NamespaceRegistry::new();
        registry.add("ex", "http://example.org/");

        assert_eq!(registry.get_namespace("ex"), Some("http://example.org/"));

        registry.remove("ex");
        assert_eq!(registry.get_namespace("ex"), None);
    }

    #[test]
    fn test_registry_unknown_prefix() {
        let registry = NamespaceRegistry::new();
        let result = registry.expand_str("unknown:something");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_merge() {
        let mut base = NamespaceRegistry::with_defaults();
        let mut extra = NamespaceRegistry::new();
        extra.add("ex", "http://example.org/");
        extra.add("test", "http://test.org/");

        base.merge(&extra);

        assert!(base.has_prefix("rdf")); // Original
        assert!(base.has_prefix("ex")); // Merged
        assert!(base.has_prefix("test")); // Merged
    }
}
