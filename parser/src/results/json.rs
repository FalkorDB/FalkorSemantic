//! SPARQL JSON Results Format
//!
//! Implements the SPARQL Query Results JSON Format as per:
//! https://www.w3.org/TR/sparql11-results-json/

use super::{AskResult, ResultsError, ResultsResult, ResultsWriter, SelectResults, Term};
use std::io::Write;

/// Writer for SPARQL JSON Results Format
#[derive(Debug, Clone, Default)]
pub struct JsonResultsWriter {
    /// Whether to pretty-print the JSON
    pretty: bool,
}

impl JsonResultsWriter {
    /// Create a new JSON results writer
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable pretty-printing
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Write a term as JSON
    fn write_term<W: Write>(&self, term: &Term, writer: &mut W) -> ResultsResult<()> {
        write!(writer, r#"{{"type":"{}","value":""#, term.term_type())?;
        self.write_json_string(term.value(), writer)?;
        write!(writer, "\"")?;

        // Add datatype or language for literals
        if let Term::Literal(lit) = term {
            if let Some(lang) = lit.language() {
                write!(writer, r#","xml:lang":""#)?;
                self.write_json_string(lang, writer)?;
                write!(writer, "\"")?;
            } else if let Some(dt) = lit.explicit_datatype() {
                // Only include datatype if not xsd:string (default)
                let dt_str = dt.as_str();
                if dt_str != "http://www.w3.org/2001/XMLSchema#string" {
                    write!(writer, r#","datatype":""#)?;
                    self.write_json_string(dt_str, writer)?;
                    write!(writer, "\"")?;
                }
            }
        }

        write!(writer, "}}")?;
        Ok(())
    }

    /// Write a JSON-escaped string
    fn write_json_string<W: Write>(&self, s: &str, writer: &mut W) -> ResultsResult<()> {
        for c in s.chars() {
            match c {
                '"' => write!(writer, "\\\"")?,
                '\\' => write!(writer, "\\\\")?,
                '\n' => write!(writer, "\\n")?,
                '\r' => write!(writer, "\\r")?,
                '\t' => write!(writer, "\\t")?,
                c if c.is_control() => write!(writer, "\\u{:04x}", c as u32)?,
                c => write!(writer, "{}", c)?,
            }
        }
        Ok(())
    }

    fn nl<W: Write>(&self, writer: &mut W) -> ResultsResult<()> {
        if self.pretty {
            writeln!(writer)?;
        }
        Ok(())
    }

    fn indent<W: Write>(&self, writer: &mut W, level: usize) -> ResultsResult<()> {
        if self.pretty {
            for _ in 0..level {
                write!(writer, "  ")?;
            }
        }
        Ok(())
    }
}

impl ResultsWriter for JsonResultsWriter {
    fn write_select<W: Write>(&self, results: &SelectResults, mut writer: W) -> ResultsResult<()> {
        write!(writer, "{{")?;
        self.nl(&mut writer)?;

        // Head section with variables
        self.indent(&mut writer, 1)?;
        write!(writer, r#""head":{{"#)?;
        self.nl(&mut writer)?;
        self.indent(&mut writer, 2)?;
        write!(writer, r#""vars":["#)?;

        for (i, var) in results.variables.iter().enumerate() {
            if i > 0 {
                write!(writer, ",")?;
            }
            write!(writer, "\"")?;
            self.write_json_string(var, &mut writer)?;
            write!(writer, "\"")?;
        }

        write!(writer, "]")?;
        self.nl(&mut writer)?;
        self.indent(&mut writer, 1)?;
        write!(writer, "}},")?;
        self.nl(&mut writer)?;

        // Results section
        self.indent(&mut writer, 1)?;
        write!(writer, r#""results":{{"#)?;
        self.nl(&mut writer)?;
        self.indent(&mut writer, 2)?;
        write!(writer, r#""bindings":["#)?;

        if !results.bindings.is_empty() {
            self.nl(&mut writer)?;
        }

        for (i, binding) in results.bindings.iter().enumerate() {
            if i > 0 {
                write!(writer, ",")?;
                self.nl(&mut writer)?;
            }
            self.indent(&mut writer, 3)?;
            write!(writer, "{{")?;

            let mut first = true;
            for var in &results.variables {
                if let Some(term) = binding.get(var) {
                    if !first {
                        write!(writer, ",")?;
                    }
                    first = false;
                    write!(writer, "\"")?;
                    self.write_json_string(var, &mut writer)?;
                    write!(writer, "\":")?;
                    self.write_term(term, &mut writer)?;
                }
            }

            write!(writer, "}}")?;
        }

        if !results.bindings.is_empty() {
            self.nl(&mut writer)?;
            self.indent(&mut writer, 2)?;
        }
        write!(writer, "]")?;
        self.nl(&mut writer)?;
        self.indent(&mut writer, 1)?;
        write!(writer, "}}")?;
        self.nl(&mut writer)?;

        write!(writer, "}}")?;
        self.nl(&mut writer)?;

        Ok(())
    }

    fn write_ask<W: Write>(&self, result: &AskResult, mut writer: W) -> ResultsResult<()> {
        write!(writer, "{{")?;
        self.nl(&mut writer)?;

        self.indent(&mut writer, 1)?;
        write!(writer, r#""head":{{}},"#)?;
        self.nl(&mut writer)?;

        self.indent(&mut writer, 1)?;
        write!(writer, r#""boolean":{}"#, result.result)?;
        self.nl(&mut writer)?;

        write!(writer, "}}")?;
        self.nl(&mut writer)?;

        Ok(())
    }
}

/// Convenience function to serialize SELECT results to JSON
pub fn select_to_json(results: &SelectResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    JsonResultsWriter::new().write_select(results, &mut buf)?;
    String::from_utf8(buf).map_err(|e| ResultsError::Serialization(e.to_string()))
}

/// Convenience function to serialize ASK result to JSON
pub fn ask_to_json(result: &AskResult) -> ResultsResult<String> {
    let mut buf = Vec::new();
    JsonResultsWriter::new().write_ask(result, &mut buf)?;
    String::from_utf8(buf).map_err(|e| ResultsError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Binding;

    #[test]
    fn test_empty_select_results() {
        let results = SelectResults::with_variables(vec!["s".to_string(), "p".to_string()]);
        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#""vars":["s","p"]"#));
        assert!(json.contains(r#""bindings":[]"#));
    }

    #[test]
    fn test_select_with_bindings() {
        let mut results = SelectResults::with_variables(vec!["s".to_string(), "o".to_string()]);

        let mut binding = Binding::new();
        binding.insert("s".to_string(), Term::iri("http://example.org/s1"));
        binding.insert("o".to_string(), Term::literal("hello"));
        results.add_binding(binding);

        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#""type":"uri""#));
        assert!(json.contains(r#""value":"http://example.org/s1""#));
        assert!(json.contains(r#""type":"literal""#));
        assert!(json.contains(r#""value":"hello""#));
    }

    #[test]
    fn test_select_with_typed_literal() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert(
            "x".to_string(),
            Term::typed_literal("42", "http://www.w3.org/2001/XMLSchema#integer"),
        );
        results.add_binding(binding);

        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#""datatype":"http://www.w3.org/2001/XMLSchema#integer""#));
    }

    #[test]
    fn test_select_with_lang_literal() {
        let mut results = SelectResults::with_variables(vec!["label".to_string()]);

        let mut binding = Binding::new();
        binding.insert("label".to_string(), Term::lang_literal("hello", "en"));
        results.add_binding(binding);

        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#""xml:lang":"en""#));
    }

    #[test]
    fn test_select_with_blank_node() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::blank_node("b1"));
        results.add_binding(binding);

        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#""type":"bnode""#));
        assert!(json.contains(r#""value":"b1""#));
    }

    #[test]
    fn test_ask_true() {
        let result = AskResult::new(true);
        let json = ask_to_json(&result).unwrap();
        assert!(json.contains(r#""boolean":true"#));
    }

    #[test]
    fn test_ask_false() {
        let result = AskResult::new(false);
        let json = ask_to_json(&result).unwrap();
        assert!(json.contains(r#""boolean":false"#));
    }

    #[test]
    fn test_json_escaping() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::literal("hello\n\"world\""));
        results.add_binding(binding);

        let json = select_to_json(&results).unwrap();
        assert!(json.contains(r#"hello\n\"world\""#));
    }

    #[test]
    fn test_pretty_print() {
        let results = SelectResults::with_variables(vec!["x".to_string()]);
        let mut buf = Vec::new();
        JsonResultsWriter::new()
            .pretty()
            .write_select(&results, &mut buf)
            .unwrap();
        let json = String::from_utf8(buf).unwrap();
        assert!(json.contains('\n'));
    }
}
