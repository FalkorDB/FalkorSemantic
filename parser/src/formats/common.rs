//! Common Parser Infrastructure
//!
//! Shared types, traits, and utilities for all RDF parsers.
//! This module reduces code duplication across parser implementations.

use std::io::BufRead;

use crate::rdf::{BlankNode, GraphName, Iri, Literal, Object, Quad, Subject, Triple};
use crate::ParserError;

/// Error information with location details
///
/// Used by all parsers to provide consistent error reporting
/// with optional line and column information.
#[derive(Debug, Clone)]
pub struct ParseErrorInfo {
    /// The error message
    pub message: String,
    /// Line number where the error occurred (1-indexed)
    pub line: Option<u64>,
    /// Column number where the error occurred (1-indexed)
    pub column: Option<u64>,
}

impl ParseErrorInfo {
    /// Create a new parse error with just a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// Create a parse error with line information
    pub fn with_line(message: impl Into<String>, line: u64) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
            column: None,
        }
    }

    /// Create a parse error with full location information
    pub fn with_location(message: impl Into<String>, line: u64, column: u64) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
            column: Some(column),
        }
    }
}

impl std::fmt::Display for ParseErrorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => {
                write!(f, "{}:{}: {}", line, col, self.message)
            }
            (Some(line), None) => {
                write!(f, "line {}: {}", line, self.message)
            }
            _ => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ParseErrorInfo {}

impl From<ParserError> for ParseErrorInfo {
    fn from(e: ParserError) -> Self {
        Self::new(e.to_string())
    }
}

/// Result type for parsing a single triple
pub type ParseTripleResult = std::result::Result<Triple, ParseErrorInfo>;

/// Result type for parsing a single quad
pub type ParseQuadResult = std::result::Result<Quad, ParseErrorInfo>;

/// Configuration for RDF parsers
#[derive(Debug, Clone, Default)]
pub struct ParserConfig {
    /// Prefix for blank node identifiers (for scoping across documents)
    pub blank_node_prefix: Option<String>,
    /// Base IRI for resolving relative references
    pub base_iri: Option<String>,
}

impl ParserConfig {
    /// Create a new parser configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set blank node prefix
    pub fn with_blank_node_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.blank_node_prefix = Some(prefix.into());
        self
    }

    /// Set base IRI
    pub fn with_base_iri(mut self, base: impl Into<String>) -> Self {
        self.base_iri = Some(base.into());
        self
    }
}

/// Trait for RDF triple parsers
///
/// Provides a unified interface for all triple-producing parsers.
pub trait TripleParser {
    /// Parse all triples from a string
    fn parse_str(&self, input: &str) -> Result<Vec<Triple>, ParseErrorInfo>;

    /// Parse all triples from a reader
    fn parse_read<R: BufRead>(&self, reader: R) -> Result<Vec<Triple>, ParseErrorInfo>;
}

/// Trait for RDF quad parsers
///
/// Provides a unified interface for all quad-producing parsers.
pub trait QuadParser {
    /// Parse all quads from a string
    fn parse_str(&self, input: &str) -> Result<Vec<Quad>, ParseErrorInfo>;

    /// Parse all quads from a reader
    fn parse_read<R: BufRead>(&self, reader: R) -> Result<Vec<Quad>, ParseErrorInfo>;
}

/// Converter for `rio_api` types to `FalkorSemantic` RDF types
///
/// This consolidates all the conversion logic in one place,
/// eliminating duplication across N-Triples, N-Quads, and other rio-based parsers.
pub struct RioConverter {
    blank_node_prefix: Option<String>,
}

impl RioConverter {
    /// Create a new converter
    #[must_use]
    pub const fn new() -> Self {
        Self {
            blank_node_prefix: None,
        }
    }

    /// Create a converter with a blank node prefix
    pub fn with_blank_node_prefix(prefix: impl Into<String>) -> Self {
        Self {
            blank_node_prefix: Some(prefix.into()),
        }
    }

    /// Get the blank node prefix
    #[must_use]
    pub fn blank_node_prefix(&self) -> Option<&str> {
        self.blank_node_prefix.as_deref()
    }

    /// Convert a rio Triple to our Triple type
    pub fn convert_triple(
        &self,
        rio_triple: rio_api::model::Triple<'_>,
    ) -> Result<Triple, ParserError> {
        let subject = self.convert_subject(rio_triple.subject)?;
        let predicate = self.convert_predicate(rio_triple.predicate)?;
        let object = self.convert_object(rio_triple.object)?;
        Ok(Triple::new(subject, predicate, object))
    }

    /// Convert a rio Quad to our Quad type
    pub fn convert_quad(&self, rio_quad: rio_api::model::Quad<'_>) -> Result<Quad, ParserError> {
        let subject = self.convert_subject(rio_quad.subject)?;
        let predicate = self.convert_predicate(rio_quad.predicate)?;
        let object = self.convert_object(rio_quad.object)?;
        let graph_name = self.convert_graph_name(rio_quad.graph_name)?;

        let triple = Triple::new(subject, predicate, object);
        Ok(Quad::new(triple, graph_name))
    }

    /// Convert a rio Subject to our Subject type
    pub fn convert_subject(
        &self,
        subject: rio_api::model::Subject<'_>,
    ) -> Result<Subject, ParserError> {
        match subject {
            rio_api::model::Subject::NamedNode(nn) => Ok(Subject::Iri(Iri::new(nn.iri)?)),
            rio_api::model::Subject::BlankNode(bn) => {
                Ok(Subject::BlankNode(self.convert_blank_node(bn.id)))
            }
            rio_api::model::Subject::Triple(_) => Err(ParserError::ParseError(
                "RDF-star quoted triples not supported".into(),
            )),
        }
    }

    /// Convert a rio `NamedNode` to our Iri type
    pub fn convert_predicate(
        &self,
        predicate: rio_api::model::NamedNode<'_>,
    ) -> Result<Iri, ParserError> {
        Iri::new(predicate.iri)
    }

    /// Convert a rio Term to our Object type
    pub fn convert_object(&self, object: rio_api::model::Term<'_>) -> Result<Object, ParserError> {
        match object {
            rio_api::model::Term::NamedNode(nn) => Ok(Object::Iri(Iri::new(nn.iri)?)),
            rio_api::model::Term::BlankNode(bn) => {
                Ok(Object::BlankNode(self.convert_blank_node(bn.id)))
            }
            rio_api::model::Term::Literal(lit) => Ok(Object::Literal(self.convert_literal(lit)?)),
            rio_api::model::Term::Triple(_) => Err(ParserError::ParseError(
                "RDF-star quoted triples not supported".into(),
            )),
        }
    }

    /// Convert a rio `GraphName` to our `GraphName` type
    pub fn convert_graph_name(
        &self,
        graph_name: Option<rio_api::model::GraphName<'_>>,
    ) -> Result<Option<GraphName>, ParserError> {
        match graph_name {
            None => Ok(None),
            Some(rio_api::model::GraphName::NamedNode(nn)) => {
                Ok(Some(GraphName::Iri(Iri::new(nn.iri)?)))
            }
            Some(rio_api::model::GraphName::BlankNode(bn)) => {
                Ok(Some(GraphName::BlankNode(self.convert_blank_node(bn.id))))
            }
        }
    }

    /// Convert a blank node ID, applying prefix if configured
    fn convert_blank_node(&self, id: &str) -> BlankNode {
        let label = if let Some(ref prefix) = self.blank_node_prefix {
            format!("{prefix}_{id}")
        } else {
            id.to_string()
        };
        BlankNode::new(label)
    }

    /// Convert a rio Literal to our Literal type
    fn convert_literal(&self, lit: rio_api::model::Literal<'_>) -> Result<Literal, ParserError> {
        match lit {
            rio_api::model::Literal::Simple { value } => Ok(Literal::new(value)),
            rio_api::model::Literal::LanguageTaggedString { value, language } => {
                Literal::with_language(value, language)
                    .map_err(|e| ParserError::ParseError(e.to_string()))
            }
            rio_api::model::Literal::Typed { value, datatype } => {
                Ok(Literal::with_datatype(value, Iri::new(datatype.iri)?))
            }
        }
    }
}

impl Default for RioConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a `TurtleError` from a `ParserError`
#[must_use]
pub fn parser_error_to_turtle_error(e: ParserError) -> rio_turtle::TurtleError {
    rio_turtle::TurtleError::from(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        e.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_info_display() {
        let err = ParseErrorInfo::new("test error");
        assert_eq!(format!("{}", err), "test error");

        let err_line = ParseErrorInfo::with_line("error at line", 42);
        assert_eq!(format!("{}", err_line), "line 42: error at line");

        let err_loc = ParseErrorInfo::with_location("full location", 10, 5);
        assert_eq!(format!("{}", err_loc), "10:5: full location");
    }

    #[test]
    fn test_parser_config() {
        let config = ParserConfig::new()
            .with_blank_node_prefix("doc1")
            .with_base_iri("http://example.org/");

        assert_eq!(config.blank_node_prefix, Some("doc1".to_string()));
        assert_eq!(config.base_iri, Some("http://example.org/".to_string()));
    }

    #[test]
    fn test_rio_converter_blank_node_prefix() {
        let converter = RioConverter::with_blank_node_prefix("test");
        let bn = converter.convert_blank_node("node1");
        assert_eq!(bn.label(), "test_node1");
    }
}
