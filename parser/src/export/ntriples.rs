//! N-Triples Serializer
//!
//! Implements the N-Triples format as per:
//! <https://www.w3.org/TR/n-triples>/
//!
//! N-Triples is a line-based, plain text format for encoding RDF graphs.

use super::{ExportResult, TripleWriter};
use crate::rdf::{BlankNode, Iri, Literal, Object, Subject, Triple};
use std::io::Write;

/// Writer for N-Triples format
#[derive(Debug, Clone, Default)]
pub struct NTriplesWriter;

impl NTriplesWriter {
    /// Create a new N-Triples writer
    #[must_use] 
    pub const fn new() -> Self {
        Self
    }

    /// Write an IRI in N-Triples format
    fn write_iri<W: Write>(&self, iri: &Iri, writer: &mut W) -> ExportResult<()> {
        write!(writer, "<")?;
        self.write_escaped_iri(iri.as_str(), writer)?;
        write!(writer, ">")?;
        Ok(())
    }

    /// Write an escaped IRI string
    fn write_escaped_iri<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '<' => write!(writer, "\\u003C")?,
                '>' => write!(writer, "\\u003E")?,
                '"' => write!(writer, "\\u0022")?,
                '{' => write!(writer, "\\u007B")?,
                '}' => write!(writer, "\\u007D")?,
                '|' => write!(writer, "\\u007C")?,
                '^' => write!(writer, "\\u005E")?,
                '`' => write!(writer, "\\u0060")?,
                '\\' => write!(writer, "\\u005C")?,
                ' ' => write!(writer, "\\u0020")?,
                c if c.is_control() => write!(writer, "\\u{:04X}", c as u32)?,
                c => write!(writer, "{c}")?,
            }
        }
        Ok(())
    }

    /// Write a blank node in N-Triples format
    fn write_blank_node<W: Write>(&self, bn: &BlankNode, writer: &mut W) -> ExportResult<()> {
        write!(writer, "_:{}", bn.label())?;
        Ok(())
    }

    /// Write a literal in N-Triples format
    fn write_literal<W: Write>(&self, lit: &Literal, writer: &mut W) -> ExportResult<()> {
        write!(writer, "\"")?;
        self.write_escaped_string(lit.value(), writer)?;
        write!(writer, "\"")?;

        if let Some(lang) = lit.language() {
            write!(writer, "@{lang}")?;
        } else {
            let dt = lit.datatype();
            let dt_str = dt.as_str();
            // Always write datatype for non-xsd:string literals
            if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                write!(writer, "^^<")?;
                self.write_escaped_iri(dt_str, writer)?;
                write!(writer, ">")?;
            }
        }
        Ok(())
    }

    /// Write an escaped string (N-Triples string escaping)
    fn write_escaped_string<W: Write>(&self, s: &str, writer: &mut W) -> ExportResult<()> {
        for c in s.chars() {
            match c {
                '\\' => write!(writer, "\\\\")?,
                '"' => write!(writer, "\\\"")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c if c.is_control() => {
                    if (c as u32) <= 0xFFFF {
                        write!(writer, "\\u{:04X}", c as u32)?;
                    } else {
                        write!(writer, "\\U{:08X}", c as u32)?;
                    }
                }
                c => write!(writer, "{c}")?,
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
}

impl TripleWriter for NTriplesWriter {
    fn write_triple<W: Write>(&self, triple: &Triple, writer: &mut W) -> ExportResult<()> {
        self.write_subject(&triple.subject, writer)?;
        write!(writer, " ")?;
        self.write_iri(&triple.predicate, writer)?;
        write!(writer, " ")?;
        self.write_object(&triple.object, writer)?;
        writeln!(writer, " .")?;
        Ok(())
    }
}

/// Convenience function to write triples to N-Triples format
pub fn write_ntriples<'a, I>(triples: I) -> ExportResult<String>
where
    I: IntoIterator<Item = &'a Triple>,
{
    let mut buf = Vec::new();
    let writer = NTriplesWriter::new();
    writer.write_triples(triples, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Streaming writer for N-Triples
///
/// Writes triples one at a time for memory-efficient export of large graphs.
pub struct NTriplesStreamWriter<W: Write> {
    writer: W,
    nt_writer: NTriplesWriter,
    count: usize,
}

impl<W: Write> NTriplesStreamWriter<W> {
    /// Create a new streaming N-Triples writer
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            nt_writer: NTriplesWriter::new(),
            count: 0,
        }
    }

    /// Write a single triple
    pub fn write(&mut self, triple: &Triple) -> ExportResult<()> {
        self.nt_writer.write_triple(triple, &mut self.writer)?;
        self.count += 1;
        Ok(())
    }

    /// Get the number of triples written
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Finish writing and return the underlying writer
    #[allow(dead_code)]
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Convenience function for streaming N-Triples export
pub fn write_ntriples_streaming<'a, W, I>(writer: W, triples: I) -> ExportResult<usize>
where
    W: Write,
    I: IntoIterator<Item = &'a Triple>,
{
    let mut stream = NTriplesStreamWriter::new(writer);
    for triple in triples {
        stream.write(triple)?;
    }
    Ok(stream.count())
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
    fn test_simple_triple() {
        let triple = make_triple(
            "http://example.org/subject",
            "http://example.org/predicate",
            "http://example.org/object",
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert_eq!(
            nt,
            "<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> .\n"
        );
    }

    #[test]
    fn test_literal_object() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::new("hello world")),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.contains("\"hello world\""));
    }

    #[test]
    fn test_typed_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_datatype(
                "42",
                Iri::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
            )),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.contains("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
    }

    #[test]
    fn test_language_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_language("hello", "en").unwrap()),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.contains("\"hello\"@en"));
    }

    #[test]
    fn test_blank_node_subject() {
        let triple = Triple::new(
            Subject::BlankNode(BlankNode::new("b1")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Iri(Iri::new_unchecked("http://example.org/o")),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.starts_with("_:b1 "));
    }

    #[test]
    fn test_blank_node_object() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::BlankNode(BlankNode::new("b2")),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.contains("_:b2 ."));
    }

    #[test]
    fn test_escape_string() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::new("line1\nline2\ttab\"quote")),
        );
        let nt = write_ntriples([&triple]).unwrap();
        assert!(nt.contains("\\n"));
        assert!(nt.contains("\\t"));
        assert!(nt.contains("\\\""));
    }

    #[test]
    fn test_multiple_triples() {
        let triples = vec![
            make_triple(
                "http://example.org/s1",
                "http://example.org/p",
                "http://example.org/o1",
            ),
            make_triple(
                "http://example.org/s2",
                "http://example.org/p",
                "http://example.org/o2",
            ),
        ];
        let nt = write_ntriples(&triples).unwrap();
        let lines: Vec<_> = nt.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_streaming_writer() {
        let triples = vec![
            make_triple(
                "http://example.org/s1",
                "http://example.org/p",
                "http://example.org/o1",
            ),
            make_triple(
                "http://example.org/s2",
                "http://example.org/p",
                "http://example.org/o2",
            ),
            make_triple(
                "http://example.org/s3",
                "http://example.org/p",
                "http://example.org/o3",
            ),
        ];

        let mut buf = Vec::new();
        let count = write_ntriples_streaming(&mut buf, &triples).unwrap();

        assert_eq!(count, 3);
        let output = String::from_utf8_lossy(&buf);
        assert_eq!(output.lines().count(), 3);
    }
}
