//! Turtle Serializer
//!
//! Implements the Turtle format as per:
//! https://www.w3.org/TR/turtle/
//!
//! Turtle is a compact, human-readable format for RDF graphs that
//! supports prefixes, subject/predicate grouping, and shorthand notations.

use super::{ExportResult, TripleWriter};
use crate::rdf::{BlankNode, Iri, Literal, Object, Subject, Triple};
use std::collections::HashMap;
use std::io::Write;

/// Writer for Turtle format
#[derive(Debug, Clone)]
pub struct TurtleWriter {
    /// Prefix mappings (prefix -> namespace IRI)
    prefixes: HashMap<String, String>,
    /// Whether to group by subject
    group_by_subject: bool,
    /// Indentation string
    indent: String,
}

impl Default for TurtleWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl TurtleWriter {
    /// Create a new Turtle writer
    pub fn new() -> Self {
        Self {
            prefixes: HashMap::new(),
            group_by_subject: true,
            indent: "    ".to_string(),
        }
    }

    /// Add a prefix mapping
    pub fn with_prefix(mut self, prefix: &str, namespace: &str) -> Self {
        self.prefixes
            .insert(prefix.to_string(), namespace.to_string());
        self
    }

    /// Add common prefixes (rdf, rdfs, xsd, owl)
    pub fn with_common_prefixes(self) -> Self {
        self.with_prefix("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#")
            .with_prefix("rdfs", "http://www.w3.org/2000/01/rdf-schema#")
            .with_prefix("xsd", "http://www.w3.org/2001/XMLSchema#")
            .with_prefix("owl", "http://www.w3.org/2002/07/owl#")
    }

    /// Disable subject grouping (write each triple on its own line)
    pub fn without_grouping(mut self) -> Self {
        self.group_by_subject = false;
        self
    }

    /// Set custom indentation
    pub fn with_indent(mut self, indent: &str) -> Self {
        self.indent = indent.to_string();
        self
    }

    /// Write prefix declarations
    fn write_prefixes<W: Write>(&self, writer: &mut W) -> ExportResult<()> {
        for (prefix, namespace) in &self.prefixes {
            writeln!(writer, "@prefix {}: <{}> .", prefix, namespace)?;
        }
        if !self.prefixes.is_empty() {
            writeln!(writer)?;
        }
        Ok(())
    }

    /// Try to compact an IRI using prefixes
    fn compact_iri(&self, iri: &str) -> Option<String> {
        for (prefix, namespace) in &self.prefixes {
            if iri.starts_with(namespace) {
                let local = &iri[namespace.len()..];
                // Check if local part is a valid PN_LOCAL
                if Self::is_valid_local_name(local) {
                    return Some(format!("{}:{}", prefix, local));
                }
            }
        }
        None
    }

    /// Check if a string is a valid Turtle local name
    fn is_valid_local_name(s: &str) -> bool {
        if s.is_empty() {
            return true; // Empty local name is valid
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();

        // First char must be letter, underscore, or digit
        if !first.is_ascii_alphanumeric() && first != '_' {
            return false;
        }

        // Rest can include dots, hyphens
        for c in chars {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.' {
                return false;
            }
        }

        // Cannot end with '.'
        !s.ends_with('.')
    }

    /// Write an IRI (possibly compacted)
    fn write_iri<W: Write>(&self, iri: &Iri, writer: &mut W) -> ExportResult<()> {
        if let Some(compacted) = self.compact_iri(iri.as_str()) {
            write!(writer, "{}", compacted)?;
        } else {
            write!(writer, "<")?;
            self.write_escaped_iri(iri.as_str(), writer)?;
            write!(writer, ">")?;
        }
        Ok(())
    }

    /// Write an escaped IRI string
    fn write_escaped_iri<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\' => {
                    write!(writer, "\\u{:04X}", c as u32)?;
                }
                ' ' => write!(writer, "%20")?,
                c if c.is_control() => write!(writer, "\\u{:04X}", c as u32)?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    /// Write a blank node
    fn write_blank_node<W: Write>(&self, bn: &BlankNode, writer: &mut W) -> ExportResult<()> {
        write!(writer, "_:{}", bn.label())?;
        Ok(())
    }

    /// Write a literal
    fn write_literal<W: Write>(&self, lit: &Literal, writer: &mut W) -> ExportResult<()> {
        let value = lit.value();

        // Use long string format if contains newlines
        if value.contains('\n') || value.contains('\r') {
            write!(writer, "\"\"\"")?;
            self.write_escaped_long_string(value, writer)?;
            write!(writer, "\"\"\"")?;
        } else {
            write!(writer, "\"")?;
            self.write_escaped_string(value, writer)?;
            write!(writer, "\"")?;
        }

        if let Some(lang) = lit.language() {
            write!(writer, "@{}", lang)?;
        } else {
            let dt = lit.datatype();
            let dt_str = dt.as_str();

            // Use shorthand for common types
            match dt_str {
                "http://www.w3.org/2001/XMLSchema#string" => {}
                "http://www.w3.org/2001/XMLSchema#integer" => {
                    // Could use bare integer, but for now use explicit type
                    write!(writer, "^^")?;
                    self.write_iri(&dt, writer)?;
                }
                "http://www.w3.org/2001/XMLSchema#decimal" => {
                    write!(writer, "^^")?;
                    self.write_iri(&dt, writer)?;
                }
                "http://www.w3.org/2001/XMLSchema#boolean" => {
                    write!(writer, "^^")?;
                    self.write_iri(&dt, writer)?;
                }
                _ => {
                    write!(writer, "^^")?;
                    self.write_iri(&dt, writer)?;
                }
            }
        }
        Ok(())
    }

    /// Write an escaped string
    fn write_escaped_string<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '\\' => write!(writer, "\\\\")?,
                '"' => write!(writer, "\\\"")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c if c.is_control() => write!(writer, "\\u{:04X}", c as u32)?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    /// Write an escaped long string (triple-quoted)
    fn write_escaped_long_string<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '\\' => write!(writer, "\\\\")?,
                '"' => write!(writer, "\\\"")?, // Could also allow unescaped in long strings
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    /// Write a subject
    fn write_subject<W: Write>(&self, subject: &Subject, writer: &mut W) -> ExportResult<()> {
        match subject {
            Subject::Iri(iri) => self.write_iri(iri, writer),
            Subject::BlankNode(bn) => self.write_blank_node(bn, writer),
        }
    }

    /// Write an object
    fn write_object<W: Write>(&self, object: &Object, writer: &mut W) -> ExportResult<()> {
        match object {
            Object::Iri(iri) => self.write_iri(iri, writer),
            Object::BlankNode(bn) => self.write_blank_node(bn, writer),
            Object::Literal(lit) => self.write_literal(lit, writer),
        }
    }

    /// Write triples grouped by subject
    pub fn write_grouped<'a, W, I>(&self, triples: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a Triple>,
    {
        self.write_prefixes(writer)?;

        // Group triples by subject
        let mut by_subject: HashMap<String, Vec<&Triple>> = HashMap::new();
        for triple in triples {
            let key = match &triple.subject {
                Subject::Iri(iri) => format!("iri:{}", iri.as_str()),
                Subject::BlankNode(bn) => format!("bn:{}", bn.label()),
            };
            by_subject.entry(key).or_default().push(triple);
        }

        let mut first_subject = true;
        for triples in by_subject.values() {
            if !first_subject {
                writeln!(writer)?;
            }
            first_subject = false;

            // Group by predicate within subject
            let mut by_predicate: HashMap<String, Vec<&Object>> = HashMap::new();
            for triple in triples {
                let key = triple.predicate.as_str().to_string();
                by_predicate.entry(key).or_default().push(&triple.object);
            }

            // Write subject
            self.write_subject(&triples[0].subject, writer)?;

            let predicates: Vec<_> = by_predicate.iter().collect();
            for (i, (pred_iri, objects)) in predicates.iter().enumerate() {
                if i == 0 {
                    writeln!(writer)?;
                    write!(writer, "{}", self.indent)?;
                } else {
                    writeln!(writer, " ;")?;
                    write!(writer, "{}", self.indent)?;
                }

                // Write predicate
                let pred = Iri::new_unchecked(pred_iri.as_str());

                // Use 'a' shorthand for rdf:type
                if pred_iri.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
                    write!(writer, "a")?;
                } else {
                    self.write_iri(&pred, writer)?;
                }

                // Write objects
                for (j, obj) in objects.iter().enumerate() {
                    if j == 0 {
                        write!(writer, " ")?;
                    } else {
                        write!(writer, ", ")?;
                    }
                    self.write_object(obj, writer)?;
                }
            }

            writeln!(writer, " .")?;
        }

        Ok(())
    }
}

impl TripleWriter for TurtleWriter {
    fn write_triple<W: Write>(&self, triple: &Triple, writer: &mut W) -> ExportResult<()> {
        self.write_subject(&triple.subject, writer)?;
        write!(writer, " ")?;

        // Use 'a' shorthand for rdf:type
        if triple.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            write!(writer, "a")?;
        } else {
            self.write_iri(&triple.predicate, writer)?;
        }

        write!(writer, " ")?;
        self.write_object(&triple.object, writer)?;
        writeln!(writer, " .")?;
        Ok(())
    }

    fn write_triples<'a, W, I>(&self, triples: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a Triple>,
    {
        if self.group_by_subject {
            self.write_grouped(triples, writer)
        } else {
            self.write_prefixes(writer)?;
            for triple in triples {
                self.write_triple(triple, writer)?;
            }
            Ok(())
        }
    }
}

/// Convenience function to write triples to Turtle format
pub fn write_turtle<'a, I>(triples: I) -> ExportResult<String>
where
    I: IntoIterator<Item = &'a Triple>,
{
    let mut buf = Vec::new();
    let writer = TurtleWriter::new();
    writer.write_triples(triples, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::Predicate;

    fn make_triple(s: &str, p: &str, o: &str) -> Triple {
        Triple::new(
            Subject::Iri(Iri::new_unchecked(s)),
            Predicate::new_unchecked(p),
            Object::Iri(Iri::new_unchecked(o)),
        )
    }

    #[test]
    fn test_simple_turtle() {
        let triple = make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains("<http://example.org/s>"));
        assert!(ttl.contains("<http://example.org/p>"));
        assert!(ttl.contains("<http://example.org/o>"));
    }

    #[test]
    fn test_with_prefixes() {
        let triple = make_triple(
            "http://example.org/s",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://example.org/Type",
        );

        let mut buf = Vec::new();
        TurtleWriter::new()
            .with_prefix("ex", "http://example.org/")
            .with_prefix("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#")
            .write_triples([&triple], &mut buf)
            .unwrap();

        let ttl = String::from_utf8_lossy(&buf);
        assert!(ttl.contains("@prefix ex:"));
        assert!(ttl.contains("ex:s"));
        assert!(ttl.contains("a ")); // rdf:type shorthand
        assert!(ttl.contains("ex:Type"));
    }

    #[test]
    fn test_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/name"),
            Object::Literal(Literal::new("Hello World")),
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains("\"Hello World\""));
    }

    #[test]
    fn test_language_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/name"),
            Object::Literal(Literal::with_language("Bonjour", "fr").unwrap()),
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains("\"Bonjour\"@fr"));
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

        let mut buf = Vec::new();
        TurtleWriter::new()
            .with_prefix("xsd", "http://www.w3.org/2001/XMLSchema#")
            .write_triples([&triple], &mut buf)
            .unwrap();

        let ttl = String::from_utf8_lossy(&buf);
        assert!(ttl.contains("\"42\"^^xsd:integer"));
    }

    #[test]
    fn test_blank_node() {
        let triple = Triple::new(
            Subject::BlankNode(BlankNode::new("b1")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::BlankNode(BlankNode::new("b2")),
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains("_:b1"));
        assert!(ttl.contains("_:b2"));
    }

    #[test]
    fn test_multiline_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/text"),
            Object::Literal(Literal::new("Line 1\nLine 2\nLine 3")),
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains("\"\"\""));
    }

    #[test]
    fn test_subject_grouping() {
        let triples = vec![
            make_triple(
                "http://example.org/s",
                "http://example.org/p1",
                "http://example.org/o1",
            ),
            make_triple(
                "http://example.org/s",
                "http://example.org/p2",
                "http://example.org/o2",
            ),
        ];

        let mut buf = Vec::new();
        TurtleWriter::new()
            .with_prefix("ex", "http://example.org/")
            .write_triples(&triples, &mut buf)
            .unwrap();

        let ttl = String::from_utf8_lossy(&buf);
        // Should have semicolon separating predicates
        assert!(ttl.contains(";"));
        // Should have both predicates (order may vary due to HashMap iteration)
        assert!(ttl.contains("ex:p1 ex:o1"));
        assert!(ttl.contains("ex:p2 ex:o2"));
        // One should end with semicolon, one with period
        let has_semicolon_pred = ttl.contains("ex:p1 ex:o1 ;") || ttl.contains("ex:p2 ex:o2 ;");
        let has_period_pred = ttl.contains("ex:p1 ex:o1 .") || ttl.contains("ex:p2 ex:o2 .");
        assert!(
            has_semicolon_pred,
            "Should have a predicate ending with semicolon"
        );
        assert!(
            has_period_pred,
            "Should have a predicate ending with period"
        );
    }

    #[test]
    fn test_rdf_type_shorthand() {
        let triple = make_triple(
            "http://example.org/s",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://example.org/Class",
        );
        let ttl = write_turtle([&triple]).unwrap();
        assert!(ttl.contains(" a "));
    }
}
