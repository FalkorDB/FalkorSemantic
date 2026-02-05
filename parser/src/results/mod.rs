//! SPARQL Query Results Serialization
//!
//! This module provides serializers for SPARQL query results in various formats:
//! - SPARQL JSON Results Format (application/sparql-results+json)
//! - SPARQL XML Results Format (application/sparql-results+xml)
//! - CSV Results Format (text/csv)
//! - TSV Results Format (text/tab-separated-values)
//! - RDF/JSON for CONSTRUCT results
//! - Turtle for CONSTRUCT results

mod binding;
mod csv;
mod json;
mod term;
mod turtle;
mod xml;

pub use binding::{AskResult, Binding, ConstructResults, ResultSet, SelectResults};
pub use csv::{
    ask_to_csv, ask_to_tsv, select_to_csv, select_to_tsv, CsvResultsWriter, TsvResultsWriter,
};
pub use json::{ask_to_json, select_to_json, JsonResultsWriter};
pub use term::Term;
pub use turtle::{
    construct_to_rdf_json, construct_to_turtle, RdfJsonResultsWriter, TurtleResultsWriter,
};
pub use xml::{ask_to_xml, select_to_xml, XmlResultsWriter};

use std::io::Write;
use thiserror::Error;

/// Errors that can occur during result serialization
#[derive(Debug, Error)]
pub enum ResultsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Result type for serialization operations
pub type ResultsResult<T> = std::result::Result<T, ResultsError>;

/// Trait for result set writers
pub trait ResultsWriter {
    /// Write SELECT query results
    fn write_select<W: Write>(&self, results: &SelectResults, writer: W) -> ResultsResult<()>;

    /// Write ASK query result
    fn write_ask<W: Write>(&self, result: &AskResult, writer: W) -> ResultsResult<()>;
}

/// Trait for RDF result writers (CONSTRUCT/DESCRIBE)
pub trait RdfResultsWriter {
    /// Write CONSTRUCT/DESCRIBE query results as RDF
    fn write_rdf<W: Write>(&self, results: &ConstructResults, writer: W) -> ResultsResult<()>;
}

/// Available result formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultFormat {
    /// SPARQL JSON Results (application/sparql-results+json)
    Json,
    /// SPARQL XML Results (application/sparql-results+xml)
    Xml,
    /// CSV (text/csv)
    Csv,
    /// TSV (text/tab-separated-values)
    Tsv,
    /// Turtle (text/turtle) - for CONSTRUCT results
    Turtle,
    /// RDF/JSON (application/rdf+json) - for CONSTRUCT results
    RdfJson,
}

impl ResultFormat {
    /// Get the MIME type for this format
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/sparql-results+json",
            Self::Xml => "application/sparql-results+xml",
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
            Self::Turtle => "text/turtle",
            Self::RdfJson => "application/rdf+json",
        }
    }

    /// Get the file extension for this format
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Turtle => "ttl",
            Self::RdfJson => "rj",
        }
    }
}
