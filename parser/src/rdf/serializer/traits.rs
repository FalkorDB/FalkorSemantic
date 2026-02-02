//! Serializer traits
//!
//! Defines the common interface for RDF serializers with streaming support.

use std::io::Write;

use super::error::SerializerResult;
use crate::rdf::{Quad, Triple};

/// Trait for serializing individual RDF triples
pub trait TripleSerializer {
    /// Serialize a single triple to the writer
    fn serialize_triple<W: Write>(&mut self, triple: &Triple, writer: &mut W) -> SerializerResult<()>;

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
    !local.is_empty() && local.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
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
}
