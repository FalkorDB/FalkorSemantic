//! N-Quads Parser
//!
//! Streaming parser for the N-Quads RDF format.
//! N-Quads extends N-Triples with support for named graphs.

use std::io::BufRead;

use rio_api::parser::QuadsParser;
use rio_turtle::NQuadsParser;

use crate::rdf::{BlankNode, GraphName, Iri, Literal, Object, Quad, Subject, Triple};
use crate::ParserError;

use super::ntriples::ParseErrorInfo;

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

    /// Parse all quads from a reader into a vector
    ///
    /// Returns an error on the first parse failure.
    pub fn parse_all<R: BufRead>(&self, reader: R) -> Result<Vec<Quad>, ParseErrorInfo> {
        let mut quads = Vec::new();
        let mut parser = NQuadsParser::new(reader);
        let blank_node_prefix = self.blank_node_prefix.as_deref();

        while !parser.is_end() {
            if let Err(e) = parser.parse_step(&mut |rio_quad| {
                match convert_quad(rio_quad, blank_node_prefix) {
                    Ok(quad) => {
                        quads.push(quad);
                        Ok(())
                    }
                    Err(e) => Err(rio_turtle::TurtleError::from(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    ))),
                }
            }) {
                return Err(ParseErrorInfo {
                    message: e.to_string(),
                    line: None,
                    column: None,
                });
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

/// Collects quads from a parser
pub struct QuadCollector<R: BufRead> {
    parser: NQuadsParser<R>,
    blank_node_prefix: Option<String>,
    pending: Vec<Quad>,
    finished: bool,
}

impl<R: BufRead> QuadCollector<R> {
    fn new(reader: R, blank_node_prefix: Option<String>) -> Self {
        Self {
            parser: NQuadsParser::new(reader),
            blank_node_prefix,
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
        let blank_node_prefix = self.blank_node_prefix.as_deref();
        let pending = &mut self.pending;

        match self.parser.parse_step(&mut |rio_quad| {
            match convert_quad(rio_quad, blank_node_prefix) {
                Ok(quad) => {
                    pending.push(quad);
                    Ok(())
                }
                Err(e) => Err(rio_turtle::TurtleError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))),
            }
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
                Some(Err(ParseErrorInfo {
                    message: e.to_string(),
                    line: None,
                    column: None,
                }))
            }
        }
    }
}

/// Convenience type alias for the iterator
pub type NQuadsIterator<R> = QuadCollector<R>;

fn convert_quad(
    rio_quad: rio_api::model::Quad<'_>,
    blank_node_prefix: Option<&str>,
) -> Result<Quad, ParserError> {
    let subject = convert_subject(rio_quad.subject, blank_node_prefix)?;
    let predicate = convert_predicate(rio_quad.predicate)?;
    let object = convert_object(rio_quad.object, blank_node_prefix)?;
    let graph_name = convert_graph_name(rio_quad.graph_name, blank_node_prefix)?;

    let triple = Triple::new(subject, predicate, object);
    Ok(Quad::new(triple, graph_name))
}

fn convert_subject(
    subject: rio_api::model::Subject<'_>,
    blank_node_prefix: Option<&str>,
) -> Result<Subject, ParserError> {
    match subject {
        rio_api::model::Subject::NamedNode(nn) => Ok(Subject::Iri(Iri::new(nn.iri)?)),
        rio_api::model::Subject::BlankNode(bn) => {
            let label = if let Some(prefix) = blank_node_prefix {
                format!("{}_{}", prefix, bn.id)
            } else {
                bn.id.to_string()
            };
            Ok(Subject::BlankNode(BlankNode::new(label)))
        }
        rio_api::model::Subject::Triple(_) => Err(ParserError::ParseError(
            "RDF-star quoted triples not supported".into(),
        )),
    }
}

fn convert_predicate(predicate: rio_api::model::NamedNode<'_>) -> Result<Iri, ParserError> {
    Iri::new(predicate.iri)
}

fn convert_object(
    object: rio_api::model::Term<'_>,
    blank_node_prefix: Option<&str>,
) -> Result<Object, ParserError> {
    match object {
        rio_api::model::Term::NamedNode(nn) => Ok(Object::Iri(Iri::new(nn.iri)?)),
        rio_api::model::Term::BlankNode(bn) => {
            let label = if let Some(prefix) = blank_node_prefix {
                format!("{}_{}", prefix, bn.id)
            } else {
                bn.id.to_string()
            };
            Ok(Object::BlankNode(BlankNode::new(label)))
        }
        rio_api::model::Term::Literal(lit) => {
            let literal = match lit {
                rio_api::model::Literal::Simple { value } => Literal::new(value),
                rio_api::model::Literal::LanguageTaggedString { value, language } => {
                    Literal::with_language(value, language)
                        .map_err(|e| ParserError::ParseError(e.to_string()))?
                }
                rio_api::model::Literal::Typed { value, datatype } => {
                    Literal::with_datatype(value, Iri::new(datatype.iri)?)
                }
            };
            Ok(Object::Literal(literal))
        }
        rio_api::model::Term::Triple(_) => Err(ParserError::ParseError(
            "RDF-star quoted triples not supported".into(),
        )),
    }
}

fn convert_graph_name(
    graph_name: Option<rio_api::model::GraphName<'_>>,
    blank_node_prefix: Option<&str>,
) -> Result<Option<GraphName>, ParserError> {
    match graph_name {
        None => Ok(None),
        Some(rio_api::model::GraphName::NamedNode(nn)) => {
            Ok(Some(GraphName::Iri(Iri::new(nn.iri)?)))
        }
        Some(rio_api::model::GraphName::BlankNode(bn)) => {
            let label = if let Some(prefix) = blank_node_prefix {
                format!("{}_{}", prefix, bn.id)
            } else {
                bn.id.to_string()
            };
            Ok(Some(GraphName::BlankNode(BlankNode::new(label))))
        }
    }
}

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

    #[test]
    fn test_parse_quad_without_graph() {
        let input =
            r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> ."#;
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
        let input = r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> _:g1 ."#;
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
        let input =
            r#"<http://example.org/s> <http://example.org/p> "bonjour"@fr <http://example.org/g> ."#;
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
