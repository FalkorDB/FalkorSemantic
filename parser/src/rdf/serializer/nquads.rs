//! N-Quads Serializer
//!
//! Serializes RDF quads in the W3C N-Quads format.
//! <https://www.w3.org/TR/n-quads>/

use std::io::Write;

use super::error::SerializerResult;
use super::traits::QuadSerializer;
use crate::rdf::{GraphName, Quad};

/// N-Quads serializer
///
/// Serializes RDF quads in the N-Quads format, which extends N-Triples
/// with support for named graphs.
#[derive(Debug, Default)]
pub struct NQuadsSerializer;

impl NQuadsSerializer {
    /// Create a new N-Quads serializer
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Serialize a quad directly
    pub fn write_quad<W: Write>(&self, quad: &Quad, writer: &mut W) -> SerializerResult<()> {
        // Write subject
        match &quad.triple.subject {
            crate::rdf::Subject::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            crate::rdf::Subject::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
        }

        write!(writer, " ")?;

        // Write predicate
        write!(writer, "<{}>", quad.triple.predicate.as_str())?;

        write!(writer, " ")?;

        // Write object
        match &quad.triple.object {
            crate::rdf::Object::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            crate::rdf::Object::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
            crate::rdf::Object::Literal(lit) => {
                let escaped = super::traits::escape_string(lit.value());
                write!(writer, "\"{escaped}\"")?;
                if let Some(lang) = lit.language() {
                    write!(writer, "@{lang}")?;
                } else if let Some(datatype) = lit.explicit_datatype() {
                    if datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, "^^<{}>", datatype.as_str())?;
                    }
                }
            }
        }

        // Write graph name if present
        if let Some(graph) = &quad.graph {
            write!(writer, " ")?;
            match graph {
                GraphName::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
                GraphName::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
            }
        }

        writeln!(writer, " .")?;
        Ok(())
    }

    /// Serialize multiple quads
    pub fn write_quads<'a, W, I>(&self, quads: I, writer: &mut W) -> SerializerResult<()>
    where
        W: Write,
        I: Iterator<Item = &'a Quad>,
    {
        for quad in quads {
            self.write_quad(quad, writer)?;
        }
        Ok(())
    }
}

impl QuadSerializer for NQuadsSerializer {
    fn serialize_quad<W: Write>(&mut self, quad: &Quad, writer: &mut W) -> SerializerResult<()> {
        self.write_quad(quad, writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{Iri, Literal, Triple};

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_quad_default_graph() {
        let serializer = NQuadsSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            Literal::new("value"),
        );
        let quad = Quad::in_default_graph(triple);

        let mut output = Vec::new();
        serializer.write_quad(&quad, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(
            result,
            "<http://example.org/s> <http://example.org/p> \"value\" .\n"
        );
    }

    #[test]
    fn test_quad_named_graph() {
        let serializer = NQuadsSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            test_iri("http://example.org/o"),
        );
        let quad = Quad::in_named_graph(triple, test_iri("http://example.org/graph1"));

        let mut output = Vec::new();
        serializer.write_quad(&quad, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(
            result,
            "<http://example.org/s> <http://example.org/p> <http://example.org/o> <http://example.org/graph1> .\n"
        );
    }

    #[test]
    fn test_multiple_quads() {
        let serializer = NQuadsSerializer::new();
        let quads = vec![
            Quad::in_default_graph(Triple::new(
                test_iri("http://example.org/s1"),
                test_iri("http://example.org/p"),
                Literal::new("v1"),
            )),
            Quad::in_named_graph(
                Triple::new(
                    test_iri("http://example.org/s2"),
                    test_iri("http://example.org/p"),
                    Literal::new("v2"),
                ),
                test_iri("http://example.org/g"),
            ),
        ];

        let mut output = Vec::new();
        serializer.write_quads(quads.iter(), &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        let lines: Vec<_> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(!lines[0].contains("http://example.org/g"));
        assert!(lines[1].contains("http://example.org/g"));
    }
}
