//! Turtle Serializer
//!
//! Serializes RDF triples in the W3C Turtle format with prefix support.
//! <https://www.w3.org/TR/turtle>/

use std::collections::HashMap;
use std::io::Write;

use super::error::SerializerResult;
use super::traits::{escape_string, GraphSerializer, TripleSerializer};
use crate::rdf::{Iri, Literal, Object, Subject, Triple};

/// Turtle serializer with prefix support
///
/// Serializes RDF triples in the human-readable Turtle format.
/// Supports namespace prefixes for compact output.
#[derive(Debug)]
pub struct TurtleSerializer {
    /// Namespace prefixes (prefix -> IRI)
    prefixes: HashMap<String, String>,
    /// Whether the header has been written
    header_written: bool,
    /// Current subject for grouping predicates
    current_subject: Option<String>,
    /// Whether we're in the middle of a subject block
    in_subject_block: bool,
}

impl Default for TurtleSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl TurtleSerializer {
    /// Create a new Turtle serializer
    #[must_use]
    pub fn new() -> Self {
        Self {
            prefixes: HashMap::new(),
            header_written: false,
            current_subject: None,
            in_subject_block: false,
        }
    }

    /// Create a Turtle serializer with common prefixes
    #[must_use]
    pub fn with_common_prefixes() -> Self {
        let mut serializer = Self::new();
        serializer.add_prefix("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#");
        serializer.add_prefix("rdfs", "http://www.w3.org/2000/01/rdf-schema#");
        serializer.add_prefix("xsd", "http://www.w3.org/2001/XMLSchema#");
        serializer.add_prefix("owl", "http://www.w3.org/2002/07/owl#");
        serializer
    }

    /// Add a namespace prefix
    pub fn add_prefix(&mut self, prefix: &str, iri: &str) {
        self.prefixes.insert(prefix.to_string(), iri.to_string());
    }

    /// Add multiple prefixes from a map
    pub fn add_prefixes(&mut self, prefixes: impl IntoIterator<Item = (String, String)>) {
        for (prefix, iri) in prefixes {
            self.prefixes.insert(prefix, iri);
        }
    }

    /// Try to compact an IRI using prefixes
    fn compact_iri(&self, iri: &str) -> String {
        for (prefix, namespace) in &self.prefixes {
            if iri.starts_with(namespace) {
                let local = &iri[namespace.len()..];
                // Check if local name is valid (no special chars)
                if Self::is_valid_local_name(local) {
                    return format!("{prefix}:{local}");
                }
            }
        }
        // Return full IRI in angle brackets
        format!("<{iri}>")
    }

    /// Check if a string is a valid Turtle local name
    #[must_use]
    pub fn is_valid_local_name(s: &str) -> bool {
        if s.is_empty() {
            return true; // Empty local name is valid (just prefix:)
        }

        let mut chars = s.chars();

        // First char must be a letter, underscore, or digit for PN_LOCAL
        if let Some(first) = chars.next() {
            if !first.is_alphanumeric() && first != '_' {
                return false;
            }
        }

        // Remaining chars
        for c in chars {
            if !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' {
                return false;
            }
        }

        true
    }

    /// Write a subject in Turtle format
    fn write_subject<W: Write>(&self, subject: &Subject, writer: &mut W) -> SerializerResult<()> {
        match subject {
            Subject::Iri(iri) => write!(writer, "{}", self.compact_iri(iri.as_str()))?,
            Subject::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
        }
        Ok(())
    }

    /// Write a predicate in Turtle format
    fn write_predicate<W: Write>(&self, predicate: &Iri, writer: &mut W) -> SerializerResult<()> {
        // Special case for rdf:type -> 'a'
        if predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            write!(writer, "a")?;
        } else {
            write!(writer, "{}", self.compact_iri(predicate.as_str()))?;
        }
        Ok(())
    }

    /// Write an object in Turtle format
    fn write_object<W: Write>(&self, object: &Object, writer: &mut W) -> SerializerResult<()> {
        match object {
            Object::Iri(iri) => write!(writer, "{}", self.compact_iri(iri.as_str()))?,
            Object::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
            Object::Literal(lit) => self.write_literal(lit, writer)?,
        }
        Ok(())
    }

    /// Write a literal in Turtle format
    fn write_literal<W: Write>(&self, literal: &Literal, writer: &mut W) -> SerializerResult<()> {
        let value = literal.value();

        // Check if we can use a short form for numbers/booleans
        if let Some(datatype) = literal.explicit_datatype() {
            let dt_str = datatype.as_str();

            // Integer shorthand
            if dt_str == "http://www.w3.org/2001/XMLSchema#integer" && value.parse::<i64>().is_ok()
            {
                write!(writer, "{value}")?;
                return Ok(());
            }

            // Decimal shorthand
            if (dt_str == "http://www.w3.org/2001/XMLSchema#decimal"
                || dt_str == "http://www.w3.org/2001/XMLSchema#double")
                && value.parse::<f64>().is_ok()
                && value.contains('.')
            {
                write!(writer, "{value}")?;
                return Ok(());
            }

            // Boolean shorthand
            if dt_str == "http://www.w3.org/2001/XMLSchema#boolean"
                && (value == "true" || value == "false")
            {
                write!(writer, "{value}")?;
                return Ok(());
            }
        }

        // Check if multiline string
        if value.contains('\n') || value.contains('\r') {
            let escaped = escape_string(value);
            write!(writer, "\"\"\"{escaped}\"\"\"")?;
        } else {
            let escaped = escape_string(value);
            write!(writer, "\"{escaped}\"")?;
        }

        if let Some(lang) = literal.language() {
            write!(writer, "@{lang}")?;
        } else if let Some(datatype) = literal.explicit_datatype() {
            let dt_str = datatype.as_str();
            // Don't write xsd:string as it's the default
            if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                write!(writer, "^^{}", self.compact_iri(dt_str))?;
            }
        }

        Ok(())
    }

    /// Get subject as string for comparison
    fn subject_string(&self, subject: &Subject) -> String {
        match subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::BlankNode(bn) => format!("_:{}", bn.label()),
        }
    }
}

impl TripleSerializer for TurtleSerializer {
    fn serialize_triple<W: Write>(
        &mut self,
        triple: &Triple,
        writer: &mut W,
    ) -> SerializerResult<()> {
        let subject_str = self.subject_string(&triple.subject);

        // Check if this is the same subject as the previous triple
        let same_subject = self.current_subject.as_ref() == Some(&subject_str);

        if same_subject && self.in_subject_block {
            // Continue previous subject block
            write!(writer, " ;\n    ")?;
        } else {
            // Close previous subject block if any
            if self.in_subject_block {
                writeln!(writer, " .")?;
            }

            // Start new subject block
            self.write_subject(&triple.subject, writer)?;
            write!(writer, " ")?;
            self.current_subject = Some(subject_str);
            self.in_subject_block = true;
        }

        self.write_predicate(&triple.predicate, writer)?;
        write!(writer, " ")?;
        self.write_object(&triple.object, writer)?;

        Ok(())
    }

    fn finish<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        if self.in_subject_block {
            writeln!(writer, " .")?;
            self.in_subject_block = false;
            self.current_subject = None;
        }
        Ok(())
    }
}

impl GraphSerializer for TurtleSerializer {
    fn write_header<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        if self.header_written {
            return Ok(());
        }

        // Write prefix declarations
        let mut prefixes: Vec<_> = self.prefixes.iter().collect();
        prefixes.sort_by_key(|(k, _)| k.as_str());

        for (prefix, iri) in prefixes {
            writeln!(writer, "@prefix {prefix}: <{iri}> .")?;
        }

        if !self.prefixes.is_empty() {
            writeln!(writer)?;
        }

        self.header_written = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::BlankNode;

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_simple_triple() {
        let mut serializer = TurtleSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/predicate"),
            Literal::new("value"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("<http://example.org/subject>"));
        assert!(result.contains("\"value\""));
    }

    #[test]
    fn test_with_prefixes() {
        let mut serializer = TurtleSerializer::new();
        serializer.add_prefix("ex", "http://example.org/");

        let triple = Triple::new(
            test_iri("http://example.org/subject"),
            test_iri("http://example.org/predicate"),
            test_iri("http://example.org/object"),
        );

        let mut output = Vec::new();
        serializer.write_header(&mut output).unwrap();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("@prefix ex:"));
        assert!(result.contains("ex:subject"));
        assert!(result.contains("ex:predicate"));
        assert!(result.contains("ex:object"));
    }

    #[test]
    fn test_rdf_type_shorthand() {
        let mut serializer = TurtleSerializer::new();
        serializer.add_prefix("ex", "http://example.org/");

        let triple = Triple::new(
            test_iri("http://example.org/thing"),
            test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            test_iri("http://example.org/Class"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(
            result.contains(" a "),
            "Expected 'a' shorthand for rdf:type"
        );
    }

    #[test]
    fn test_integer_shorthand() {
        let mut serializer = TurtleSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/thing"),
            test_iri("http://example.org/count"),
            Literal::integer(42),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(
            result.contains(" 42 ") || result.contains(" 42."),
            "Expected integer shorthand"
        );
    }

    #[test]
    fn test_same_subject_grouping() {
        let mut serializer = TurtleSerializer::new();
        let triples = vec![
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p1"),
                Literal::new("v1"),
            ),
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p2"),
                Literal::new("v2"),
            ),
        ];

        let mut output = Vec::new();
        for triple in &triples {
            serializer.serialize_triple(triple, &mut output).unwrap();
        }
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        // Should use semicolon to separate predicates for same subject
        assert!(
            result.contains(";"),
            "Expected semicolon for predicate grouping"
        );
        // Should only have one period at the end
        assert_eq!(result.matches(" .").count(), 1, "Expected single period");
    }

    #[test]
    fn test_language_tag() {
        let mut serializer = TurtleSerializer::new();
        let triple = Triple::new(
            test_iri("http://example.org/thing"),
            test_iri("http://example.org/label"),
            Literal::with_language("Bonjour", "fr").unwrap(),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"Bonjour\"@fr"));
    }

    #[test]
    fn test_blank_node() {
        let mut serializer = TurtleSerializer::new();
        let triple = Triple::new(
            BlankNode::new("b0"),
            test_iri("http://example.org/predicate"),
            Literal::new("value"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.starts_with("_:b0"));
    }
}
