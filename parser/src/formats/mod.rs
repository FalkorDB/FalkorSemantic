//! RDF Format Parsers
//!
//! This module provides parsers for various RDF serialization formats.

pub mod nquads;
pub mod ntriples;

pub use nquads::{
    parse_nquads, parse_nquads_reader, NQuadsIterator, NQuadsReader, ParseQuadResult,
    QuadCollector,
};
pub use ntriples::{
    parse_ntriples, parse_ntriples_reader, NTriplesIterator, NTriplesReader, ParseErrorInfo,
    ParseTripleResult, TripleCollector,
};
