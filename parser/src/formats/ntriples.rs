//! N-Triples Parser
//!
//! Streaming parser for the N-Triples RDF format.
//! Uses the rio_turtle crate for parsing.

use std::io::BufRead;

use rio_api::parser::TriplesParser;
use rio_turtle::NTriplesParser;

use crate::rdf::{BlankNode, Iri, Literal, Object, Subject, Triple};
use crate::ParserError;

/// Error information with location details
#[derive(Debug, Clone)]
pub struct ParseErrorInfo {
    /// The error message
    pub message: String,
    /// Line number where the error occurred (1-indexed)
    pub line: Option<u64>,
    /// Column number where the error occurred (1-indexed)
    pub column: Option<u64>,
}

impl std::fmt::Display for ParseErrorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => {
                write!(f, "{}:{}: {}", line, col, self.message)
            }
            (Some(line), None) => {
                write!(f, "line {}: {}", line, self.message)
            }
            _ => write!(f, "{}", self.message),
        }
    }
}

/// Result of parsing a single triple
pub type ParseTripleResult = std::result::Result<Triple, ParseErrorInfo>;

/// N-Triples format parser
///
/// Provides streaming parsing of N-Triples data with error handling
/// that includes line numbers.
#[derive(Debug, Default)]
pub struct NTriplesReader {
    /// Blank node prefix for scoping
    blank_node_prefix: Option<String>,
}

impl NTriplesReader {
    /// Create a new N-Triples reader
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

    /// Parse all triples from a reader into a vector
    ///
    /// Returns an error on the first parse failure.
    pub fn parse_all<R: BufRead>(&self, reader: R) -> Result<Vec<Triple>, ParseErrorInfo> {
        let mut triples = Vec::new();
        let mut parser = NTriplesParser::new(reader);
        let blank_node_prefix = self.blank_node_prefix.as_deref();

        while !parser.is_end() {
            if let Err(e) = parser.parse_step(&mut |rio_triple| {
                match convert_triple(rio_triple, blank_node_prefix) {
                    Ok(triple) => {
                        triples.push(triple);
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

        Ok(triples)
    }

    /// Parse all triples from a string into a vector
    pub fn parse_all_str(&self, input: &str) -> Result<Vec<Triple>, ParseErrorInfo> {
        self.parse_all(input.as_bytes())
    }

    /// Parse N-Triples from a reader, collecting into a vector
    pub fn parse_iter<R: BufRead>(&self, reader: R) -> TripleCollector<R> {
        TripleCollector::new(reader, self.blank_node_prefix.clone())
    }

    /// Parse N-Triples from a string
    pub fn parse_str<'a>(&self, input: &'a str) -> TripleCollector<&'a [u8]> {
        TripleCollector::new(input.as_bytes(), self.blank_node_prefix.clone())
    }
}

/// Collects triples from a parser
pub struct TripleCollector<R: BufRead> {
    parser: NTriplesParser<R>,
    blank_node_prefix: Option<String>,
    pending: Vec<Triple>,
    finished: bool,
    error: Option<ParseErrorInfo>,
}

impl<R: BufRead> TripleCollector<R> {
    fn new(reader: R, blank_node_prefix: Option<String>) -> Self {
        Self {
            parser: NTriplesParser::new(reader),
            blank_node_prefix,
            pending: Vec::new(),
            finished: false,
            error: None,
        }
    }
}

impl<R: BufRead> Iterator for TripleCollector<R> {
    type Item = ParseTripleResult;

    fn next(&mut self) -> Option<Self::Item> {
        // Return any pending triples first
        if !self.pending.is_empty() {
            return Some(Ok(self.pending.remove(0)));
        }

        // Return error if we have one
        if let Some(err) = self.error.take() {
            self.finished = true;
            return Some(Err(err));
        }

        if self.finished || self.parser.is_end() {
            return None;
        }

        // Parse the next step
        let blank_node_prefix = self.blank_node_prefix.as_deref();
        let pending = &mut self.pending;

        match self.parser.parse_step(&mut |rio_triple| {
            match convert_triple(rio_triple, blank_node_prefix) {
                Ok(triple) => {
                    pending.push(triple);
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
pub type NTriplesIterator<'a, R> = TripleCollector<R>;

fn convert_triple(
    rio_triple: rio_api::model::Triple<'_>,
    blank_node_prefix: Option<&str>,
) -> Result<Triple, ParserError> {
    let subject = convert_subject(rio_triple.subject, blank_node_prefix)?;
    let predicate = convert_predicate(rio_triple.predicate)?;
    let object = convert_object(rio_triple.object, blank_node_prefix)?;
    Ok(Triple::new(subject, predicate, object))
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

/// Parse N-Triples from a string
///
/// Convenience function for simple parsing.
pub fn parse_ntriples(input: &str) -> Result<Vec<Triple>, ParseErrorInfo> {
    NTriplesReader::new().parse_all_str(input)
}

/// Parse N-Triples from a reader
pub fn parse_ntriples_reader<R: BufRead>(reader: R) -> Result<Vec<Triple>, ParseErrorInfo> {
    NTriplesReader::new().parse_all(reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_triple() {
        let input = r#"<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> ."#;
        let triples = parse_ntriples(input).unwrap();

        assert_eq!(triples.len(), 1);
        let triple = &triples[0];

        assert!(triple.subject.is_iri());
        assert_eq!(
            triple.subject.as_iri().unwrap().as_str(),
            "http://example.org/subject"
        );
        assert_eq!(triple.predicate.as_str(), "http://example.org/predicate");
        assert!(triple.object.is_iri());
    }

    #[test]
    fn test_parse_literal_object() {
        let input = r#"<http://example.org/s> <http://example.org/p> "hello world" ."#;
        let triples = parse_ntriples(input).unwrap();

        assert_eq!(triples.len(), 1);
        let triple = &triples[0];

        assert!(triple.object.is_literal());
        let lit = triple.object.as_literal().unwrap();
        assert_eq!(lit.value(), "hello world");
    }

    #[test]
    fn test_parse_typed_literal() {
        let input = r#"<http://example.org/s> <http://example.org/p> "42"^^<http://www.w3.org/2001/XMLSchema#integer> ."#;
        let triples = parse_ntriples(input).unwrap();

        assert_eq!(triples.len(), 1);
        let lit = triples[0].object.as_literal().unwrap();
        assert_eq!(lit.value(), "42");
        assert_eq!(
            lit.datatype().as_str(),
            "http://www.w3.org/2001/XMLSchema#integer"
        );
    }

    #[test]
    fn test_parse_language_tagged_literal() {
        let input = r#"<http://example.org/s> <http://example.org/p> "bonjour"@fr ."#;
        let triples = parse_ntriples(input).unwrap();

        assert_eq!(triples.len(), 1);
        let lit = triples[0].object.as_literal().unwrap();
        assert_eq!(lit.value(), "bonjour");
        assert_eq!(lit.language(), Some("fr"));
    }

    #[test]
    fn test_parse_blank_node() {
        let input = r#"_:b1 <http://example.org/p> _:b2 ."#;
        let triples = parse_ntriples(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert!(triples[0].subject.is_blank_node());
        assert!(triples[0].object.is_blank_node());
    }

    #[test]
    fn test_parse_multiple_triples() {
        let input = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> .
<http://example.org/s3> <http://example.org/p> "value" ."#;

        let triples = parse_ntriples(input).unwrap();
        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn test_parse_empty_input() {
        let input = "";
        let triples = parse_ntriples(input).unwrap();
        assert!(triples.is_empty());
    }

    #[test]
    fn test_parse_comments() {
        let input = r#"# This is a comment
<http://example.org/s> <http://example.org/p> <http://example.org/o> .
# Another comment"#;

        let triples = parse_ntriples(input).unwrap();
        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn test_parse_escaped_string() {
        let input = r#"<http://example.org/s> <http://example.org/p> "line1\nline2\ttab" ."#;
        let triples = parse_ntriples(input).unwrap();

        let lit = triples[0].object.as_literal().unwrap();
        assert_eq!(lit.value(), "line1\nline2\ttab");
    }

    #[test]
    fn test_streaming_iterator() {
        let input = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> ."#;

        let reader = NTriplesReader::new();
        let mut count = 0;

        for result in reader.parse_str(input) {
            assert!(result.is_ok());
            count += 1;
        }

        assert_eq!(count, 2);
    }

    #[test]
    fn test_blank_node_prefix() {
        let input = r#"_:node1 <http://example.org/p> _:node2 ."#;

        let reader = NTriplesReader::new().with_blank_node_prefix("doc1");
        let triples = reader.parse_all_str(input).unwrap();

        let subject_bn = triples[0].subject.as_blank_node().unwrap();
        assert!(subject_bn.label().starts_with("doc1_"));
    }

    #[test]
    fn test_parse_error() {
        let input = r#"<http://example.org/s> <http://example.org/p> "unclosed string ."#;
        let result = parse_ntriples(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unicode() {
        let input = r#"<http://example.org/s> <http://example.org/p> "日本語テスト" ."#;
        let triples = parse_ntriples(input).unwrap();

        let lit = triples[0].object.as_literal().unwrap();
        assert_eq!(lit.value(), "日本語テスト");
    }

    #[test]
    fn test_real_world_example() {
        let input = r#"<http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .
<http://www.w3.org/1999/02/22-rdf-syntax-ns#subject> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .
<http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> .
<http://www.w3.org/1999/02/22-rdf-syntax-ns#object> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Property> ."#;

        let triples = parse_ntriples(input).unwrap();
        assert_eq!(triples.len(), 4);

        // All subjects should be rdf namespace IRIs
        for triple in &triples {
            let subject_iri = triple.subject.as_iri().unwrap();
            assert!(subject_iri
                .as_str()
                .starts_with("http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
        }
    }
}
