//! RDF Literal implementation
//!
//! Literals represent values in RDF (strings, numbers, dates, etc.)
//! with optional datatype IRI and language tag.

use std::fmt;

use super::Iri;
use crate::ParserError;

/// Well-known XSD datatype IRIs
pub mod xsd {
    use super::Iri;

    /// XSD namespace
    pub const NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema#";

    /// Create an XSD datatype IRI
    pub fn datatype(local_name: &str) -> Iri {
        Iri::new_unchecked(format!("{}{}", NAMESPACE, local_name))
    }

    /// xsd:string
    pub fn string() -> Iri {
        datatype("string")
    }

    /// xsd:boolean
    pub fn boolean() -> Iri {
        datatype("boolean")
    }

    /// xsd:integer
    pub fn integer() -> Iri {
        datatype("integer")
    }

    /// xsd:decimal
    pub fn decimal() -> Iri {
        datatype("decimal")
    }

    /// xsd:double
    pub fn double() -> Iri {
        datatype("double")
    }

    /// xsd:float
    pub fn float() -> Iri {
        datatype("float")
    }

    /// xsd:date
    pub fn date() -> Iri {
        datatype("date")
    }

    /// xsd:dateTime
    pub fn date_time() -> Iri {
        datatype("dateTime")
    }

    /// xsd:time
    pub fn time() -> Iri {
        datatype("time")
    }

    /// xsd:anyURI
    pub fn any_uri() -> Iri {
        datatype("anyURI")
    }
}

/// RDF langString datatype
pub const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// An RDF Literal value
///
/// A literal has a lexical value, an optional datatype, and an optional language tag.
/// If a language tag is present, the datatype is implicitly rdf:langString.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Literal {
    /// The lexical value
    value: String,
    /// The datatype IRI (defaults to xsd:string if not specified)
    datatype: Option<Iri>,
    /// The language tag (e.g., "en", "en-US")
    language: Option<String>,
}

impl Literal {
    /// Create a new plain literal (xsd:string)
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            datatype: None,
            language: None,
        }
    }

    /// Create a typed literal
    pub fn with_datatype(value: impl Into<String>, datatype: Iri) -> Self {
        Self {
            value: value.into(),
            datatype: Some(datatype),
            language: None,
        }
    }

    /// Create a language-tagged literal
    pub fn with_language(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Result<Self, ParserError> {
        let language = language.into();
        Self::validate_language_tag(&language)?;
        Ok(Self {
            value: value.into(),
            datatype: None,
            language: Some(language),
        })
    }

    /// Validate a language tag (basic BCP 47 validation)
    fn validate_language_tag(tag: &str) -> Result<(), ParserError> {
        if tag.is_empty() {
            return Err(ParserError::InvalidInput(
                "Language tag cannot be empty".into(),
            ));
        }

        // Basic validation: alphanumeric and hyphens, starts with letter
        let first = tag.chars().next().unwrap();
        if !first.is_ascii_alphabetic() {
            return Err(ParserError::InvalidInput(
                "Language tag must start with a letter".into(),
            ));
        }

        for ch in tag.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '-' {
                return Err(ParserError::InvalidInput(format!(
                    "Invalid character in language tag: {}",
                    ch
                )));
            }
        }

        Ok(())
    }

    /// Get the lexical value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Get the datatype IRI
    ///
    /// Returns xsd:string for plain literals, rdf:langString for language-tagged literals
    pub fn datatype(&self) -> Iri {
        if self.language.is_some() {
            Iri::new_unchecked(RDF_LANG_STRING)
        } else {
            self.datatype.clone().unwrap_or_else(xsd::string)
        }
    }

    /// Get the explicit datatype if one was set
    pub fn explicit_datatype(&self) -> Option<&Iri> {
        self.datatype.as_ref()
    }

    /// Get the language tag, if present
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Check if this is a plain literal (no explicit datatype or language)
    pub fn is_plain(&self) -> bool {
        self.datatype.is_none() && self.language.is_none()
    }

    /// Check if this is a language-tagged literal
    pub fn is_language_tagged(&self) -> bool {
        self.language.is_some()
    }

    /// Try to parse the value as a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    /// Try to parse the value as an integer
    pub fn as_integer(&self) -> Option<i64> {
        self.value.parse().ok()
    }

    /// Try to parse the value as a float
    pub fn as_float(&self) -> Option<f64> {
        self.value.parse().ok()
    }

    /// Try to parse the value as a date in ISO 8601 format (YYYY-MM-DD)
    ///
    /// Returns the original string if it's a valid ISO 8601 date format.
    /// This validates the basic ISO 8601 format with 4-digit years (0000-9999).
    /// Extended year representations are not supported.
    pub fn as_date(&self) -> Option<&str> {
        let s = self.value.as_str();
        
        // Basic format check: YYYY-MM-DD (10 characters)
        if s.len() != 10 {
            return None;
        }
        
        let bytes = s.as_bytes();
        // Check format: YYYY-MM-DD (year must be 4 digits)
        if bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        
        // Verify year portion is exactly 4 digits
        if !bytes[0..4].iter().all(|b| b.is_ascii_digit()) {
            return None;
        }
        
        // Parse year, month, day
        let year = s[0..4].parse::<i32>().ok()?;
        let month = s[5..7].parse::<u32>().ok()?;
        let day = s[8..10].parse::<u32>().ok()?;
        
        // Basic validation
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        
        // More precise day validation based on month
        let max_day = match month {
            2 => {
                // Leap year check
                if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        
        if day > max_day {
            return None;
        }
        
        Some(s)
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Escape the value for N-Triples format
        let escaped = self
            .value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        write!(f, "\"{}\"", escaped)?;

        if let Some(ref lang) = self.language {
            write!(f, "@{}", lang)
        } else if let Some(ref dt) = self.datatype {
            write!(f, "^^{}", dt)
        } else {
            Ok(())
        }
    }
}

/// Convenience constructors for common literal types
impl Literal {
    /// Create a boolean literal
    pub fn boolean(value: bool) -> Self {
        Self::with_datatype(if value { "true" } else { "false" }, xsd::boolean())
    }

    /// Create an integer literal
    pub fn integer(value: i64) -> Self {
        Self::with_datatype(value.to_string(), xsd::integer())
    }

    /// Create a decimal literal
    pub fn decimal(value: f64) -> Self {
        Self::with_datatype(value.to_string(), xsd::decimal())
    }

    /// Create a double literal
    pub fn double(value: f64) -> Self {
        Self::with_datatype(value.to_string(), xsd::double())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_literal() {
        let lit = Literal::new("hello");
        assert_eq!(lit.value(), "hello");
        assert!(lit.is_plain());
        assert!(!lit.is_language_tagged());
    }

    #[test]
    fn test_typed_literal() {
        let lit = Literal::with_datatype("42", xsd::integer());
        assert_eq!(lit.value(), "42");
        assert_eq!(lit.as_integer(), Some(42));
        assert!(!lit.is_plain());
    }

    #[test]
    fn test_language_tagged_literal() {
        let lit = Literal::with_language("hello", "en").unwrap();
        assert_eq!(lit.value(), "hello");
        assert_eq!(lit.language(), Some("en"));
        assert!(lit.is_language_tagged());
        assert_eq!(lit.datatype().as_str(), RDF_LANG_STRING);
    }

    #[test]
    fn test_boolean_literal() {
        let lit = Literal::boolean(true);
        assert_eq!(lit.as_bool(), Some(true));
        assert_eq!(
            format!("{}", lit),
            "\"true\"^^<http://www.w3.org/2001/XMLSchema#boolean>"
        );
    }

    #[test]
    fn test_literal_display() {
        let plain = Literal::new("test");
        assert_eq!(format!("{}", plain), "\"test\"");

        let lang = Literal::with_language("bonjour", "fr").unwrap();
        assert_eq!(format!("{}", lang), "\"bonjour\"@fr");

        let typed = Literal::integer(42);
        assert_eq!(
            format!("{}", typed),
            "\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>"
        );
    }

    #[test]
    fn test_literal_escaping() {
        let lit = Literal::new("line1\nline2\ttab\"quote");
        assert_eq!(format!("{}", lit), "\"line1\\nline2\\ttab\\\"quote\"");
    }

    #[test]
    fn test_invalid_language_tag() {
        assert!(Literal::with_language("test", "").is_err());
        assert!(Literal::with_language("test", "123").is_err());
        assert!(Literal::with_language("test", "en US").is_err());
    }

    #[test]
    fn test_as_date_valid() {
        let lit = Literal::with_datatype("1995-10-01", xsd::date());
        assert_eq!(lit.as_date(), Some("1995-10-01"));

        let lit2 = Literal::with_datatype("2024-02-29", xsd::date()); // leap year
        assert_eq!(lit2.as_date(), Some("2024-02-29"));

        let lit3 = Literal::with_datatype("2000-12-31", xsd::date());
        assert_eq!(lit3.as_date(), Some("2000-12-31"));
    }

    #[test]
    fn test_as_date_invalid() {
        // Wrong format
        let lit = Literal::with_datatype("10-01-1995", xsd::date());
        assert_eq!(lit.as_date(), None);

        // Invalid month
        let lit2 = Literal::with_datatype("1995-13-01", xsd::date());
        assert_eq!(lit2.as_date(), None);

        // Invalid day
        let lit3 = Literal::with_datatype("1995-02-30", xsd::date());
        assert_eq!(lit3.as_date(), None);

        // Non-leap year February 29
        let lit4 = Literal::with_datatype("1995-02-29", xsd::date());
        assert_eq!(lit4.as_date(), None);

        // Invalid format (too short)
        let lit5 = Literal::with_datatype("1995-1-1", xsd::date());
        assert_eq!(lit5.as_date(), None);

        // Not a date at all
        let lit6 = Literal::new("hello");
        assert_eq!(lit6.as_date(), None);

        // Negative year (extended format not supported)
        let lit7 = Literal::with_datatype("-123-01-01", xsd::date());
        assert_eq!(lit7.as_date(), None);
    }
}
