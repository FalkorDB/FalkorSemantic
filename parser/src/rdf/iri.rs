//! IRI (Internationalized Resource Identifier) implementation
//!
//! Provides parsing, validation, and handling of IRIs according to RFC 3987.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::ParserError;

/// An IRI (Internationalized Resource Identifier)
///
/// IRIs are used in RDF to identify resources. They are similar to URIs but
/// allow Unicode characters.
#[derive(Debug, Clone, Eq)]
pub struct Iri {
    /// The full IRI string
    value: String,
    /// Cached scheme end position (after "://")
    scheme_end: usize,
    /// Cached fragment start position (after "#"), if present
    fragment_start: Option<usize>,
}

impl Iri {
    /// Create a new IRI from a string, validating it
    pub fn new(value: impl Into<String>) -> Result<Self, ParserError> {
        let value = value.into();
        Self::validate(&value)?;

        let scheme_end = value
            .find("://")
            .map(|i| i + 3)
            .or_else(|| value.find(':').map(|i| i + 1))
            .unwrap_or(0);

        let fragment_start = value.find('#').map(|i| i + 1);

        Ok(Self {
            value,
            scheme_end,
            fragment_start,
        })
    }

    /// Create an IRI without validation (use with caution)
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        let value = value.into();
        let scheme_end = value
            .find("://")
            .map(|i| i + 3)
            .or_else(|| value.find(':').map(|i| i + 1))
            .unwrap_or(0);
        let fragment_start = value.find('#').map(|i| i + 1);

        Self {
            value,
            scheme_end,
            fragment_start,
        }
    }

    /// Validate an IRI string
    fn validate(value: &str) -> Result<(), ParserError> {
        if value.is_empty() {
            return Err(ParserError::InvalidInput("IRI cannot be empty".into()));
        }

        // Must contain a scheme (contains ':')
        if !value.contains(':') {
            return Err(ParserError::InvalidInput(
                "IRI must contain a scheme (no ':' found)".into(),
            ));
        }

        // Check for invalid characters (basic validation)
        // IRIs allow Unicode but disallow certain control characters and spaces
        for ch in value.chars() {
            if ch.is_control() || ch == ' ' || ch == '<' || ch == '>' {
                return Err(ParserError::InvalidInput(format!(
                    "IRI contains invalid character: {ch:?}"
                )));
            }
        }

        Ok(())
    }

    /// Get the full IRI as a string
    #[must_use] 
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Get the scheme part of the IRI (e.g., "http", "https", "urn")
    #[must_use] 
    pub fn scheme(&self) -> &str {
        let end = self.value.find(':').unwrap_or(self.scheme_end);
        &self.value[..end]
    }

    /// Get the fragment part of the IRI (after '#'), if present
    #[must_use] 
    pub fn fragment(&self) -> Option<&str> {
        self.fragment_start.map(|start| &self.value[start..])
    }

    /// Get the namespace part of the IRI (everything before the local name)
    ///
    /// The namespace is everything up to and including the last '#' or '/'
    #[must_use] 
    pub fn namespace(&self) -> &str {
        if let Some(pos) = self.value.rfind('#') {
            &self.value[..=pos]
        } else if let Some(pos) = self.value.rfind('/') {
            &self.value[..=pos]
        } else {
            &self.value
        }
    }

    /// Get the local name part of the IRI (after the last '#' or '/')
    #[must_use] 
    pub fn local_name(&self) -> &str {
        if let Some(pos) = self.value.rfind('#') {
            &self.value[pos + 1..]
        } else if let Some(pos) = self.value.rfind('/') {
            &self.value[pos + 1..]
        } else {
            ""
        }
    }

    /// Resolve a relative IRI against this base IRI
    pub fn resolve(&self, relative: &str) -> Result<Self, ParserError> {
        if relative.contains("://") {
            // Already absolute
            return Self::new(relative);
        }

        if relative.starts_with('#') {
            // Fragment-only reference
            let base = if let Some(pos) = self.value.find('#') {
                &self.value[..pos]
            } else {
                &self.value
            };
            return Self::new(format!("{base}{relative}"));
        }

        if relative.starts_with('/') {
            // Absolute path reference
            if let Some(authority_end) = self.value.find("://").map(|i| {
                self.value[i + 3..]
                    .find('/')
                    .map_or(self.value.len(), |j| i + 3 + j)
            }) {
                return Self::new(format!("{}{}", &self.value[..authority_end], relative));
            }
        }

        // Relative path reference
        let base = self.namespace();
        Self::new(format!("{base}{relative}"))
    }
}

impl PartialEq for Iri {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Hash for Iri {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl fmt::Display for Iri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.value)
    }
}

impl AsRef<str> for Iri {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iri_creation() {
        let iri = Iri::new("http://example.org/resource").unwrap();
        assert_eq!(iri.as_str(), "http://example.org/resource");
    }

    #[test]
    fn test_iri_scheme() {
        let iri = Iri::new("http://example.org/resource").unwrap();
        assert_eq!(iri.scheme(), "http");

        let urn = Iri::new("urn:isbn:0451450523").unwrap();
        assert_eq!(urn.scheme(), "urn");
    }

    #[test]
    fn test_iri_fragment() {
        let iri = Iri::new("http://example.org/ontology#Person").unwrap();
        assert_eq!(iri.fragment(), Some("Person"));

        let iri_no_frag = Iri::new("http://example.org/resource").unwrap();
        assert_eq!(iri_no_frag.fragment(), None);
    }

    #[test]
    fn test_iri_namespace_and_local_name() {
        let iri = Iri::new("http://example.org/ontology#Person").unwrap();
        assert_eq!(iri.namespace(), "http://example.org/ontology#");
        assert_eq!(iri.local_name(), "Person");

        let iri2 = Iri::new("http://example.org/resource/123").unwrap();
        assert_eq!(iri2.namespace(), "http://example.org/resource/");
        assert_eq!(iri2.local_name(), "123");
    }

    #[test]
    fn test_iri_validation() {
        assert!(Iri::new("").is_err());
        assert!(Iri::new("no-scheme").is_err());
        assert!(Iri::new("http://example.org/with space").is_err());
        assert!(Iri::new("http://example.org/valid").is_ok());
    }

    #[test]
    fn test_iri_resolve() {
        let base = Iri::new("http://example.org/base/page").unwrap();

        let resolved = base.resolve("#section").unwrap();
        assert_eq!(resolved.as_str(), "http://example.org/base/page#section");

        let resolved2 = base.resolve("other").unwrap();
        assert_eq!(resolved2.as_str(), "http://example.org/base/other");
    }

    #[test]
    fn test_iri_display() {
        let iri = Iri::new("http://example.org/resource").unwrap();
        assert_eq!(format!("{}", iri), "<http://example.org/resource>");
    }
}
