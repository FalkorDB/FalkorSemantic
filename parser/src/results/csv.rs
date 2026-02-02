//! CSV and TSV Results Formats
//!
//! Implements CSV (RFC 4180) and TSV result formats for SPARQL:
//! https://www.w3.org/TR/sparql11-results-csv-tsv/

use super::{AskResult, ResultsResult, ResultsWriter, SelectResults, Term};
use std::io::Write;

/// Writer for CSV Results Format
#[derive(Debug, Clone, Default)]
pub struct CsvResultsWriter;

impl CsvResultsWriter {
    /// Create a new CSV results writer
    pub fn new() -> Self {
        Self
    }

    /// Write a CSV-escaped value
    fn write_value<W: Write>(&self, term: &Term, writer: &mut W) -> ResultsResult<()> {
        let value = term.value();
        let needs_quoting = value.contains(',')
            || value.contains('"')
            || value.contains('\n')
            || value.contains('\r');

        if needs_quoting {
            write!(writer, "\"")?;
            for c in value.chars() {
                if c == '"' {
                    write!(writer, "\"\"")?;
                } else {
                    write!(writer, "{}", c)?;
                }
            }
            write!(writer, "\"")?;
        } else {
            write!(writer, "{}", value)?;
        }
        Ok(())
    }
}

impl ResultsWriter for CsvResultsWriter {
    fn write_select<W: Write>(&self, results: &SelectResults, mut writer: W) -> ResultsResult<()> {
        // Write header row
        for (i, var) in results.variables.iter().enumerate() {
            if i > 0 {
                write!(writer, ",")?;
            }
            // Variable names shouldn't need escaping, but handle it anyway
            if var.contains(',') || var.contains('"') {
                write!(writer, "\"{}\"", var.replace('"', "\"\""))?;
            } else {
                write!(writer, "{}", var)?;
            }
        }
        writeln!(writer)?;

        // Write data rows
        for binding in &results.bindings {
            for (i, var) in results.variables.iter().enumerate() {
                if i > 0 {
                    write!(writer, ",")?;
                }
                if let Some(term) = binding.get(var) {
                    self.write_value(term, &mut writer)?;
                }
                // Unbound variables are empty (no output)
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    fn write_ask<W: Write>(&self, result: &AskResult, mut writer: W) -> ResultsResult<()> {
        // ASK results in CSV: single column "result" with "true" or "false"
        writeln!(writer, "result")?;
        writeln!(writer, "{}", result.result)?;
        Ok(())
    }
}

/// Writer for TSV Results Format
#[derive(Debug, Clone, Default)]
pub struct TsvResultsWriter;

impl TsvResultsWriter {
    /// Create a new TSV results writer
    pub fn new() -> Self {
        Self
    }

    /// Write a TSV-escaped value
    /// In TSV, tabs and newlines in values are escaped as \t and \n
    fn write_value<W: Write>(&self, term: &Term, writer: &mut W) -> ResultsResult<()> {
        // TSV uses SPARQL term syntax
        match term {
            Term::Iri(iri) => {
                write!(writer, "<")?;
                self.write_escaped(iri.as_str(), writer)?;
                write!(writer, ">")?;
            }
            Term::Literal(lit) => {
                write!(writer, "\"")?;
                self.write_escaped(lit.value(), writer)?;
                write!(writer, "\"")?;
                if let Some(lang) = lit.language() {
                    write!(writer, "@{}", lang)?;
                } else if let Some(dt) = lit.explicit_datatype() {
                    let dt_str = dt.as_str();
                    if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, "^^<{}>", dt_str)?;
                    }
                }
            }
            Term::BlankNode(bn) => {
                write!(writer, "_:{}", bn.label())?;
            }
        }
        Ok(())
    }

    fn write_escaped<W: Write>(&self, s: &str, writer: &mut W) -> ResultsResult<()> {
        for c in s.chars() {
            match c {
                '\t' => write!(writer, "\\t")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\\' => write!(writer, "\\\\")?,
                '"' => write!(writer, "\\\"")?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }
}

impl ResultsWriter for TsvResultsWriter {
    fn write_select<W: Write>(&self, results: &SelectResults, mut writer: W) -> ResultsResult<()> {
        // Write header row with ? prefix for variables
        for (i, var) in results.variables.iter().enumerate() {
            if i > 0 {
                write!(writer, "\t")?;
            }
            write!(writer, "?{}", var)?;
        }
        writeln!(writer)?;

        // Write data rows
        for binding in &results.bindings {
            for (i, var) in results.variables.iter().enumerate() {
                if i > 0 {
                    write!(writer, "\t")?;
                }
                if let Some(term) = binding.get(var) {
                    self.write_value(term, &mut writer)?;
                }
                // Unbound variables are empty
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    fn write_ask<W: Write>(&self, result: &AskResult, mut writer: W) -> ResultsResult<()> {
        // ASK results in TSV
        writeln!(writer, "?result")?;
        writeln!(writer, "{}", result.result)?;
        Ok(())
    }
}

/// Convenience function to serialize SELECT results to CSV
pub fn select_to_csv(results: &SelectResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    CsvResultsWriter::new().write_select(results, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Convenience function to serialize SELECT results to TSV
pub fn select_to_tsv(results: &SelectResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    TsvResultsWriter::new().write_select(results, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Convenience function to serialize ASK result to CSV
pub fn ask_to_csv(result: &AskResult) -> ResultsResult<String> {
    let mut buf = Vec::new();
    CsvResultsWriter::new().write_ask(result, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Convenience function to serialize ASK result to TSV
pub fn ask_to_tsv(result: &AskResult) -> ResultsResult<String> {
    let mut buf = Vec::new();
    TsvResultsWriter::new().write_ask(result, &mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Binding;

    #[test]
    fn test_csv_empty_results() {
        let results = SelectResults::with_variables(vec!["s".to_string(), "p".to_string()]);
        let csv = select_to_csv(&results).unwrap();
        assert_eq!(csv, "s,p\n");
    }

    #[test]
    fn test_csv_with_bindings() {
        let mut results = SelectResults::with_variables(vec!["s".to_string(), "o".to_string()]);

        let mut binding = Binding::new();
        binding.insert("s".to_string(), Term::iri("http://example.org/s1"));
        binding.insert("o".to_string(), Term::literal("hello"));
        results.add_binding(binding);

        let csv = select_to_csv(&results).unwrap();
        assert!(csv.contains("s,o\n"));
        assert!(csv.contains("http://example.org/s1,hello\n"));
    }

    #[test]
    fn test_csv_escaping_comma() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::literal("hello,world"));
        results.add_binding(binding);

        let csv = select_to_csv(&results).unwrap();
        assert!(csv.contains("\"hello,world\""));
    }

    #[test]
    fn test_csv_escaping_quote() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::literal("say \"hi\""));
        results.add_binding(binding);

        let csv = select_to_csv(&results).unwrap();
        assert!(csv.contains("\"say \"\"hi\"\"\""));
    }

    #[test]
    fn test_csv_unbound_variable() {
        let mut results = SelectResults::with_variables(vec!["x".to_string(), "y".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::literal("hello"));
        // y is unbound
        results.add_binding(binding);

        let csv = select_to_csv(&results).unwrap();
        assert!(csv.contains("hello,\n"));
    }

    #[test]
    fn test_csv_ask_true() {
        let result = AskResult::new(true);
        let csv = ask_to_csv(&result).unwrap();
        assert_eq!(csv, "result\ntrue\n");
    }

    #[test]
    fn test_tsv_empty_results() {
        let results = SelectResults::with_variables(vec!["s".to_string(), "p".to_string()]);
        let tsv = select_to_tsv(&results).unwrap();
        assert_eq!(tsv, "?s\t?p\n");
    }

    #[test]
    fn test_tsv_with_bindings() {
        let mut results = SelectResults::with_variables(vec!["s".to_string(), "o".to_string()]);

        let mut binding = Binding::new();
        binding.insert("s".to_string(), Term::iri("http://example.org/s1"));
        binding.insert("o".to_string(), Term::literal("hello"));
        results.add_binding(binding);

        let tsv = select_to_tsv(&results).unwrap();
        assert!(tsv.contains("?s\t?o\n"));
        assert!(tsv.contains("<http://example.org/s1>\t\"hello\"\n"));
    }

    #[test]
    fn test_tsv_typed_literal() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert(
            "x".to_string(),
            Term::typed_literal("42", "http://www.w3.org/2001/XMLSchema#integer"),
        );
        results.add_binding(binding);

        let tsv = select_to_tsv(&results).unwrap();
        assert!(tsv.contains("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>"));
    }

    #[test]
    fn test_tsv_lang_literal() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::lang_literal("hello", "en"));
        results.add_binding(binding);

        let tsv = select_to_tsv(&results).unwrap();
        assert!(tsv.contains("\"hello\"@en"));
    }

    #[test]
    fn test_tsv_blank_node() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::blank_node("b1"));
        results.add_binding(binding);

        let tsv = select_to_tsv(&results).unwrap();
        assert!(tsv.contains("_:b1"));
    }

    #[test]
    fn test_tsv_escaping() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::literal("hello\tworld\n"));
        results.add_binding(binding);

        let tsv = select_to_tsv(&results).unwrap();
        assert!(tsv.contains("\\t"));
        assert!(tsv.contains("\\n"));
    }

    #[test]
    fn test_tsv_ask() {
        let result = AskResult::new(false);
        let tsv = ask_to_tsv(&result).unwrap();
        assert_eq!(tsv, "?result\nfalse\n");
    }
}
