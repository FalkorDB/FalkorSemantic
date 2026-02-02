//! Turtle and RDF/JSON Results Formats for CONSTRUCT queries
//!
//! Serializes RDF triples from CONSTRUCT/DESCRIBE query results.

use super::{ConstructResults, RdfResultsWriter, ResultsResult};
use crate::rdf::{Object, Subject, Triple};
use std::collections::HashMap;
use std::io::Write;

/// Writer for Turtle format (CONSTRUCT/DESCRIBE results)
#[derive(Debug, Clone, Default)]
pub struct TurtleResultsWriter {
    /// Whether to pretty-print with indentation
    pretty: bool,
}

impl TurtleResultsWriter {
    /// Create a new Turtle results writer
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable pretty-printing
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Write a Turtle-escaped string
    fn write_escaped_string<W: Write>(&self, s: &str, writer: &mut W) -> ResultsResult<()> {
        for c in s.chars() {
            match c {
                '"' => write!(writer, "\\\"")?,
                '\\' => write!(writer, "\\\\")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    /// Write a subject
    fn write_subject<W: Write>(&self, subject: &Subject, writer: &mut W) -> ResultsResult<()> {
        match subject {
            Subject::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            Subject::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
        }
        Ok(())
    }

    /// Write an object
    fn write_object<W: Write>(&self, object: &Object, writer: &mut W) -> ResultsResult<()> {
        match object {
            Object::Iri(iri) => write!(writer, "<{}>", iri.as_str())?,
            Object::BlankNode(bn) => write!(writer, "_:{}", bn.label())?,
            Object::Literal(lit) => {
                write!(writer, "\"")?;
                self.write_escaped_string(lit.value(), writer)?;
                write!(writer, "\"")?;

                if let Some(lang) = lit.language() {
                    write!(writer, "@{}", lang)?;
                } else if let Some(dt) = lit.explicit_datatype() {
                    let dt_str = dt.as_str();
                    // Only include datatype if not xsd:string
                    if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, "^^<{}>", dt_str)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl RdfResultsWriter for TurtleResultsWriter {
    fn write_rdf<W: Write>(&self, results: &ConstructResults, mut writer: W) -> ResultsResult<()> {
        if results.is_empty() {
            return Ok(());
        }

        if self.pretty {
            // Group by subject for nicer output
            let mut by_subject: HashMap<String, Vec<&Triple>> = HashMap::new();
            for triple in &results.triples {
                let key = format!("{}", triple.subject);
                by_subject.entry(key).or_default().push(triple);
            }

            let mut first_subject = true;
            for triples in by_subject.values() {
                if !first_subject {
                    writeln!(writer)?;
                }
                first_subject = false;

                let first = triples[0];
                self.write_subject(&first.subject, &mut writer)?;

                for (i, triple) in triples.iter().enumerate() {
                    if i == 0 {
                        write!(writer, " ")?;
                    } else {
                        write!(writer, " ;\n    ")?;
                    }
                    write!(writer, "<{}> ", triple.predicate.as_str())?;
                    self.write_object(&triple.object, &mut writer)?;
                }
                writeln!(writer, " .")?;
            }
        } else {
            // Simple format: one triple per line
            for triple in &results.triples {
                self.write_subject(&triple.subject, &mut writer)?;
                write!(writer, " <{}> ", triple.predicate.as_str())?;
                self.write_object(&triple.object, &mut writer)?;
                writeln!(writer, " .")?;
            }
        }

        Ok(())
    }
}

/// Writer for RDF/JSON format (CONSTRUCT/DESCRIBE results)
#[derive(Debug, Clone, Default)]
pub struct RdfJsonResultsWriter {
    #[allow(dead_code)]
    pretty: bool,
}

impl RdfJsonResultsWriter {
    /// Create a new RDF/JSON results writer
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable pretty-printing
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    fn write_json_string<W: Write>(&self, s: &str, writer: &mut W) -> ResultsResult<()> {
        for c in s.chars() {
            match c {
                '"' => write!(writer, "\\\"")?,
                '\\' => write!(writer, "\\\\")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c if c.is_control() => write!(writer, "\\u{:04x}", c as u32)?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    fn nl<W: Write>(&self, writer: &mut W) -> ResultsResult<()> {
        if self.pretty {
            writeln!(writer)?;
        }
        Ok(())
    }

    fn indent<W: Write>(&self, writer: &mut W, level: usize) -> ResultsResult<()> {
        if self.pretty {
            for _ in 0..level {
                write!(writer, "  ")?;
            }
        }
        Ok(())
    }

    fn subject_key(&self, subject: &Subject) -> String {
        match subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::BlankNode(bn) => format!("_:{}", bn.label()),
        }
    }

    fn write_object<W: Write>(&self, object: &Object, writer: &mut W) -> ResultsResult<()> {
        write!(writer, "{{")?;
        match object {
            Object::Iri(iri) => {
                write!(writer, r#""type":"uri","value":""#)?;
                self.write_json_string(iri.as_str(), writer)?;
                write!(writer, "\"")?;
            }
            Object::BlankNode(bn) => {
                write!(writer, r#""type":"bnode","value":""#)?;
                self.write_json_string(bn.label(), writer)?;
                write!(writer, "\"")?;
            }
            Object::Literal(lit) => {
                write!(writer, r#""type":"literal","value":""#)?;
                self.write_json_string(lit.value(), writer)?;
                write!(writer, "\"")?;

                if let Some(lang) = lit.language() {
                    write!(writer, r#","lang":""#)?;
                    self.write_json_string(lang, writer)?;
                    write!(writer, "\"")?;
                } else if let Some(dt) = lit.explicit_datatype() {
                    let dt_str = dt.as_str();
                    if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, r#","datatype":""#)?;
                        self.write_json_string(dt_str, writer)?;
                        write!(writer, "\"")?;
                    }
                }
            }
        }
        write!(writer, "}}")?;
        Ok(())
    }
}

impl RdfResultsWriter for RdfJsonResultsWriter {
    fn write_rdf<W: Write>(&self, results: &ConstructResults, mut writer: W) -> ResultsResult<()> {
        // Group triples by subject, then by predicate
        let mut graph: HashMap<String, HashMap<String, Vec<&Object>>> = HashMap::new();

        for triple in &results.triples {
            let subject_key = self.subject_key(&triple.subject);
            let predicate_key = triple.predicate.as_str().to_string();
            graph
                .entry(subject_key)
                .or_default()
                .entry(predicate_key)
                .or_default()
                .push(&triple.object);
        }

        write!(writer, "{{")?;
        self.nl(&mut writer)?;

        let mut first_subject = true;
        for (subject, predicates) in &graph {
            if !first_subject {
                write!(writer, ",")?;
                self.nl(&mut writer)?;
            }
            first_subject = false;

            self.indent(&mut writer, 1)?;
            write!(writer, "\"")?;
            self.write_json_string(subject, &mut writer)?;
            write!(writer, "\":{{")?;
            self.nl(&mut writer)?;

            let mut first_predicate = true;
            for (predicate, objects) in predicates {
                if !first_predicate {
                    write!(writer, ",")?;
                    self.nl(&mut writer)?;
                }
                first_predicate = false;

                self.indent(&mut writer, 2)?;
                write!(writer, "\"")?;
                self.write_json_string(predicate, &mut writer)?;
                write!(writer, "\":[")?;

                for (i, obj) in objects.iter().enumerate() {
                    if i > 0 {
                        write!(writer, ",")?;
                    }
                    self.write_object(obj, &mut writer)?;
                }

                write!(writer, "]")?;
            }

            self.nl(&mut writer)?;
            self.indent(&mut writer, 1)?;
            write!(writer, "}}")?;
        }

        self.nl(&mut writer)?;
        write!(writer, "}}")?;
        self.nl(&mut writer)?;

        Ok(())
    }
}

/// Convenience function to serialize CONSTRUCT results to Turtle
pub fn construct_to_turtle(results: &ConstructResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    TurtleResultsWriter::new().write_rdf(results, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Convenience function to serialize CONSTRUCT results to RDF/JSON
pub fn construct_to_rdf_json(results: &ConstructResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    RdfJsonResultsWriter::new().write_rdf(results, &mut buf)?;
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

    fn make_triple_literal(s: &str, p: &str, lit: &str) -> Triple {
        Triple::new(
            Subject::Iri(Iri::new_unchecked(s)),
            Predicate::new_unchecked(p),
            Object::Literal(Literal::new(lit)),
        )
    }

    #[test]
    fn test_turtle_empty() {
        let results = ConstructResults::new();
        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.is_empty());
    }

    #[test]
    fn test_turtle_single_triple() {
        let mut results = ConstructResults::new();
        results.add_triple(make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("<http://example.org/s>"));
        assert!(ttl.contains("<http://example.org/p>"));
        assert!(ttl.contains("<http://example.org/o>"));
        assert!(ttl.contains(" ."));
    }

    #[test]
    fn test_turtle_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(make_triple_literal(
            "http://example.org/s",
            "http://example.org/p",
            "hello world",
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("\"hello world\""));
    }

    #[test]
    fn test_turtle_typed_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_datatype(
                "42",
                Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
            )),
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
    }

    #[test]
    fn test_turtle_lang_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_language("hello", "en").unwrap()),
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("\"hello\"@en"));
    }

    #[test]
    fn test_turtle_blank_node() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::BlankNode(BlankNode::new("b1")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::BlankNode(BlankNode::new("b2")),
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("_:b1"));
        assert!(ttl.contains("_:b2"));
    }

    #[test]
    fn test_turtle_escaping() {
        let mut results = ConstructResults::new();
        results.add_triple(make_triple_literal(
            "http://example.org/s",
            "http://example.org/p",
            "line1\nline2\ttab",
        ));

        let ttl = construct_to_turtle(&results).unwrap();
        assert!(ttl.contains("\\n"));
        assert!(ttl.contains("\\t"));
    }

    #[test]
    fn test_rdf_json_empty() {
        let results = ConstructResults::new();
        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains("{}"));
    }

    #[test]
    fn test_rdf_json_single_triple() {
        let mut results = ConstructResults::new();
        results.add_triple(make_triple(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
        ));

        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains("\"http://example.org/s\""));
        assert!(json.contains("\"http://example.org/p\""));
        assert!(json.contains(r#""type":"uri""#));
        assert!(json.contains(r#""value":"http://example.org/o""#));
    }

    #[test]
    fn test_rdf_json_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(make_triple_literal(
            "http://example.org/s",
            "http://example.org/p",
            "hello",
        ));

        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains(r#""type":"literal""#));
        assert!(json.contains(r#""value":"hello""#));
    }

    #[test]
    fn test_rdf_json_typed_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_datatype(
                "42",
                Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
            )),
        ));

        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains(r#""datatype":"http://www.w3.org/2001/XMLSchema#integer""#));
    }

    #[test]
    fn test_rdf_json_lang_literal() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_language("hello", "en").unwrap()),
        ));

        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains(r#""lang":"en""#));
    }

    #[test]
    fn test_rdf_json_blank_node() {
        let mut results = ConstructResults::new();
        results.add_triple(Triple::new(
            Subject::BlankNode(BlankNode::new("b1")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::BlankNode(BlankNode::new("b2")),
        ));

        let json = construct_to_rdf_json(&results).unwrap();
        assert!(json.contains("\"_:b1\""));
        assert!(json.contains(r#""type":"bnode""#));
        assert!(json.contains(r#""value":"b2""#));
    }
}
