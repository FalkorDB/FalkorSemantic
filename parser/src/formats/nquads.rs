//! N-Quads Parser
//!
//! Streaming parser for the N-Quads RDF format.
//! N-Quads extends N-Triples with support for named graphs.

use std::io::BufRead;

use rio_api::parser::QuadsParser;
use rio_turtle::NQuadsParser;

use super::common::{parser_error_to_turtle_error, ParseErrorInfo, QuadParser, RioConverter};
use crate::rdf::Quad;

/// Result of parsing a single quad
pub type ParseQuadResult = std::result::Result<Quad, ParseErrorInfo>;

/// N-Quads format parser
///
/// Provides streaming parsing of N-Quads data with error handling.
/// N-Quads extends N-Triples with an optional fourth element specifying
/// the named graph.
#[derive(Debug, Default)]
pub struct NQuadsReader {
    /// Blank node prefix for scoping
    blank_node_prefix: Option<String>,
}

impl NQuadsReader {
    /// Create a new N-Quads reader
    pub fn new() -> Self {
        Self {
            blank_node_prefix: None,
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
        let mut parser = NQuadsParser::new(reader);
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

    /// Parse N-Quads from a reader, returning an iterator
    pub fn parse_iter<R: BufRead>(&self, reader: R) -> QuadCollector<R> {
        QuadCollector::new(reader, self.blank_node_prefix.clone())
    }

    /// Parse N-Quads from a string, returning an iterator
    pub fn parse_str<'a>(&self, input: &'a str) -> QuadCollector<&'a [u8]> {
        QuadCollector::new(input.as_bytes(), self.blank_node_prefix.clone())
    }
}

impl QuadParser for NQuadsReader {
    fn parse_str(&self, input: &str) -> Result<Vec<Quad>, ParseErrorInfo> {
        self.parse_all_str(input)
    }

    fn parse_read<R: BufRead>(&self, reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
        self.parse_all(reader)
    }
}

/// Collects quads from a parser
pub struct QuadCollector<R: BufRead> {
    parser: NQuadsParser<R>,
    converter: RioConverter,
    pending: Vec<Quad>,
    finished: bool,
}

impl<R: BufRead> QuadCollector<R> {
    fn new(reader: R, blank_node_prefix: Option<String>) -> Self {
        let converter = match blank_node_prefix {
            Some(prefix) => RioConverter::with_blank_node_prefix(prefix),
            None => RioConverter::new(),
        };
        Self {
            parser: NQuadsParser::new(reader),
            converter,
            pending: Vec::new(),
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for QuadCollector<R> {
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
pub type NQuadsIterator<R> = QuadCollector<R>;

/// Parse N-Quads from a string
///
/// Convenience function for simple parsing.
pub fn parse_nquads(input: &str) -> Result<Vec<Quad>, ParseErrorInfo> {
    NQuadsReader::new().parse_all_str(input)
}

/// Parse N-Quads from a reader
pub fn parse_nquads_reader<R: BufRead>(reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
    NQuadsReader::new().parse_all(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::GraphName;

    #[test]
    fn test_parse_quad_without_graph() {
        let input = r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        assert!(quads[0].is_default_graph());
        assert_eq!(
            quads[0].subject().as_iri().unwrap().as_str(),
            "http://example.org/s"
        );
    }

    #[test]
    fn test_parse_quad_with_named_graph() {
        let input = r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> <http://example.org/graph> ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        assert!(!quads[0].is_default_graph());

        let graph = quads[0].graph().unwrap();
        match graph {
            GraphName::Iri(iri) => assert_eq!(iri.as_str(), "http://example.org/graph"),
            _ => panic!("Expected IRI graph name"),
        }
    }

    #[test]
    fn test_parse_quad_with_blank_node_graph() {
        let input =
            r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> _:g1 ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        assert!(!quads[0].is_default_graph());

        let graph = quads[0].graph().unwrap();
        match graph {
            GraphName::BlankNode(bn) => assert_eq!(bn.label(), "g1"),
            _ => panic!("Expected blank node graph name"),
        }
    }

    #[test]
    fn test_parse_literal_in_quad() {
        let input =
            r#"<http://example.org/s> <http://example.org/p> "hello" <http://example.org/g> ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        let lit = quads[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "hello");
    }

    #[test]
    fn test_parse_typed_literal_in_quad() {
        let input = r#"<http://example.org/s> <http://example.org/p> "42"^^<http://www.w3.org/2001/XMLSchema#integer> <http://example.org/g> ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        let lit = quads[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "42");
        assert_eq!(
            lit.datatype().as_str(),
            "http://www.w3.org/2001/XMLSchema#integer"
        );
    }

    #[test]
    fn test_parse_language_tagged_literal_in_quad() {
        let input = r#"<http://example.org/s> <http://example.org/p> "bonjour"@fr <http://example.org/g> ."#;
        let quads = parse_nquads(input).unwrap();

        assert_eq!(quads.len(), 1);
        let lit = quads[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "bonjour");
        assert_eq!(lit.language(), Some("fr"));
    }

    #[test]
    fn test_parse_multiple_quads() {
        let input = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> <http://example.org/g1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> <http://example.org/g2> .
<http://example.org/s3> <http://example.org/p> <http://example.org/o3> ."#;

        let quads = parse_nquads(input).unwrap();
        assert_eq!(quads.len(), 3);

        // First two have named graphs
        assert!(!quads[0].is_default_graph());
        assert!(!quads[1].is_default_graph());
        // Third is in default graph
        assert!(quads[2].is_default_graph());
    }

    #[test]
    fn test_parse_mixed_graphs() {
        let input = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> <http://example.org/graph1> .
<http://example.org/s3> <http://example.org/p> <http://example.org/o3> <http://example.org/graph2> .
<http://example.org/s4> <http://example.org/p> <http://example.org/o4> ."#;

        let quads = parse_nquads(input).unwrap();
        assert_eq!(quads.len(), 4);

        // Check graph assignments
        assert!(quads[0].is_default_graph());
        assert!(!quads[1].is_default_graph());
        assert!(!quads[2].is_default_graph());
        assert!(quads[3].is_default_graph());
    }

    #[test]
    fn test_parse_empty_input() {
        let input = "";
        let quads = parse_nquads(input).unwrap();
        assert!(quads.is_empty());
    }

    #[test]
    fn test_parse_comments() {
        let input = r#"# This is a comment
<http://example.org/s> <http://example.org/p> <http://example.org/o> <http://example.org/g> .
# Another comment"#;

        let quads = parse_nquads(input).unwrap();
        assert_eq!(quads.len(), 1);
    }

    #[test]
    fn test_streaming_iterator() {
        let input = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> <http://example.org/g> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> ."#;

        let reader = NQuadsReader::new();
        let mut count = 0;

        for result in reader.parse_str(input) {
            assert!(result.is_ok());
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn test_blank_node_prefix() {
        let input = r#"_:s <http://example.org/p> _:o _:g ."#;

        let reader = NQuadsReader::new().with_blank_node_prefix("doc1");
        let quads = reader.parse_all_str(input).unwrap();

        let subject_bn = quads[0].subject().as_blank_node().unwrap();
        assert!(subject_bn.label().starts_with("doc1_"));

        let object_bn = quads[0].object().as_blank_node().unwrap();
        assert!(object_bn.label().starts_with("doc1_"));

        if let GraphName::BlankNode(bn) = quads[0].graph().unwrap() {
            assert!(bn.label().starts_with("doc1_"));
        } else {
            panic!("Expected blank node graph");
        }
    }

    #[test]
    fn test_parse_error() {
        let input = r#"<http://example.org/s> <http://example.org/p> "unclosed ."#;
        let result = parse_nquads(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unicode() {
        let input =
            r#"<http://example.org/s> <http://example.org/p> "日本語" <http://example.org/g> ."#;
        let quads = parse_nquads(input).unwrap();

        let lit = quads[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "日本語");
    }

    #[test]
    fn test_blank_nodes_in_subject_and_object() {
        let input = r#"_:subject <http://example.org/p> _:object <http://example.org/g> ."#;
        let quads = parse_nquads(input).unwrap();

        assert!(quads[0].subject().is_blank_node());
        assert!(quads[0].object().is_blank_node());
    }
}
