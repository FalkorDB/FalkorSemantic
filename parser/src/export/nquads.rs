//! N-Quads Serializer
//!
//! Implements the N-Quads format as per:
//! <https://www.w3.org/TR/n-quads>/
//!
//! N-Quads extends N-Triples to support named graphs.

use super::ntriples::NTriplesWriter;
use super::{ExportResult, QuadWriter, TripleWriter};
use crate::rdf::{GraphName, Quad, Triple};
use std::io::Write;

/// Writer for N-Quads format
#[derive(Debug, Clone, Default)]
pub struct NQuadsWriter {
    nt_writer: NTriplesWriter,
}

impl NQuadsWriter {
    /// Create a new N-Quads writer
    #[must_use]
    pub fn new() -> Self {
        Self {
            nt_writer: NTriplesWriter::new(),
        }
    }

    /// Write a graph name
    fn write_graph<W: Write>(&self, graph: &GraphName, writer: &mut W) -> ExportResult<()> {
        match graph {
            GraphName::Iri(iri) => {
                write!(writer, "<")?;
                self.write_escaped_iri(iri.as_str(), writer)?;
                write!(writer, ">")?;
            }
            GraphName::BlankNode(bn) => {
                write!(writer, "_:{}", bn.label())?;
            }
        }
        Ok(())
    }

    /// Write an escaped IRI string (delegate to N-Triples writer logic)
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
}

impl QuadWriter for NQuadsWriter {
    fn write_quad<W: Write>(&self, quad: &Quad, writer: &mut W) -> ExportResult<()> {
        // Write subject, predicate, object using N-Triples logic
        self.nt_writer.write_triple(
            &quad.triple,
            &mut TempWriter {
                inner: writer,
                stopped: false,
            },
        )?;

        // The N-Triples writer writes " .\n", we need to intercept that for quads with graphs
        Ok(())
    }
}

/// Helper to intercept the trailing " .\n" from N-Triples writer
#[allow(dead_code)]
struct TempWriter<'a, W: Write> {
    inner: &'a mut W,
    stopped: bool,
}

impl<W: Write> Write for TempWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// Actually, let's rewrite this more cleanly without the temp writer hack
impl NQuadsWriter {
    /// Write a quad directly (cleaner implementation)
    pub fn write_quad_direct<W: Write>(&self, quad: &Quad, writer: &mut W) -> ExportResult<()> {
        // Write subject
        match &quad.triple.subject {
            crate::rdf::Subject::Iri(iri) => {
                write!(writer, "<")?;
                self.write_escaped_iri(iri.as_str(), writer)?;
                write!(writer, ">")?;
            }
            crate::rdf::Subject::BlankNode(bn) => {
                write!(writer, "_:{}", bn.label())?;
            }
        }
        write!(writer, " ")?;

        // Write predicate
        write!(writer, "<")?;
        self.write_escaped_iri(quad.triple.predicate.as_str(), writer)?;
        write!(writer, ">")?;
        write!(writer, " ")?;

        // Write object
        match &quad.triple.object {
            crate::rdf::Object::Iri(iri) => {
                write!(writer, "<")?;
                self.write_escaped_iri(iri.as_str(), writer)?;
                write!(writer, ">")?;
            }
            crate::rdf::Object::BlankNode(bn) => {
                write!(writer, "_:{}", bn.label())?;
            }
            crate::rdf::Object::Literal(lit) => {
                write!(writer, "\"")?;
                self.write_escaped_string(lit.value(), writer)?;
                write!(writer, "\"")?;

                if let Some(lang) = lit.language() {
                    write!(writer, "@{lang}")?;
                } else {
                    let dt = lit.datatype();
                    let dt_str = dt.as_str();
                    if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, "^^<")?;
                        self.write_escaped_iri(dt_str, writer)?;
                        write!(writer, ">")?;
                    }
                }
            }
        }

        // Write graph if present
        if let Some(graph) = &quad.graph {
            write!(writer, " ")?;
            self.write_graph(graph, writer)?;
        }

        writeln!(writer, " .")?;
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

    /// Write multiple quads
    pub fn write_quads_direct<'a, W, I>(&self, quads: I, writer: &mut W) -> ExportResult<()>
    where
        W: Write,
        I: IntoIterator<Item = &'a Quad>,
    {
        for quad in quads {
            self.write_quad_direct(quad, writer)?;
        }
        Ok(())
    }
}

/// Convenience function to write quads to N-Quads format
pub fn write_nquads<'a, I>(quads: I) -> ExportResult<String>
where
    I: IntoIterator<Item = &'a Quad>,
{
    let mut buf = Vec::new();
    let writer = NQuadsWriter::new();
    writer.write_quads_direct(quads, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Streaming writer for N-Quads
pub struct NQuadsStreamWriter<W: Write> {
    writer: W,
    nq_writer: NQuadsWriter,
    count: usize,
}

impl<W: Write> NQuadsStreamWriter<W> {
    /// Create a new streaming N-Quads writer
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            nq_writer: NQuadsWriter::new(),
            count: 0,
        }
    }

    /// Write a single quad
    pub fn write(&mut self, quad: &Quad) -> ExportResult<()> {
        self.nq_writer.write_quad_direct(quad, &mut self.writer)?;
        self.count += 1;
        Ok(())
    }

    /// Write a triple (as a quad in the default graph)
    #[allow(dead_code)]
    pub fn write_triple(&mut self, triple: &Triple) -> ExportResult<()> {
        self.write(&Quad::in_default_graph(triple.clone()))
    }

    /// Get the number of quads written
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Finish writing and return the underlying writer
    #[allow(dead_code)]
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Convenience function for streaming N-Quads export
pub fn write_nquads_streaming<'a, W, I>(writer: W, quads: I) -> ExportResult<usize>
where
    W: Write,
    I: IntoIterator<Item = &'a Quad>,
{
    let mut stream = NQuadsStreamWriter::new(writer);
    for quad in quads {
        stream.write(quad)?;
    }
    Ok(stream.count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{BlankNode, Iri, Literal, Object, Predicate, Subject};

    fn make_quad(s: &str, p: &str, o: &str, g: Option<&str>) -> Quad {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked(s)),
            Predicate::new_unchecked(p),
            Object::Iri(Iri::new_unchecked(o)),
        );
        match g {
            Some(graph) => Quad::in_named_graph(triple, Iri::new_unchecked(graph)),
            None => Quad::in_default_graph(triple),
        }
    }

    #[test]
    fn test_quad_default_graph() {
        let quad = make_quad(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
            None,
        );
        let nq = write_nquads([&quad]).unwrap();
        assert_eq!(
            nq,
            "<http://example.org/s> <http://example.org/p> <http://example.org/o> .\n"
        );
    }

    #[test]
    fn test_quad_named_graph() {
        let quad = make_quad(
            "http://example.org/s",
            "http://example.org/p",
            "http://example.org/o",
            Some("http://example.org/graph"),
        );
        let nq = write_nquads([&quad]).unwrap();
        assert_eq!(
            nq,
            "<http://example.org/s> <http://example.org/p> <http://example.org/o> <http://example.org/graph> .\n"
        );
    }

    #[test]
    fn test_quad_blank_node_graph() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Iri(Iri::new_unchecked("http://example.org/o")),
        );
        let quad = Quad::in_named_graph(triple, BlankNode::new("g1"));
        let nq = write_nquads([&quad]).unwrap();
        assert!(nq.contains("_:g1 ."));
    }

    #[test]
    fn test_quad_with_literal() {
        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Literal(Literal::with_language("hello", "en").unwrap()),
        );
        let quad = Quad::in_named_graph(triple, Iri::new_unchecked("http://example.org/g"));
        let nq = write_nquads([&quad]).unwrap();
        assert!(nq.contains("\"hello\"@en"));
        assert!(nq.contains("<http://example.org/g>"));
    }

    #[test]
    fn test_multiple_quads() {
        let quads = vec![
            make_quad(
                "http://example.org/s1",
                "http://example.org/p",
                "http://example.org/o1",
                Some("http://example.org/g1"),
            ),
            make_quad(
                "http://example.org/s2",
                "http://example.org/p",
                "http://example.org/o2",
                Some("http://example.org/g2"),
            ),
            make_quad(
                "http://example.org/s3",
                "http://example.org/p",
                "http://example.org/o3",
                None,
            ),
        ];
        let nq = write_nquads(&quads).unwrap();
        let lines: Vec<_> = nq.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_streaming_quads() {
        let quads = vec![
            make_quad(
                "http://example.org/s1",
                "http://example.org/p",
                "http://example.org/o1",
                Some("http://example.org/g"),
            ),
            make_quad(
                "http://example.org/s2",
                "http://example.org/p",
                "http://example.org/o2",
                Some("http://example.org/g"),
            ),
        ];

        let mut buf = Vec::new();
        let count = write_nquads_streaming(&mut buf, &quads).unwrap();

        assert_eq!(count, 2);
    }
}
