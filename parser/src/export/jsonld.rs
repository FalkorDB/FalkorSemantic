//! JSON-LD Serializer
//!
//! Implements the JSON-LD format as per:
//! https://www.w3.org/TR/json-ld11/
//!
//! JSON-LD is a JSON-based format to serialize Linked Data.

use super::{ExportResult, TripleWriter};
use crate::rdf::{Object, Subject, Triple};
use std::collections::HashMap;
use std::io::Write;

/// Writer for JSON-LD format
#[derive(Debug, Clone)]
pub struct JsonLdWriter {
    /// Context entries (term -> IRI or object)
    context: HashMap<String, ContextEntry>,
    /// Whether to pretty-print the JSON
    pretty: bool,
    /// Indentation string
    indent: String,
}

/// A context entry can be a simple IRI or a more complex definition
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ContextEntry {
    /// Simple IRI mapping
    Iri(String),
    /// Term definition with @id and optional @type
    Definition {
        id: String,
        value_type: Option<String>,
    },
}

impl Default for JsonLdWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLdWriter {
    /// Create a new JSON-LD writer
    pub fn new() -> Self {
        Self {
            context: HashMap::new(),
            pretty: true,
            indent: "  ".to_string(),
        }
    }

    /// Add a term to the context
    pub fn with_term(mut self, term: &str, iri: &str) -> Self {
        self.context
            .insert(term.to_string(), ContextEntry::Iri(iri.to_string()));
        self
    }

    /// Add a prefix to the context
    pub fn with_prefix(mut self, prefix: &str, namespace: &str) -> Self {
        self.context
            .insert(prefix.to_string(), ContextEntry::Iri(namespace.to_string()));
        self
    }

    /// Add common prefixes
    pub fn with_common_prefixes(self) -> Self {
        self.with_prefix("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#")
            .with_prefix("rdfs", "http://www.w3.org/2000/01/rdf-schema#")
            .with_prefix("xsd", "http://www.w3.org/2001/XMLSchema#")
    }

    /// Disable pretty-printing
    pub fn compact(mut self) -> Self {
        self.pretty = false;
        self
    }

    /// Write JSON-escaped string content
    fn write_json_string<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '"' => write!(writer, "\\\"")?,
                '\\' => write!(writer, "\\\\")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c if c.is_control() => write!(writer, "\\u{:04X}", c as u32)?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    /// Write newline if pretty-printing
    fn nl<W: Write>(&self, writer: &mut W) -> ExportResult<()> {
        if self.pretty {
            writeln!(writer)?;
        }
        Ok(())
    }

    /// Write indentation
    fn write_indent<W: Write>(&self, writer: &mut W, level: usize) -> ExportResult<()> {
        if self.pretty {
            for _ in 0..level {
                write!(writer, "{}", self.indent)?;
            }
        }
        Ok(())
    }

    /// Try to compact an IRI using context
    fn compact_iri(&self, iri: &str) -> String {
        for (prefix, entry) in &self.context {
            if let ContextEntry::Iri(namespace) = entry {
                if iri.starts_with(namespace) {
                    let local = &iri[namespace.len()..];
                    return format!("{}:{}", prefix, local);
                }
            }
        }
        iri.to_string()
    }

    /// Write the context
    fn write_context<W: Write>(&self, writer: &mut W, level: usize) -> ExportResult<()> {
        if self.context.is_empty() {
            return Ok(());
        }

        self.write_indent(writer, level)?;
        write!(writer, "\"@context\": {{")?;
        self.nl(writer)?;

        let entries: Vec<_> = self.context.iter().collect();
        for (i, (term, entry)) in entries.iter().enumerate() {
            self.write_indent(writer, level + 1)?;
            write!(writer, "\"")?;
            self.write_json_string(term, writer)?;
            write!(writer, "\": ")?;

            match entry {
                ContextEntry::Iri(iri) => {
                    write!(writer, "\"")?;
                    self.write_json_string(iri, writer)?;
                    write!(writer, "\"")?;
                }
                ContextEntry::Definition { id, value_type } => {
                    write!(writer, "{{\"@id\": \"")?;
                    self.write_json_string(id, writer)?;
                    write!(writer, "\"")?;
                    if let Some(vt) = value_type {
                        write!(writer, ", \"@type\": \"")?;
                        self.write_json_string(vt, writer)?;
                        write!(writer, "\"")?;
                    }
                    write!(writer, "}}")?;
                }
            }

            if i < entries.len() - 1 {
                write!(writer, ",")?;
            }
            self.nl(writer)?;
        }

        self.write_indent(writer, level)?;
        write!(writer, "}}")?;
        Ok(())
    }

    /// Write a value (object of a triple)
    fn write_value<W: Write>(&self, object: &Object, writer: &mut W) -> ExportResult<()> {
        match object {
            Object::Iri(iri) => {
                let compacted = self.compact_iri(iri.as_str());
                write!(writer, "{{\"@id\": \"")?;
                self.write_json_string(&compacted, writer)?;
                write!(writer, "\"}}")?;
            }
            Object::BlankNode(bn) => {
                write!(writer, "{{\"@id\": \"_:{}\"}}", bn.label())?;
            }
            Object::Literal(lit) => {
                if let Some(lang) = lit.language() {
                    write!(writer, "{{\"@value\": \"")?;
                    self.write_json_string(lit.value(), writer)?;
                    write!(writer, "\", \"@language\": \"")?;
                    self.write_json_string(lang, writer)?;
                    write!(writer, "\"}}")?;
                } else {
                    let dt = lit.datatype();
                    let dt_str = dt.as_str();

                    if dt_str == "http://www.w3.org/2001/XMLSchema#string" {
                        // Simple string - just write the value
                        write!(writer, "\"")?;
                        self.write_json_string(lit.value(), writer)?;
                        write!(writer, "\"")?;
                    } else {
                        // Typed literal
                        write!(writer, "{{\"@value\": \"")?;
                        self.write_json_string(lit.value(), writer)?;
                        write!(writer, "\", \"@type\": \"")?;
                        let compacted_dt = self.compact_iri(dt_str);
                        self.write_json_string(&compacted_dt, writer)?;
                        write!(writer, "\"}}")?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Write triples as a JSON-LD graph
    pub fn write_graph<'a, W, I>(&self, triples: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a Triple>,
    {
        // Group triples by subject
        let mut by_subject: HashMap<String, HashMap<String, Vec<&Object>>> = HashMap::new();

        for triple in triples {
            let subject_key = match &triple.subject {
                Subject::Iri(iri) => iri.as_str().to_string(),
                Subject::BlankNode(bn) => format!("_:{}", bn.label()),
            };

            by_subject
                .entry(subject_key)
                .or_default()
                .entry(triple.predicate.as_str().to_string())
                .or_default()
                .push(&triple.object);
        }

        write!(writer, "{{")?;
        self.nl(writer)?;

        // Write context if present
        if !self.context.is_empty() {
            self.write_context(writer, 1)?;
            write!(writer, ",")?;
            self.nl(writer)?;
        }

        // Write @graph array
        self.write_indent(writer, 1)?;
        write!(writer, "\"@graph\": [")?;
        self.nl(writer)?;

        let subjects: Vec<_> = by_subject.iter().collect();
        for (i, (subject_id, predicates)) in subjects.iter().enumerate() {
            self.write_indent(writer, 2)?;
            write!(writer, "{{")?;
            self.nl(writer)?;

            // Write @id
            self.write_indent(writer, 3)?;
            write!(writer, "\"@id\": \"")?;
            let compacted_id = self.compact_iri(subject_id);
            self.write_json_string(&compacted_id, writer)?;
            write!(writer, "\"")?;

            // Write predicates
            for (pred_iri, objects) in predicates.iter() {
                write!(writer, ",")?;
                self.nl(writer)?;
                self.write_indent(writer, 3)?;

                // Compact predicate IRI
                let compacted_pred = self.compact_iri(pred_iri);
                write!(writer, "\"")?;
                self.write_json_string(&compacted_pred, writer)?;
                write!(writer, "\": ")?;

                if objects.len() == 1 {
                    self.write_value(objects[0], writer)?;
                } else {
                    write!(writer, "[")?;
                    for (j, obj) in objects.iter().enumerate() {
                        if j > 0 {
                            write!(writer, ", ")?;
                        }
                        self.write_value(obj, writer)?;
                    }
                    write!(writer, "]")?;
                }
            }

            self.nl(writer)?;
            self.write_indent(writer, 2)?;
            write!(writer, "}}")?;

            if i < subjects.len() - 1 {
                write!(writer, ",")?;
            }
            self.nl(writer)?;
        }

        self.write_indent(writer, 1)?;
        write!(writer, "]")?;
        self.nl(writer)?;

        write!(writer, "}}")?;
        self.nl(writer)?;

        Ok(())
    }
}

impl TripleWriter for JsonLdWriter {
    fn write_triple<W: Write>(&self, triple: &Triple, writer: &mut W) -> ExportResult<()> {
        self.write_graph([triple], writer)
    }

    fn write_triples<'a, W, I>(&self, triples: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a Triple>,
    {
        self.write_graph(triples, writer)
    }
}

/// Convenience function to write triples to JSON-LD format
pub fn write_jsonld<'a, I>(triples: I) -> ExportResult<String>
where
    I: IntoIterator<Item = &'a Triple>,
{
    let mut buf = Vec::new();
    let writer = JsonLdWriter::new();
    writer.write_triples(triples, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{BlankNode, Iri, Literal, Predicate};

    fn make_triple(s: &str, p: &str, o: &str) -> Triple {
        Triple::new(
            Subject::Iri(Iri::new_unchecked(s)),
            Predicate::new_unchecked(p),
            Object::Iri(Iri::new_unchecked(o)),
        )
    }

    #[test]
    fn test_simple_jsonld() {
        let triple = make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        );
        let jsonld = write_jsonld([&triple]).unwrap();
        assert!(jsonld.contains("\"@graph\""));
        assert!(jsonld.contains("\"@id\""));
        assert!(jsonld.contains("http://example.org/s"));
    }

    #[test]
    fn test_with_context() {
        let triple = make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        );

        let mut buf = Vec::new();
        JsonLdWriter::new()
            .with_prefix("ex", "http://example.org/")
            .write_triples([&triple], &mut buf)
            .unwrap();

        let jsonld = String::from_utf8_lossy(&buf);
        assert!(jsonld.contains("\"@context\""));
        assert!(jsonld.contains("\"ex\""));
        assert!(jsonld.contains("ex:s") || jsonld.contains("http://example.org/s"));
    }

    #[test]
    fn test_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/name"),
            Object::Literal(Literal::new("Hello World")),
        );
        let jsonld = write_jsonld([&triple]).unwrap();
        assert!(jsonld.contains("\"Hello World\""));
    }

    #[test]
    fn test_language_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/name"),
            Object::Literal(Literal::with_language("Bonjour", "fr").unwrap()),
        );
        let jsonld = write_jsonld([&triple]).unwrap();
        assert!(jsonld.contains("@value"));
        assert!(jsonld.contains("@language"));
        assert!(jsonld.contains("\"fr\""));
    }

    #[test]
    fn test_typed_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/age"),
            Object::Literal(Literal::with_datatype(
                "42",
                Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
            )),
        );
        let jsonld = write_jsonld([&triple]).unwrap();
        assert!(jsonld.contains("@type"));
        assert!(jsonld.contains("integer"));
    }

    #[test]
    fn test_blank_node() {
        let triple = Triple::new(
            Subject::BlankNode(BlankNode::new("b1")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::BlankNode(BlankNode::new("b2")),
        );
        let jsonld = write_jsonld([&triple]).unwrap();
        assert!(jsonld.contains("_:b1"));
        assert!(jsonld.contains("_:b2"));
    }

    #[test]
    fn test_multiple_objects() {
        let triples = vec![
            make_triple(
                "http://example.org/s",
                "http://example.org/p",
                "http://example.org/o1",
            ),
            make_triple(
                "http://example.org/s",
                "http://example.org/p",
                "http://example.org/o2",
            ),
        ];
        let jsonld = write_jsonld(&triples).unwrap();
        // Multiple objects for same predicate should be in an array
        assert!(jsonld.contains("["));
    }

    #[test]
    fn test_compact_output() {
        let triple = make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        );

        let mut buf = Vec::new();
        JsonLdWriter::new()
            .compact()
            .write_triples([&triple], &mut buf)
            .unwrap();

        let jsonld = String::from_utf8_lossy(&buf);
        // Compact output should not have newlines (except possibly at end)
        assert!(!jsonld.contains("\n  ")); // No indented lines
    }
}
