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
mod json;
mod term;
mod xml;
mod csv;
mod turtle;

pub use binding::{Binding, ResultSet, SelectResults, AskResult, ConstructResults};
pub use term::Term;
pub use json::{JsonResultsWriter, select_to_json, ask_to_json};
pub use xml::{XmlResultsWriter, select_to_xml, ask_to_xml};
pub use csv::{CsvResultsWriter, TsvResultsWriter, select_to_csv, select_to_tsv, ask_to_csv, ask_to_tsv};
pub use turtle::{TurtleResultsWriter, RdfJsonResultsWriter, construct_to_turtle, construct_to_rdf_json};

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
    pub fn mime_type(&self) -> &'static str {
        match self {
            ResultFormat::Json => "application/sparql-results+json",
            ResultFormat::Xml => "application/sparql-results+xml",
            ResultFormat::Csv => "text/csv",
            ResultFormat::Tsv => "text/tab-separated-values",
            ResultFormat::Turtle => "text/turtle",
            ResultFormat::RdfJson => "application/rdf+json",
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ResultFormat::Json => "json",
            ResultFormat::Xml => "xml",
            ResultFormat::Csv => "csv",
            ResultFormat::Tsv => "tsv",
            ResultFormat::Turtle => "ttl",
            ResultFormat::RdfJson => "rj",
        }
    }
}
