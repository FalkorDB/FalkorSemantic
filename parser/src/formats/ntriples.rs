//! N-Triples Parser
//!
//! Streaming parser for the N-Triples RDF format.
//! Uses the rio_turtle crate for parsing.

use std::io::BufRead;

use rio_api::parser::TriplesParser;
use rio_turtle::NTriplesParser;

use super::common::{parser_error_to_turtle_error, ParseErrorInfo, RioConverter, TripleParser};
use crate::rdf::Triple;

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

    /// Get the configured converter
    fn converter(&self) -> RioConverter {
        match &self.blank_node_prefix {
            Some(prefix) => RioConverter::with_blank_node_prefix(prefix.clone()),
            None => RioConverter::new(),
        }
    }

    /// Parse all triples from a reader into a vector
    ///
    /// Returns an error on the first parse failure.
    pub fn parse_all<R: BufRead>(&self, reader: R) -> Result<Vec<Triple>, ParseErrorInfo> {
        let mut triples = Vec::new();
        let mut parser = NTriplesParser::new(reader);
        let converter = self.converter();

        while !parser.is_end() {
            if let Err(e) = parser.parse_step(&mut |rio_triple| match converter
                .convert_triple(rio_triple)
            {
                Ok(triple) => {
                    triples.push(triple);
                    Ok(())
                }
                Err(e) => Err(parser_error_to_turtle_error(e)),
            }) {
                return Err(ParseErrorInfo::new(e.to_string()));
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

impl TripleParser for NTriplesReader {
    fn parse_str(&self, input: &str) -> Result<Vec<Triple>, ParseErrorInfo> {
        self.parse_all_str(input)
    }

    fn parse_read<R: BufRead>(&self, reader: R) -> Result<Vec<Triple>, ParseErrorInfo> {
        self.parse_all(reader)
    }
}

/// Collects triples from a parser
pub struct TripleCollector<R: BufRead> {
    parser: NTriplesParser<R>,
    converter: RioConverter,
    pending: Vec<Triple>,
    finished: bool,
}

impl<R: BufRead> TripleCollector<R> {
    fn new(reader: R, blank_node_prefix: Option<String>) -> Self {
        let converter = match blank_node_prefix {
            Some(prefix) => RioConverter::with_blank_node_prefix(prefix),
            None => RioConverter::new(),
        };
        Self {
            parser: NTriplesParser::new(reader),
            converter,
            pending: Vec::new(),
            finished: false,
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

        if self.finished || self.parser.is_end() {
            return None;
        }

        // Parse the next step
        let pending = &mut self.pending;
        let converter = &self.converter;

        match self.parser.parse_step(
            &mut |rio_triple| match converter.convert_triple(rio_triple) {
                Ok(triple) => {
                    pending.push(triple);
                    Ok(())
                }
                Err(e) => Err(parser_error_to_turtle_error(e)),
            },
        ) {
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
pub type NTriplesIterator<'a, R> = TripleCollector<R>;

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
