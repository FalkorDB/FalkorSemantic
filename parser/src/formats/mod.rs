//! RDF Format Parsers
//!
//! This module provides parsers for various RDF serialization formats.

pub mod common;
pub mod nquads;
pub mod ntriples;
pub mod turtle;

// Common types and traits
pub use common::{ParseErrorInfo, ParserConfig, QuadParser, RioConverter, TripleParser};

// N-Quads parser
pub use nquads::{
    parse_nquads, parse_nquads_reader, NQuadsIterator, NQuadsReader, ParseQuadResult,
    QuadCollector,
};

// N-Triples parser
pub use ntriples::{
    parse_ntriples, parse_ntriples_reader, NTriplesIterator, NTriplesReader, ParseTripleResult,
    TripleCollector,
};
