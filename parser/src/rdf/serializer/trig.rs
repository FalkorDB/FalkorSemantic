//! TriG Serializer
//!
//! Serializes RDF quads in the W3C TriG format (Turtle with named graphs).
//! https://www.w3.org/TR/trig/

use std::collections::HashMap;
use std::io::Write;

use super::error::SerializerResult;
use super::traits::{QuadSerializer, TripleSerializer};
use super::turtle::TurtleSerializer;
use crate::rdf::{GraphName, Quad};

/// TriG serializer with prefix and named graph support
///
/// Serializes RDF quads in the TriG format, which extends Turtle
/// with support for named graphs using GRAPH blocks.
#[derive(Debug)]
pub struct TriGSerializer {
    /// Namespace prefixes (prefix -> IRI)
    prefixes: HashMap<String, String>,
    /// Whether the header has been written
    header_written: bool,
    /// Current graph being written
    current_graph: Option<String>,
    /// Turtle serializer for triple content
    turtle: TurtleSerializer,
}

impl Default for TriGSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl TriGSerializer {
    /// Create a new TriG serializer
    pub fn new() -> Self {
        Self {
            prefixes: HashMap::new(),
            header_written: false,
            current_graph: None,
            turtle: TurtleSerializer::new(),
        }
    }

    /// Create a TriG serializer with common prefixes
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
        self.turtle.add_prefix(prefix, iri);
    }

    /// Write the header with prefix declarations
    pub fn write_header<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        if self.header_written {
            return Ok(());
        }

        // Write prefix declarations
        let mut prefixes: Vec<_> = self.prefixes.iter().collect();
        prefixes.sort_by_key(|(k, _)| k.as_str());

        for (prefix, iri) in prefixes {
            writeln!(writer, "@prefix {}: <{}> .", prefix, iri)?;
        }

        if !self.prefixes.is_empty() {
            writeln!(writer)?;
        }

        self.header_written = true;
        Ok(())
    }

    /// Get graph name as string for comparison
    fn graph_string(graph: &Option<GraphName>) -> Option<String> {
        graph.as_ref().map(|g| match g {
            GraphName::Iri(iri) => iri.as_str().to_string(),
            GraphName::BlankNode(bn) => format!("_:{}", bn.label()),
        })
    }

    /// Try to compact an IRI using prefixes
    fn compact_iri(&self, iri: &str) -> String {
        for (prefix, namespace) in &self.prefixes {
            if iri.starts_with(namespace) {
                let local = &iri[namespace.len()..];
                if TurtleSerializer::is_valid_local_name(local) {
                    return format!("{}:{}", prefix, local);
                }
            }
        }
        format!("<{}>", iri)
    }

    /// Write graph opening
    fn write_graph_open<W: Write>(&self, graph: &GraphName, writer: &mut W) -> SerializerResult<()> {
        match graph {
            GraphName::Iri(iri) => writeln!(writer, "GRAPH {} {{", self.compact_iri(iri.as_str()))?,
            GraphName::BlankNode(bn) => writeln!(writer, "GRAPH _:{} {{", bn.label())?,
        }
        Ok(())
    }

    /// Close current graph block
    fn close_current_graph<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        if self.current_graph.is_some() {
            // Finish any pending turtle content
            self.turtle.finish(writer)?;
            writeln!(writer, "}}")?;
            writeln!(writer)?;
            self.current_graph = None;
        }
        Ok(())
    }
}

impl QuadSerializer for TriGSerializer {
    fn serialize_quad<W: Write>(&mut self, quad: &Quad, writer: &mut W) -> SerializerResult<()> {
        use super::traits::TripleSerializer;
        
        let graph_str = Self::graph_string(&quad.graph);

        // Check if we need to switch graphs
        if graph_str != self.current_graph {
            self.close_current_graph(writer)?;

            if let Some(ref graph) = quad.graph {
                self.write_graph_open(graph, writer)?;
                self.current_graph = graph_str;
            } else {
                // Default graph - no GRAPH block needed
                self.current_graph = None;
            }
            
            // Reset turtle serializer state for new graph
            self.turtle = TurtleSerializer::new();
            for (prefix, iri) in &self.prefixes {
                self.turtle.add_prefix(prefix, iri);
            }
        }

        // Add indentation for named graphs
        if self.current_graph.is_some() {
            write!(writer, "  ")?;
        }

        self.turtle.serialize_triple(&quad.triple, writer)?;
        
        Ok(())
    }

    fn finish<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        self.close_current_graph(writer)?;
        
        // Finish default graph content
        self.turtle.finish(writer)?;
        
        Ok(())
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
    fn test_default_graph() {
        let mut serializer = TriGSerializer::new();
        let quad = Quad::in_default_graph(Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            Literal::new("value"),
        ));

        let mut output = Vec::new();
        serializer.serialize_quad(&quad, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(!result.contains("GRAPH"), "Default graph should not have GRAPH block");
        assert!(result.contains("<http://example.org/s>"));
    }

    #[test]
    fn test_named_graph() {
        let mut serializer = TriGSerializer::new();
        let quad = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                Literal::new("value"),
            ),
            test_iri("http://example.org/graph1"),
        );

        let mut output = Vec::new();
        serializer.serialize_quad(&quad, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("GRAPH <http://example.org/graph1>"), "Expected GRAPH block");
        assert!(result.contains("{"), "Expected opening brace");
        assert!(result.contains("}"), "Expected closing brace");
    }

    #[test]
    fn test_multiple_graphs() {
        let mut serializer = TriGSerializer::new();
        let quads = vec![
            Quad::in_named_graph(
                Triple::new(
                    test_iri("http://example.org/s1"),
                    test_iri("http://example.org/p"),
                    Literal::new("v1"),
                ),
                test_iri("http://example.org/g1"),
            ),
            Quad::in_named_graph(
                Triple::new(
                    test_iri("http://example.org/s2"),
                    test_iri("http://example.org/p"),
                    Literal::new("v2"),
                ),
                test_iri("http://example.org/g2"),
            ),
        ];

        let mut output = Vec::new();
        for quad in &quads {
            serializer.serialize_quad(quad, &mut output).unwrap();
        }
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("http://example.org/g1"));
        assert!(result.contains("http://example.org/g2"));
        assert_eq!(result.matches("GRAPH").count(), 2);
    }

    #[test]
    fn test_with_prefixes() {
        let mut serializer = TriGSerializer::new();
        serializer.add_prefix("ex", "http://example.org/");
        
        let quad = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                test_iri("http://example.org/o"),
            ),
            test_iri("http://example.org/graph"),
        );

        let mut output = Vec::new();
        serializer.write_header(&mut output).unwrap();
        serializer.serialize_quad(&quad, &mut output).unwrap();
        serializer.finish(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("@prefix ex:"));
        assert!(result.contains("GRAPH ex:graph"));
    }
}
