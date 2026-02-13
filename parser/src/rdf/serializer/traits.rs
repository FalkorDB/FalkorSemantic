//! Serializer traits
//!
//! Defines the common interface for RDF serializers with streaming support.

use std::io::Write;

use super::error::SerializerResult;
use crate::rdf::{Quad, Triple};

/// Trait for serializing individual RDF triples
pub trait TripleSerializer {
    /// Serialize a single triple to the writer
    fn serialize_triple<W: Write>(
        &mut self,
        triple: &Triple,
        writer: &mut W,
    ) -> SerializerResult<()>;

    /// Serialize multiple triples to the writer
    fn serialize_triples<'a, W, I>(&mut self, triples: I, writer: &mut W) -> SerializerResult<()>
    where
        W: Write,
        I: Iterator<Item = &'a Triple>,
    {
        for triple in triples {
            self.serialize_triple(triple, writer)?;
        }
        Ok(())
    }

    /// Finalize the serialization (write any closing content)
    fn finish<W: Write>(&mut self, _writer: &mut W) -> SerializerResult<()> {
        Ok(())
    }
}

/// Trait for serializing RDF quads (triples with named graphs)
pub trait QuadSerializer {
    /// Serialize a single quad to the writer
    fn serialize_quad<W: Write>(&mut self, quad: &Quad, writer: &mut W) -> SerializerResult<()>;

    /// Serialize multiple quads to the writer
    fn serialize_quads<'a, W, I>(&mut self, quads: I, writer: &mut W) -> SerializerResult<()>
    where
        W: Write,
        I: Iterator<Item = &'a Quad>,
    {
        for quad in quads {
            self.serialize_quad(quad, writer)?;
        }
        Ok(())
    }

    /// Finalize the serialization (write any closing content)
    fn finish<W: Write>(&mut self, _writer: &mut W) -> SerializerResult<()> {
        Ok(())
    }
}

/// Trait for serializing complete RDF graphs
pub trait GraphSerializer: TripleSerializer {
    /// Write any header/prefix declarations
    fn write_header<W: Write>(&mut self, _writer: &mut W) -> SerializerResult<()> {
        Ok(())
    }

    /// Serialize a complete graph with header and footer
    fn serialize_graph<'a, W, I>(&mut self, triples: I, writer: &mut W) -> SerializerResult<()>
    where
        W: Write,
        I: Iterator<Item = &'a Triple>,
    {
        self.write_header(writer)?;
        self.serialize_triples(triples, writer)?;
        self.finish(writer)?;
        Ok(())
    }
}

/// Helper function to escape a string for N-Triples/Turtle format
pub fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                // Escape control characters as \uXXXX
                result.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Helper function to escape a string for JSON
pub fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Check if a local name is valid for prefix compaction
/// A valid local name contains only alphanumeric characters, underscores, and hyphens
pub fn is_valid_local_name(local: &str) -> bool {
    !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_string("tab\there"), "tab\\there");
        assert_eq!(escape_string("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello"), "hello");
        assert_eq!(escape_json_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_json_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_escape_string_carriage_return() {
        assert_eq!(escape_string("line\rend"), "line\\rend");
    }

    #[test]
    fn test_escape_string_control_char() {
        // BEL character (0x07) should be escaped as \u0007
        assert_eq!(escape_string("\x07"), "\\u0007");
    }

    #[test]
    fn test_escape_json_string_backspace_formfeed() {
        assert_eq!(escape_json_string("\x08"), "\\b");
        assert_eq!(escape_json_string("\x0C"), "\\f");
    }

    #[test]
    fn test_escape_json_string_control_char() {
        assert_eq!(escape_json_string("\x07"), "\\u0007");
    }

    #[test]
    fn test_is_valid_local_name_valid() {
        assert!(is_valid_local_name("Person"));
        assert!(is_valid_local_name("schema_type"));
        assert!(is_valid_local_name("my-name"));
        assert!(is_valid_local_name("abc123"));
    }

    #[test]
    fn test_is_valid_local_name_invalid() {
        assert!(!is_valid_local_name("")); // empty
        assert!(!is_valid_local_name("has space"));
        assert!(!is_valid_local_name("has.dot"));
        assert!(!is_valid_local_name("has/slash"));
    }

    // A minimal TripleSerializer to exercise default serialize_triples
    struct NoOpTripleSerializer;
    impl super::TripleSerializer for NoOpTripleSerializer {
        fn serialize_triple<W: std::io::Write>(
            &mut self,
            _triple: &crate::rdf::Triple,
            _writer: &mut W,
        ) -> super::SerializerResult<()> {
            Ok(())
        }
    }

    // A minimal QuadSerializer to exercise default serialize_quads
    struct NoOpQuadSerializer;
    impl super::QuadSerializer for NoOpQuadSerializer {
        fn serialize_quad<W: std::io::Write>(
            &mut self,
            _quad: &crate::rdf::Quad,
            _writer: &mut W,
        ) -> super::SerializerResult<()> {
            Ok(())
        }
    }

    // GraphSerializer requires TripleSerializer
    impl super::GraphSerializer for NoOpTripleSerializer {}

    fn make_triple() -> crate::rdf::Triple {
        use crate::rdf::{Iri, Literal, Subject, Triple};
        Triple::new(
            Subject::Iri(Iri::new("http://example.org/s").unwrap()),
            Iri::new("http://example.org/p").unwrap(),
            Literal::new("hello"),
        )
    }

    fn make_quad() -> crate::rdf::Quad {
        crate::rdf::Quad::new(make_triple(), None)
    }

    #[test]
    fn test_serialize_triples_default() {
        let mut ser = NoOpTripleSerializer;
        let triple = make_triple();
        let triples = [triple];
        let mut buf = Vec::new();
        assert!(ser.serialize_triples(triples.iter(), &mut buf).is_ok());
    }

    #[test]
    fn test_serialize_quads_default() {
        let mut ser = NoOpQuadSerializer;
        let quad = make_quad();
        let quads = [quad];
        let mut buf = Vec::new();
        assert!(ser.serialize_quads(quads.iter(), &mut buf).is_ok());
    }

    #[test]
    fn test_serialize_graph_default() {
        let mut ser = NoOpTripleSerializer;
        let triple = make_triple();
        let triples = [triple];
        let mut buf = Vec::new();
        assert!(ser.serialize_graph(triples.iter(), &mut buf).is_ok());
    }

    #[test]
    fn test_finish_default() {
        let mut ser = NoOpTripleSerializer;
        let mut buf = Vec::new();
        assert!(ser.finish(&mut buf).is_ok());

        let mut qser = NoOpQuadSerializer;
        assert!(qser.finish(&mut buf).is_ok());
    }
}
