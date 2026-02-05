//! SPARQL Error Types
//!
//! Error types for SPARQL parsing with position information.

use std::fmt;

/// Result type for SPARQL operations
pub type SparqlResult<T> = std::result::Result<T, SparqlError>;

/// SPARQL parsing error with location information
#[derive(Debug, Clone)]
pub struct SparqlError {
    /// The kind of error
    pub kind: SparqlErrorKind,
    /// Error message
    pub message: String,
    /// Line number where the error occurred (1-indexed)
    pub line: Option<usize>,
    /// Column number where the error occurred (1-indexed)
    pub column: Option<usize>,
    /// Byte offset in the input
    pub offset: Option<usize>,
}

impl SparqlError {
    /// Create a new parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self {
            kind: SparqlErrorKind::Parse,
            message: message.into(),
            line: None,
            column: None,
            offset: None,
        }
    }

    /// Create a new validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: SparqlErrorKind::Validation,
            message: message.into(),
            line: None,
            column: None,
            offset: None,
        }
    }

    /// Create a new unsupported feature error
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: SparqlErrorKind::Unsupported,
            message: message.into(),
            line: None,
            column: None,
            offset: None,
        }
    }

    /// Add position information from byte offset
    #[must_use]
    pub fn with_position(mut self, input: &str, offset: usize) -> Self {
        self.offset = Some(offset);

        // Calculate line and column from offset
        let before = &input[..offset.min(input.len())];
        let line = before.matches('\n').count() + 1;
        let last_newline = before.rfind('\n').map_or(0, |i| i + 1);
        let column = offset - last_newline + 1;

        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Add explicit line/column information
    #[must_use]
    pub const fn with_location(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

impl fmt::Display for SparqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(col)) => {
                write!(f, "{}:{}: {} - {}", line, col, self.kind, self.message)
            }
            (Some(line), None) => {
                write!(f, "line {}: {} - {}", line, self.kind, self.message)
            }
            _ => write!(f, "{} - {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for SparqlError {}

/// Kind of SPARQL error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparqlErrorKind {
    /// Syntax/parse error
    Parse,
    /// Semantic validation error
    Validation,
    /// Unsupported SPARQL feature
    Unsupported,
}

impl fmt::Display for SparqlErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => write!(f, "parse error"),
            Self::Validation => write!(f, "validation error"),
            Self::Unsupported => write!(f, "unsupported"),
        }
    }
}

impl From<spargebra::SparqlSyntaxError> for SparqlError {
    fn from(err: spargebra::SparqlSyntaxError) -> Self {
        // spargebra exposes position info
        let message = err.to_string();
        Self::parse(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SparqlError::parse("unexpected token");
        assert!(err.to_string().contains("parse error"));
        assert!(err.to_string().contains("unexpected token"));
    }

    #[test]
    fn test_error_with_position() {
        let input = "SELECT ?x\nWHERE { error }";
        let err = SparqlError::parse("test").with_position(input, 16);

        assert_eq!(err.line, Some(2));
        assert_eq!(err.column, Some(7));
    }

    #[test]
    fn test_error_with_location() {
        let err = SparqlError::validation("undefined variable").with_location(5, 10);
        assert_eq!(err.line, Some(5));
        assert_eq!(err.column, Some(10));
    }
}
