//! SPARQL XML Results Format
//!
//! Implements the SPARQL Query Results XML Format as per:
//! <https://www.w3.org/TR/rdf-sparql-XMLres>/

use super::{AskResult, ResultsError, ResultsResult, ResultsWriter, SelectResults, Term};
use std::io::Write;

/// Writer for SPARQL XML Results Format
#[derive(Debug, Clone, Default)]
pub struct XmlResultsWriter {
    /// Whether to pretty-print the XML
    pretty: bool,
}

impl XmlResultsWriter {
    /// Create a new XML results writer
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable pretty-printing
    #[must_use] 
    pub const fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Write XML-escaped text
    fn write_escaped<W: Write>(&self, s: &str, writer: &mut W) -> ResultsResult<()> {
        for c in s.chars() {
            match c {
                '&' => write!(writer, "&amp;")?,
                '<' => write!(writer, "&lt;")?,
                '>' => write!(writer, "&gt;")?,
                '"' => write!(writer, "&quot;")?,
                '\'' => write!(writer, "&apos;")?,
                c => write!(writer, "{c}")?,
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

    /// Write XML header
    fn write_header<W: Write>(&self, writer: &mut W) -> ResultsResult<()> {
        write!(writer, r#"<?xml version="1.0"?>"#)?;
        self.nl(writer)?;
        write!(
            writer,
            r#"<sparql xmlns="http://www.w3.org/2005/sparql-results#">"#
        )?;
        self.nl(writer)?;
        Ok(())
    }

    /// Write XML footer
    fn write_footer<W: Write>(&self, writer: &mut W) -> ResultsResult<()> {
        write!(writer, "</sparql>")?;
        self.nl(writer)?;
        Ok(())
    }

    /// Write a term as XML binding content
    fn write_term<W: Write>(&self, term: &Term, writer: &mut W) -> ResultsResult<()> {
        match term {
            Term::Iri(iri) => {
                write!(writer, "<uri>")?;
                self.write_escaped(iri.as_str(), writer)?;
                write!(writer, "</uri>")?;
            }
            Term::Literal(lit) => {
                if let Some(lang) = lit.language() {
                    write!(writer, r#"<literal xml:lang=""#)?;
                    self.write_escaped(lang, writer)?;
                    write!(writer, r#"">"#)?;
                } else if let Some(dt) = lit.explicit_datatype() {
                    let dt_str = dt.as_str();
                    if dt_str == "http://www.w3.org/2001/XMLSchema#string" {
                        write!(writer, "<literal>")?;
                    } else {
                        write!(writer, r#"<literal datatype=""#)?;
                        self.write_escaped(dt_str, writer)?;
                        write!(writer, r#"">"#)?;
                    }
                } else {
                    write!(writer, "<literal>")?;
                }
                self.write_escaped(lit.value(), writer)?;
                write!(writer, "</literal>")?;
            }
            Term::BlankNode(bn) => {
                write!(writer, "<bnode>")?;
                self.write_escaped(bn.label(), writer)?;
                write!(writer, "</bnode>")?;
            }
        }
        Ok(())
    }
}

impl ResultsWriter for XmlResultsWriter {
    fn write_select<W: Write>(&self, results: &SelectResults, mut writer: W) -> ResultsResult<()> {
        self.write_header(&mut writer)?;

        // Head section
        self.indent(&mut writer, 1)?;
        write!(writer, "<head>")?;
        self.nl(&mut writer)?;

        for var in &results.variables {
            self.indent(&mut writer, 2)?;
            write!(writer, r#"<variable name=""#)?;
            self.write_escaped(var, &mut writer)?;
            write!(writer, r#""/>"#)?;
            self.nl(&mut writer)?;
        }

        self.indent(&mut writer, 1)?;
        write!(writer, "</head>")?;
        self.nl(&mut writer)?;

        // Results section
        self.indent(&mut writer, 1)?;
        write!(writer, "<results>")?;
        self.nl(&mut writer)?;

        for binding in &results.bindings {
            self.indent(&mut writer, 2)?;
            write!(writer, "<result>")?;
            self.nl(&mut writer)?;

            for var in &results.variables {
                if let Some(term) = binding.get(var) {
                    self.indent(&mut writer, 3)?;
                    write!(writer, r#"<binding name=""#)?;
                    self.write_escaped(var, &mut writer)?;
                    write!(writer, r#"">"#)?;
                    self.write_term(term, &mut writer)?;
                    write!(writer, "</binding>")?;
                    self.nl(&mut writer)?;
                }
            }

            self.indent(&mut writer, 2)?;
            write!(writer, "</result>")?;
            self.nl(&mut writer)?;
        }

        self.indent(&mut writer, 1)?;
        write!(writer, "</results>")?;
        self.nl(&mut writer)?;

        self.write_footer(&mut writer)?;
        Ok(())
    }

    fn write_ask<W: Write>(&self, result: &AskResult, mut writer: W) -> ResultsResult<()> {
        self.write_header(&mut writer)?;

        self.indent(&mut writer, 1)?;
        write!(writer, "<head/>")?;
        self.nl(&mut writer)?;

        self.indent(&mut writer, 1)?;
        write!(writer, "<boolean>{}</boolean>", result.result)?;
        self.nl(&mut writer)?;

        self.write_footer(&mut writer)?;
        Ok(())
    }
}

/// Convenience function to serialize SELECT results to XML
pub fn select_to_xml(results: &SelectResults) -> ResultsResult<String> {
    let mut buf = Vec::new();
    XmlResultsWriter::new().write_select(results, &mut buf)?;
    String::from_utf8(buf).map_err(|e| ResultsError::Serialization(e.to_string()))
}

/// Convenience function to serialize ASK result to XML
pub fn ask_to_xml(result: &AskResult) -> ResultsResult<String> {
    let mut buf = Vec::new();
    XmlResultsWriter::new().write_ask(result, &mut buf)?;
    String::from_utf8(buf).map_err(|e| ResultsError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Binding;

    #[test]
    fn test_empty_select_results() {
        let results = SelectResults::with_variables(vec!["s".to_string(), "p".to_string()]);
        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains(r#"<variable name="s"/>"#));
        assert!(xml.contains(r#"<variable name="p"/>"#));
        assert!(xml.contains("<results></results>") || xml.contains("<results>\n</results>"));
    }

    #[test]
    fn test_select_with_bindings() {
        let mut results = SelectResults::with_variables(vec!["s".to_string(), "o".to_string()]);

        let mut binding = Binding::new();
        binding.insert("s".to_string(), Term::iri("http://example.org/s1"));
        binding.insert("o".to_string(), Term::literal("hello"));
        results.add_binding(binding);

        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains("<uri>http://example.org/s1</uri>"));
        assert!(xml.contains("<literal>hello</literal>"));
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

        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains(r#"datatype="http://www.w3.org/2001/XMLSchema#integer""#));
        assert!(xml.contains(">42</literal>"));
    }

    #[test]
    fn test_select_with_lang_literal() {
        let mut results = SelectResults::with_variables(vec!["label".to_string()]);

        let mut binding = Binding::new();
        binding.insert("label".to_string(), Term::lang_literal("hello", "en"));
        results.add_binding(binding);

        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains(r#"xml:lang="en""#));
    }

    #[test]
    fn test_select_with_blank_node() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert("x".to_string(), Term::blank_node("b1"));
        results.add_binding(binding);

        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains("<bnode>b1</bnode>"));
    }

    #[test]
    fn test_ask_true() {
        let result = AskResult::new(true);
        let xml = ask_to_xml(&result).unwrap();
        assert!(xml.contains("<boolean>true</boolean>"));
    }

    #[test]
    fn test_ask_false() {
        let result = AskResult::new(false);
        let xml = ask_to_xml(&result).unwrap();
        assert!(xml.contains("<boolean>false</boolean>"));
    }

    #[test]
    fn test_xml_escaping() {
        let mut results = SelectResults::with_variables(vec!["x".to_string()]);

        let mut binding = Binding::new();
        binding.insert(
            "x".to_string(),
            Term::literal("<script>alert('xss')</script>"),
        );
        results.add_binding(binding);

        let xml = select_to_xml(&results).unwrap();
        assert!(xml.contains("&lt;script&gt;"));
        assert!(!xml.contains("<script>"));
    }

    #[test]
    fn test_xml_header() {
        let results = SelectResults::with_variables(vec!["x".to_string()]);
        let xml = select_to_xml(&results).unwrap();
        assert!(xml.starts_with(r#"<?xml version="1.0"?>"#));
        assert!(xml.contains(r#"xmlns="http://www.w3.org/2005/sparql-results#""#));
    }
}
