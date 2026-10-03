//! SPARQL Parser Module
//!
//! This module provides SPARQL query parsing using the spargebra crate,
//! with AST wrapper types for internal use.
//!
//! # Supported Query Types
//!
//! - SELECT queries
//! - CONSTRUCT queries
//! - ASK queries
//! - DESCRIBE queries
//!
//! # Example
//!
//! ```
//! use falkorsemantic_parser::sparql::{SparqlParser, Query};
//!
//! let parser = SparqlParser::new();
//!
//! // Parse a SELECT query
//! let query = parser.parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o }").unwrap();
//! assert!(query.is_select());
//!
//! // Parse with a base IRI
//! let query = parser.parse_with_base(
//!     "SELECT * WHERE { <subject> ?p ?o }",
//!     "http://example.org/"
//! ).unwrap();
//! ```

mod ast;
mod error;
mod parser;
mod prefixes;
mod validation;

pub use ast::{
    AskQuery, ConstructQuery, DescribeQuery, Expression, GraphPattern, LiteralPattern, NamedNode,
    OrderCondition, Query, QueryDataset, SelectQuery, TermPattern, TriplePattern, Variable,
};
pub use error::{SparqlError, SparqlErrorKind, SparqlResult};
pub use parser::{parse_sparql, parse_sparql_with_base, QueryType, SparqlParser};
pub use prefixes::PrefixMap;
pub use validation::{QueryValidator, ValidationError};
