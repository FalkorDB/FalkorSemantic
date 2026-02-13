//! TriG Parser
//!
//! Streaming parser for the TriG RDF format.
//! TriG extends Turtle with support for named graphs via `GRAPH <iri> { ... }` blocks.

use std::io::BufRead;

use oxiri::Iri;
use rio_api::parser::QuadsParser;
use rio_turtle::TriGParser;

use super::common::{parser_error_to_turtle_error, ParseErrorInfo, QuadParser, RioConverter};
use crate::rdf::Quad;

/// Result of parsing a single quad
pub type ParseQuadResult = std::result::Result<Quad, ParseErrorInfo>;

/// TriG format parser
///
/// Provides streaming parsing of TriG data with error handling.
/// TriG extends Turtle with named graph blocks: `GRAPH <iri> { ... }`.
#[derive(Debug, Default)]
pub struct TriGReader {
    /// Blank node prefix for scoping
    blank_node_prefix: Option<String>,
    /// Base IRI for resolving relative IRIs
    base_iri: Option<Iri<String>>,
}

impl TriGReader {
    /// Create a new TriG reader
    #[must_use]
    pub const fn new() -> Self {
        Self {
            blank_node_prefix: None,
            base_iri: None,
        }
    }

    /// Set a prefix for blank node identifiers
    ///
    /// This is useful when parsing multiple documents to ensure
    /// blank node IDs don't collide.
    pub fn with_blank_node_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.blank_node_prefix = Some(prefix.into());
        self
    }

    /// Set a base IRI for resolving relative IRIs
    pub fn with_base_iri(mut self, base: Iri<String>) -> Self {
        self.base_iri = Some(base);
        self
    }

    /// Get the configured converter
    fn converter(&self) -> RioConverter {
        match &self.blank_node_prefix {
            Some(prefix) => RioConverter::with_blank_node_prefix(prefix.clone()),
            None => RioConverter::new(),
        }
    }

    /// Parse all quads from a reader into a vector
    ///
    /// Returns an error on the first parse failure.
    pub fn parse_all<R: BufRead>(&self, reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
        let mut quads = Vec::new();
        let mut parser = TriGParser::new(reader, self.base_iri.clone());
        let converter = self.converter();

        while !parser.is_end() {
            if let Err(e) =
                parser.parse_step(&mut |rio_quad| match converter.convert_quad(rio_quad) {
                    Ok(quad) => {
                        quads.push(quad);
                        Ok(())
                    }
                    Err(e) => Err(parser_error_to_turtle_error(e)),
                })
            {
                return Err(ParseErrorInfo::new(e.to_string()));
            }
        }

        Ok(quads)
    }

    /// Parse all quads from a string into a vector
    pub fn parse_all_str(&self, input: &str) -> Result<Vec<Quad>, ParseErrorInfo> {
        self.parse_all(input.as_bytes())
    }

    /// Parse TriG from a reader, returning an iterator
    pub fn parse_iter<R: BufRead>(&self, reader: R) -> TriGQuadCollector<R> {
        TriGQuadCollector::new(reader, self.blank_node_prefix.clone(), self.base_iri.clone())
    }

    /// Parse TriG from a string, returning an iterator
    #[must_use]
    pub fn parse_str<'a>(&self, input: &'a str) -> TriGQuadCollector<&'a [u8]> {
        TriGQuadCollector::new(input.as_bytes(), self.blank_node_prefix.clone(), self.base_iri.clone())
    }
}

impl QuadParser for TriGReader {
    fn parse_str(&self, input: &str) -> Result<Vec<Quad>, ParseErrorInfo> {
        self.parse_all_str(input)
    }

    fn parse_read<R: BufRead>(&self, reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
        self.parse_all(reader)
    }
}

/// Collects quads from a TriG parser
pub struct TriGQuadCollector<R: BufRead> {
    parser: TriGParser<R>,
    converter: RioConverter,
    pending: Vec<Quad>,
    finished: bool,
}

impl<R: BufRead> TriGQuadCollector<R> {
    fn new(reader: R, blank_node_prefix: Option<String>, base_iri: Option<Iri<String>>) -> Self {
        let converter = match blank_node_prefix {
            Some(prefix) => RioConverter::with_blank_node_prefix(prefix),
            None => RioConverter::new(),
        };
        Self {
            parser: TriGParser::new(reader, base_iri),
            converter,
            pending: Vec::new(),
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for TriGQuadCollector<R> {
    type Item = ParseQuadResult;

    fn next(&mut self) -> Option<Self::Item> {
        // Return any pending quads first
        if !self.pending.is_empty() {
            return Some(Ok(self.pending.remove(0)));
        }

        if self.finished || self.parser.is_end() {
            return None;
        }

        // Parse the next step
        let pending = &mut self.pending;
        let converter = &self.converter;

        match self
            .parser
            .parse_step(&mut |rio_quad| match converter.convert_quad(rio_quad) {
                Ok(quad) => {
                    pending.push(quad);
                    Ok(())
                }
                Err(e) => Err(parser_error_to_turtle_error(e)),
            }) {
            Ok(()) => {
                if !self.pending.is_empty() {
                    Some(Ok(self.pending.remove(0)))
                } else if self.parser.is_end() {
                    self.finished = true;
                    None
                } else {
                    // Try again
                    self.next()
                }
            }
            Err(e) => {
                self.finished = true;
                Some(Err(ParseErrorInfo::new(e.to_string())))
            }
        }
    }
}

/// Convenience type alias for the iterator
pub type TriGIterator<R> = TriGQuadCollector<R>;

/// Parse TriG from a string
///
/// Convenience function for simple parsing.
pub fn parse_trig(input: &str) -> Result<Vec<Quad>, ParseErrorInfo> {
    TriGReader::new().parse_all_str(input)
}

/// Parse TriG from a reader
pub fn parse_trig_reader<R: BufRead>(reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
    TriGReader::new().parse_all(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::GraphName;

    #[test]
    fn test_parse_named_graph() {
        let input = r#"
            <http://example.org/s> <http://example.org/p> <http://example.org/o> .

            GRAPH <http://example.org/graph1> {
                <http://example.org/s1> <http://example.org/p1> <http://example.org/o1> .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 2);

        // First triple is in default graph
        assert!(quads[0].is_default_graph());

        // Second triple is in named graph
        assert!(!quads[1].is_default_graph());
        match quads[1].graph().unwrap() {
            GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/graph1"),
            _ => panic!("Expected IRI graph name"),
        }
    }

    #[test]
    fn test_parse_with_prefixes() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            ex:s ex:p ex:o .

            GRAPH ex:g1 {
                ex:s1 ex:p1 "value1" .
                ex:s2 ex:p2 "value2" .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 3);
        assert!(quads[0].is_default_graph());
        assert!(!quads[1].is_default_graph());
        assert!(!quads[2].is_default_graph());

        // Both named graph quads should share the same graph
        let g1 = quads[1].graph().unwrap();
        let g2 = quads[2].graph().unwrap();
        match (g1, g2) {
            (GraphName::Iri(iri1), GraphName::Iri(iri2)) => {
                assert_eq!(iri1.as_str(), "http://example.org/g1");
                assert_eq!(iri2.as_str(), "http://example.org/g1");
            }
            _ => panic!("Expected IRI graph names"),
        }
    }

    #[test]
    fn test_parse_multiple_named_graphs() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g1 {
                ex:s1 ex:p "in g1" .
            }

            GRAPH ex:g2 {
                ex:s2 ex:p "in g2" .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 2);

        match quads[0].graph().unwrap() {
            GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/g1"),
            _ => panic!("Expected IRI graph name"),
        }
        match quads[1].graph().unwrap() {
            GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/g2"),
            _ => panic!("Expected IRI graph name"),
        }
    }

    #[test]
    fn test_parse_default_graph_only() {
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:s1 ex:p1 ex:o1 .
            ex:s2 ex:p2 ex:o2 .
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 2);
        assert!(quads[0].is_default_graph());
        assert!(quads[1].is_default_graph());
    }

    #[test]
    fn test_parse_mixed_default_and_named() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            ex:default1 ex:p "default" .

            GRAPH ex:g1 {
                ex:named1 ex:p "named" .
            }

            ex:default2 ex:p "also default" .
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 3);
        assert!(quads[0].is_default_graph());
        assert!(!quads[1].is_default_graph());
        assert!(quads[2].is_default_graph());
    }

    #[test]
    fn test_parse_empty_graph() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:empty {
            }

            ex:s ex:p ex:o .
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 1);
        assert!(quads[0].is_default_graph());
    }

    #[test]
    fn test_parse_blank_node_graph_name() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH _:g {
                ex:s ex:p ex:o .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 1);
        match quads[0].graph().unwrap() {
            GraphName::BlankNode(bn) => assert_eq!(bn.label(), "g"),
            _ => panic!("Expected blank node graph name"),
        }
    }

    #[test]
    fn test_parse_literal_types() {
        let input = r#"
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            GRAPH ex:g {
                ex:s ex:age "42"^^xsd:integer .
                ex:s ex:name "Alice"@en .
                ex:s ex:desc "A person" .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 3);

        let lit0 = quads[0].object().as_literal().unwrap();
        assert_eq!(lit0.value(), "42");
        assert_eq!(
            lit0.datatype().as_str(),
            "http://www.w3.org/2001/XMLSchema#integer"
        );

        let lit1 = quads[1].object().as_literal().unwrap();
        assert_eq!(lit1.value(), "Alice");
        assert_eq!(lit1.language(), Some("en"));

        let lit2 = quads[2].object().as_literal().unwrap();
        assert_eq!(lit2.value(), "A person");
    }

    #[test]
    fn test_parse_blank_nodes_in_triples() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                _:s ex:p _:o .
                _:s ex:name "blank" .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 2);
        assert!(quads[0].subject().is_blank_node());
        assert!(quads[0].object().is_blank_node());
    }

    #[test]
    fn test_parse_predicate_object_list() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                ex:s ex:p1 "v1" ;
                     ex:p2 "v2" ;
                     ex:p3 "v3" .
            }
        "#;
        let quads = parse_trig(input).unwrap();
        assert_eq!(quads.len(), 3);

        // All should be in the same graph
        for quad in &quads {
            match quad.graph().unwrap() {
                GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/g"),
                _ => panic!("Expected IRI graph name"),
            }
        }
    }

    #[test]
    fn test_parse_object_list() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                ex:s ex:p "a", "b", "c" .
            }
        "#;
        let quads = parse_trig(input).unwrap();
        assert_eq!(quads.len(), 3);
    }

    #[test]
    fn test_parse_collections() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                ex:s ex:list ( "a" "b" "c" ) .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        // Collections expand to rdf:first/rdf:rest triples
        assert!(quads.len() > 1);

        // All should be in the same named graph
        for quad in &quads {
            assert!(!quad.is_default_graph());
        }
    }

    #[test]
    fn test_parse_empty_input() {
        let input = "";
        let quads = parse_trig(input).unwrap();
        assert!(quads.is_empty());
    }

    #[test]
    fn test_parse_comments_only() {
        let input = "# Just comments\n# Nothing else\n";
        let quads = parse_trig(input).unwrap();
        assert!(quads.is_empty());
    }

    #[test]
    fn test_parse_error() {
        let input = r#"
            GRAPH <http://example.org/g> {
                <http://example.org/s> <http://example.org/p> "unclosed .
            }
        "#;
        let result = parse_trig(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_streaming_iterator() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            ex:s1 ex:p "default" .

            GRAPH ex:g {
                ex:s2 ex:p "named" .
            }
        "#;

        let reader = TriGReader::new();
        let mut count = 0;

        for result in reader.parse_str(input) {
            assert!(result.is_ok());
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn test_blank_node_prefix() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                _:s ex:p _:o .
            }
        "#;

        let reader = TriGReader::new().with_blank_node_prefix("doc1");
        let quads = reader.parse_all_str(input).unwrap();

        let subject_bn = quads[0].subject().as_blank_node().unwrap();
        assert!(subject_bn.label().starts_with("doc1_"));

        let object_bn = quads[0].object().as_blank_node().unwrap();
        assert!(object_bn.label().starts_with("doc1_"));
    }

    #[test]
    fn test_parse_unicode() {
        let input = r#"
            @prefix ex: <http://example.org/> .

            GRAPH ex:g {
                ex:s ex:p "日本語" .
            }
        "#;
        let quads = parse_trig(input).unwrap();
        let lit = quads[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "日本語");
    }

    #[test]
    fn test_quad_parser_trait() {
        let input = r#"
            @prefix ex: <http://example.org/> .
            GRAPH ex:g { ex:s ex:p ex:o . }
        "#;

        let reader = TriGReader::new();
        let quads = QuadParser::parse_str(&reader, input).unwrap();
        assert_eq!(quads.len(), 1);
        assert!(!quads[0].is_default_graph());
    }

    #[test]
    fn test_full_iri_graph_name() {
        let input = r#"
            GRAPH <http://example.org/my-graph> {
                <http://example.org/s> <http://example.org/p> <http://example.org/o> .
            }
        "#;
        let quads = parse_trig(input).unwrap();

        assert_eq!(quads.len(), 1);
        match quads[0].graph().unwrap() {
            GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/my-graph"),
            _ => panic!("Expected IRI graph name"),
        }
    }
}
