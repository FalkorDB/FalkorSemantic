//! RDF Export Serializers
//!
//! This module provides serializers for exporting RDF data in various formats:
//! - N-Triples (text/n-triples)
//! - N-Quads (text/n-quads)
//! - Turtle (text/turtle)
//! - JSON-LD (application/ld+json)
//!
//! All serializers support streaming output for efficient handling of large graphs.

mod jsonld;
mod nquads;
mod ntriples;
mod turtle;

pub use jsonld::{write_jsonld, JsonLdWriter};
pub use nquads::{write_nquads, write_nquads_streaming, NQuadsWriter};
pub use ntriples::{write_ntriples, write_ntriples_streaming, NTriplesWriter};
pub use turtle::{write_turtle, TurtleWriter};

use std::io::Write;
use thiserror::Error;

/// Errors that can occur during RDF export
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Result type for export operations
pub type ExportResult<T> = std::result::Result<T, ExportError>;

/// Trait for RDF triple serializers
pub trait TripleWriter {
    /// Write a single triple
    fn write_triple<W: Write>(
        &self,
        triple: &crate::rdf::Triple,
        writer: &mut W,
    ) -> ExportResult<()>;

    /// Write multiple triples
    fn write_triples<'a, W, I>(&self, triples: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a crate::rdf::Triple>,
    {
        for triple in triples {
            self.write_triple(triple, writer)?;
        }
        Ok(())
    }
}

/// Trait for RDF quad serializers (triples with named graphs)
pub trait QuadWriter {
    /// Write a single quad
    fn write_quad<W: Write>(&self, quad: &crate::rdf::Quad, writer: &mut W) -> ExportResult<()>;

    /// Write multiple quads
    fn write_quads<'a, W, I>(&self, quads: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a crate::rdf::Quad>,
    {
        for quad in quads {
            self.write_quad(quad, writer)?;
        }
        Ok(())
    }
}

/// Available export formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// N-Triples (text/n-triples)
    NTriples,
    /// N-Quads (text/n-quads)
    NQuads,
    /// Turtle (text/turtle)
    Turtle,
    /// JSON-LD (application/ld+json)
    JsonLd,
}

impl ExportFormat {
    /// Get the MIME type for this format
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::NTriples => "text/n-triples",
            Self::NQuads => "text/n-quads",
            Self::Turtle => "text/turtle",
            Self::JsonLd => "application/ld+json",
        }
    }

    /// Get the file extension for this format
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::NTriples => "nt",
            Self::NQuads => "nq",
            Self::Turtle => "ttl",
            Self::JsonLd => "jsonld",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{Iri, Literal, Quad, Subject, Triple};

    #[test]
    fn test_export_format_mime_types() {
        assert_eq!(ExportFormat::NTriples.mime_type(), "text/n-triples");
        assert_eq!(ExportFormat::NQuads.mime_type(), "text/n-quads");
        assert_eq!(ExportFormat::Turtle.mime_type(), "text/turtle");
        assert_eq!(ExportFormat::JsonLd.mime_type(), "application/ld+json");
    }

    #[test]
    fn test_export_format_extensions() {
        assert_eq!(ExportFormat::NTriples.extension(), "nt");
        assert_eq!(ExportFormat::NQuads.extension(), "nq");
        assert_eq!(ExportFormat::Turtle.extension(), "ttl");
        assert_eq!(ExportFormat::JsonLd.extension(), "jsonld");
    }

    #[test]
    fn test_export_format_equality() {
        assert_eq!(ExportFormat::NTriples, ExportFormat::NTriples);
        assert_ne!(ExportFormat::NTriples, ExportFormat::Turtle);
    }

    #[test]
    fn test_export_error_display() {
        let io_err = ExportError::Io(std::io::Error::new(std::io::ErrorKind::Other, "test"));
        assert!(io_err.to_string().contains("IO error"));

        let ser_err = ExportError::Serialization("bad data".to_string());
        assert!(ser_err.to_string().contains("bad data"));

        let inv_err = ExportError::InvalidData("invalid".to_string());
        assert!(inv_err.to_string().contains("invalid"));
    }

    fn make_triple() -> Triple {
        Triple::new(
            Subject::Iri(Iri::new("http://example.org/s").unwrap()),
            Iri::new("http://example.org/p").unwrap(),
            Literal::new("hello"),
        )
    }

    fn make_quad() -> Quad {
        Quad::new(make_triple(), None)
    }

    // Minimal TripleWriter impl to exercise the default write_triples method
    struct TestTripleWriter;
    impl TripleWriter for TestTripleWriter {
        fn write_triple<W: Write>(
            &self,
            _triple: &crate::rdf::Triple,
            _writer: &mut W,
        ) -> ExportResult<()> {
            Ok(())
        }
    }

    // Minimal QuadWriter impl to exercise the default write_quads method
    struct TestQuadWriter;
    impl QuadWriter for TestQuadWriter {
        fn write_quad<W: Write>(
            &self,
            _quad: &crate::rdf::Quad,
            _writer: &mut W,
        ) -> ExportResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_triple_writer_write_triples_default() {
        let writer = TestTripleWriter;
        let triple = make_triple();
        let triples = [triple];
        let mut buf = Vec::new();
        let result = writer.write_triples(triples.iter(), &mut buf);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quad_writer_write_quads_default() {
        let writer = TestQuadWriter;
        let quad = make_quad();
        let quads = [quad];
        let mut buf = Vec::new();
        let result = writer.write_quads(quads.iter(), &mut buf);
        assert!(result.is_ok());
    }
}
