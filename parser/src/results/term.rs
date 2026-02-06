//! Term representation for SPARQL results
//!
//! Represents values that can appear in query results.

use crate::rdf::{BlankNode, Iri, Literal};
use std::fmt;

/// A term in SPARQL results
///
/// Represents any value that can appear in a result binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    /// An IRI reference
    Iri(Iri),
    /// A literal value with optional language tag or datatype
    Literal(Literal),
    /// A blank node
    BlankNode(BlankNode),
}

impl Term {
    /// Create a new IRI term
    pub fn iri(value: impl Into<String>) -> Self {
        Self::Iri(Iri::new_unchecked(value))
    }

    /// Create a new simple literal term
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(Literal::new(value))
    }

    /// Create a new typed literal term
    pub fn typed_literal(value: impl Into<String>, datatype: impl Into<String>) -> Self {
        Self::Literal(Literal::with_datatype(value, Iri::new_unchecked(datatype)))
    }

    /// Create a new language-tagged literal term
    pub fn lang_literal(value: impl Into<String>, lang: impl Into<String>) -> Self {
        // Use with_language which returns Result, unwrap since we trust the input
        Self::Literal(Literal::with_language(value, lang).unwrap_or_else(|_| Literal::new("")))
    }

    /// Create a new blank node term
    pub fn blank_node(id: impl Into<String>) -> Self {
        Self::BlankNode(BlankNode::new(id))
    }

    /// Check if this is an IRI
    #[must_use]
    pub const fn is_iri(&self) -> bool {
        matches!(self, Self::Iri(_))
    }

    /// Check if this is a literal
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(_))
    }

    /// Check if this is a blank node
    #[must_use]
    pub const fn is_blank_node(&self) -> bool {
        matches!(self, Self::BlankNode(_))
    }

    /// Get as IRI if this is an IRI term
    #[must_use]
    pub const fn as_iri(&self) -> Option<&Iri> {
        match self {
            Self::Iri(iri) => Some(iri),
            _ => None,
        }
    }

    /// Get as literal if this is a literal term
    #[must_use]
    pub const fn as_literal(&self) -> Option<&Literal> {
        match self {
            Self::Literal(lit) => Some(lit),
            _ => None,
        }
    }

    /// Get as blank node if this is a blank node term
    #[must_use]
    pub const fn as_blank_node(&self) -> Option<&BlankNode> {
        match self {
            Self::BlankNode(bn) => Some(bn),
            _ => None,
        }
    }

    /// Get the term type as a string (for JSON serialization)
    #[must_use]
    pub const fn term_type(&self) -> &'static str {
        match self {
            Self::Iri(_) => "uri",
            Self::Literal(_) => "literal",
            Self::BlankNode(_) => "bnode",
        }
    }

    /// Get the value as a string
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::Iri(iri) => iri.as_str(),
            Self::Literal(lit) => lit.value(),
            Self::BlankNode(bn) => bn.label(),
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Iri(iri) => write!(f, "<{iri}>"),
            Self::Literal(lit) => write!(f, "{lit}"),
            Self::BlankNode(bn) => write!(f, "_:{}", bn.label()),
        }
    }
}

impl From<Iri> for Term {
    fn from(iri: Iri) -> Self {
        Self::Iri(iri)
    }
}

impl From<Literal> for Term {
    fn from(lit: Literal) -> Self {
        Self::Literal(lit)
    }
}

impl From<BlankNode> for Term {
    fn from(bn: BlankNode) -> Self {
        Self::BlankNode(bn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_iri() {
        let term = Term::iri("http://example.org/resource");
        assert!(term.is_iri());
        assert!(!term.is_literal());
        assert!(!term.is_blank_node());
        assert_eq!(term.term_type(), "uri");
        assert_eq!(term.value(), "http://example.org/resource");
    }

    #[test]
    fn test_term_literal() {
        let term = Term::literal("hello");
        assert!(!term.is_iri());
        assert!(term.is_literal());
        assert!(!term.is_blank_node());
        assert_eq!(term.term_type(), "literal");
        assert_eq!(term.value(), "hello");
    }

    #[test]
    fn test_term_blank_node() {
        let term = Term::blank_node("b1");
        assert!(!term.is_iri());
        assert!(!term.is_literal());
        assert!(term.is_blank_node());
        assert_eq!(term.term_type(), "bnode");
        assert_eq!(term.value(), "b1");
    }

    #[test]
    fn test_typed_literal() {
        let term = Term::typed_literal("42", "http://www.w3.org/2001/XMLSchema#integer");
        assert!(term.is_literal());
        let lit = term.as_literal().unwrap();
        assert!(lit.explicit_datatype().is_some());
    }

    #[test]
    fn test_lang_literal() {
        let term = Term::lang_literal("hello", "en");
        assert!(term.is_literal());
        let lit = term.as_literal().unwrap();
        assert_eq!(lit.language(), Some("en"));
    }
}
