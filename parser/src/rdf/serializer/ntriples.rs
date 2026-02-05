//! N-Triples Serializer
//!
//! Serializes RDF triples in the W3C N-Triples format.
//! <https://www.w3.org/TR/n-triples>/

use std::io::Write;

use super::error::SerializerResult;
use super::traits::{escape_string, GraphSerializer, TripleSerializer};
use crate::rdf::{Literal, Object, Subject, Triple};

/// N-Triples serializer
///
/// Serializes RDF triples in the simple line-based N-Triples format.
/// Each triple is written on a single line as: subject predicate object .
#[derive(Debug, Default)]
pub struct NTriplesSerializer;

impl NTriplesSerializer {
    /// Create a new N-Triples serializer
    #[must_use] 
    pub const fn new() -> Self {
        Self
    }

    /// Serialize a subject to N-Triples format
    fn write_subject<W: Write>(&self, subject: &Subject, writer: &mut W) -> SerializerResult<()> {
        match subject {
            Subject::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            Subject::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
        }
        Ok(())
    }

    /// Serialize an object to N-Triples format
    fn write_object<W: Write>(&self, object: &Object, writer: &mut W) -> SerializerResult<()> {
        match object {
            Object::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            Object::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
            Object::Literal(lit) => self.write_literal(lit, writer)?,
        }
        Ok(())
    }

    /// Serialize a literal to N-Triples format
    fn write_literal<W: Write>(&self, literal: &Literal, writer: &mut W) -> SerializerResult<()> {
        let escaped = escape_string(literal.value());
        write!(writer, "\"{escaped}\"")?;

        if let Some(lang) = literal.language() {
            write!(writer, "@{lang}")?;
        } else if let Some(datatype) = literal.explicit_datatype() {
            // Only write datatype if it's not xsd:string (the default)
            if datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                write!(writer, "^^<{}>", datatype.as_str())?;
            }
        }
        Ok(())
    }
}

impl TripleSerializer for NTriplesSerializer {
    fn serialize_triple<W: Write>(
        &mut self,
        triple: &Triple,
        writer: &mut W,
    ) -> SerializerResult<()> {
        self.write_subject(&triple.subject, writer)?;
        write!(writer, " ")?;
        write!(writer, "<{}>", triple.predicate.as_str())?;
        write!(writer, " ")?;
        self.write_object(&triple.object, writer)?;
        writeln!(writer, " .")?;
        Ok(())
    }
}

impl GraphSerializer for NTriplesSerializer {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{BlankNode, Iri, Literal};

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_simple_triple() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/predicate"),
            test_iri("http://example.org/object"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(
            result,
            "<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> .\n"
        );
    }

    #[test]
    fn test_literal_object() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/name"),
            Literal::new("John Doe"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(
            result,
            "<http://example.org/subject> <http://example.org/name> \"John Doe\" .\n"
        );
    }

    #[test]
    fn test_language_tagged_literal() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/label"),
            Literal::with_language("Bonjour", "fr").unwrap(),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(
            result,
            "<http://example.org/subject> <http://example.org/label> \"Bonjour\"@fr .\n"
        );
    }

    #[test]
    fn test_typed_literal() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/age"),
            Literal::integer(42),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
    }

    #[test]
    fn test_blank_node_subject() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            BlankNode::new("node1"),
            test_iri("http://example.org/predicate"),
            Literal::new("value"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.starts_with("_:node1"));
    }

    #[test]
    fn test_escape_special_chars() {
        let mut serializer = NTriplesSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/text"),
            Literal::new("line1\nline2\t\"quoted\""),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("line1\\nline2\\t\\\"quoted\\\""));
    }

    #[test]
    fn test_multiple_triples() {
        let mut serializer = NTriplesSerializer::new();
        let triples = vec![
            Triple::new(
                test_iri("http://example.org/s1"),
                test_iri("http://example.org/p"),
                Literal::new("v1"),
            ),
            Triple::new(
                test_iri("http://example.org/s2"),
                test_iri("http://example.org/p"),
                Literal::new("v2"),
            ),
        ];

        let mut output = Vec::new();
        serializer
            .serialize_triples(triples.iter(), &mut output)
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        let lines: Vec<_> = result.lines().collect();
        assert_eq!(lines.len(), 2);
    }
}
