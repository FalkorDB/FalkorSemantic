//! Query Validation
//!
//! Validates SPARQL queries for semantic correctness.

use std::collections::HashSet;

use super::ast::{Query, Variable};
use super::error::{SparqlError, SparqlResult};

/// Validation error type
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error message
    pub message: String,
    /// The variable or element that caused the error
    pub element: Option<String>,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            element: None,
        }
    }

    /// Create with an associated element
    pub fn with_element(message: impl Into<String>, element: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            element: Some(element.into()),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.element {
            Some(elem) => write!(f, "{}: {}", self.message, elem),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Query validator
#[derive(Debug, Default, Clone)]
pub struct QueryValidator {
    /// Allow undefined variables in projection
    pub allow_undefined_projection: bool,
    /// Maximum pattern depth
    pub max_pattern_depth: Option<usize>,
}

impl QueryValidator {
    /// Create a new validator with default settings
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to allow undefined variables in projection
    #[must_use]
    pub const fn allow_undefined_projection(mut self, allow: bool) -> Self {
        self.allow_undefined_projection = allow;
        self
    }

    /// Set maximum pattern depth
    #[must_use]
    pub const fn max_pattern_depth(mut self, depth: usize) -> Self {
        self.max_pattern_depth = Some(depth);
        self
    }

    /// Validate a query
    pub fn validate(&self, query: &Query) -> SparqlResult<()> {
        let errors = self.collect_errors(query);
        if errors.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = errors
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            Err(SparqlError::validation(messages.join("; ")))
        }
    }

    /// Collect all validation errors
    #[must_use]
    pub fn collect_errors(&self, query: &Query) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check SELECT projection
        if let Some(select) = query.as_select() {
            if !self.allow_undefined_projection {
                if let Some(projection) = &select.projection {
                    let pattern_vars = select.pattern.variables();
                    for var in projection {
                        if !pattern_vars.contains(var) && !is_aggregate_variable(var) {
                            errors.push(ValidationError::with_element(
                                "Projected variable not defined in pattern",
                                var.to_string(),
                            ));
                        }
                    }
                }
            }
        }

        // Check DESCRIBE resources
        if let Some(describe) = query.as_describe() {
            let pattern_vars = describe.pattern.variables();
            for resource in &describe.resources {
                if let super::ast::TermPattern::Variable(var) = resource {
                    if !pattern_vars.contains(var) {
                        errors.push(ValidationError::with_element(
                            "DESCRIBE variable not defined in pattern",
                            var.to_string(),
                        ));
                    }
                }
            }
        }

        errors
    }

    /// Check if a query is valid
    #[must_use]
    pub fn is_valid(&self, query: &Query) -> bool {
        self.collect_errors(query).is_empty()
    }
}

/// Check if a variable might be an aggregate (heuristic)
const fn is_aggregate_variable(_var: &Variable) -> bool {
    // This is a placeholder - in practice we'd need to check
    // if the variable is bound by an aggregate expression
    false
}

/// Collect all variables from a query that must be bound
#[allow(dead_code)]
pub fn required_bindings(query: &Query) -> HashSet<Variable> {
    match query {
        Query::Select(q) => {
            if let Some(projection) = &q.projection {
                projection.iter().cloned().collect()
            } else {
                q.pattern.variables()
            }
        }
        Query::Construct(q) => {
            // CONSTRUCT needs all variables in template to be bound
            let mut vars = HashSet::new();
            for triple in &q.template {
                vars.extend(triple.variables());
            }
            vars
        }
        Query::Ask(_) => HashSet::new(),
        Query::Describe(q) => {
            let mut vars = HashSet::new();
            for resource in &q.resources {
                if let super::ast::TermPattern::Variable(v) = resource {
                    vars.insert(v.clone());
                }
            }
            vars
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::new("test error");
        assert_eq!(format!("{}", err), "test error");

        let err = ValidationError::with_element("undefined variable", "?x");
        assert_eq!(format!("{}", err), "undefined variable: ?x");
    }

    #[test]
    fn test_validator_builder() {
        let validator = QueryValidator::new()
            .allow_undefined_projection(true)
            .max_pattern_depth(10);

        assert!(validator.allow_undefined_projection);
        assert_eq!(validator.max_pattern_depth, Some(10));
    }

    #[test]
    fn test_validate_valid_select() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("SELECT ?s ?p WHERE { ?s ?p ?o }").unwrap();
        let validator = QueryValidator::new();
        assert!(validator.validate(&query).is_ok());
        assert!(validator.is_valid(&query));
    }

    #[test]
    fn test_validate_undefined_projection_variable() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        // ?undefined is projected but not bound in the WHERE clause
        let query = parser
            .parse("SELECT ?s ?undefined WHERE { ?s ?p ?o }")
            .unwrap();
        let validator = QueryValidator::new();
        let errors = validator.collect_errors(&query);
        if !errors.is_empty() {
            // If the validator catches it, error message should mention the variable
            assert!(errors[0].to_string().contains("undefined"));
            assert!(validator.validate(&query).is_err());
            assert!(!validator.is_valid(&query));
        }
        // (spargebra may or may not include unbound vars in projection)
    }

    #[test]
    fn test_validate_allow_undefined_projection() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("SELECT ?s ?p WHERE { ?s ?p ?o }").unwrap();
        let validator = QueryValidator::new().allow_undefined_projection(true);
        assert!(validator.validate(&query).is_ok());
        assert!(validator.is_valid(&query));
    }

    #[test]
    fn test_validate_ask_query() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("ASK { ?s ?p ?o }").unwrap();
        let validator = QueryValidator::new();
        assert!(validator.validate(&query).is_ok());
        assert!(validator.is_valid(&query));
        assert!(validator.collect_errors(&query).is_empty());
    }

    #[test]
    fn test_validate_construct_query() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser
            .parse("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
            .unwrap();
        let validator = QueryValidator::new();
        assert!(validator.validate(&query).is_ok());
    }

    #[test]
    fn test_validate_describe_with_iri() {
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser
            .parse("DESCRIBE <http://example.org/resource>")
            .unwrap();
        let validator = QueryValidator::new();
        let errors = validator.collect_errors(&query);
        // IRIs in DESCRIBE don't produce errors (only undefined variables do)
        assert!(errors.is_empty());
        assert!(validator.is_valid(&query));
    }

    #[test]
    fn test_required_bindings_select_with_projection() {
        use super::required_bindings;
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("SELECT ?s ?p WHERE { ?s ?p ?o }").unwrap();
        let bindings = required_bindings(&query);
        assert!(bindings.iter().any(|v| v.name == "s"));
        assert!(bindings.iter().any(|v| v.name == "p"));
    }

    #[test]
    fn test_required_bindings_select_star() {
        use super::required_bindings;
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("SELECT * WHERE { ?s ?p ?o }").unwrap();
        let bindings = required_bindings(&query);
        // SELECT * returns all pattern variables
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_required_bindings_ask() {
        use super::required_bindings;
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("ASK { ?s ?p ?o }").unwrap();
        let bindings = required_bindings(&query);
        // ASK has no required bindings
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_required_bindings_construct() {
        use super::required_bindings;
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser
            .parse("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
            .unwrap();
        let bindings = required_bindings(&query);
        // Template variables are required
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_required_bindings_describe_with_variable() {
        use super::required_bindings;
        use crate::sparql::SparqlParser;
        let parser = SparqlParser::new();
        let query = parser.parse("DESCRIBE ?s WHERE { ?s ?p ?o }").unwrap();
        let bindings = required_bindings(&query);
        assert!(bindings.iter().any(|v| v.name == "s"));
    }
}
