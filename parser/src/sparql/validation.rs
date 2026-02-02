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
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to allow undefined variables in projection
    pub fn allow_undefined_projection(mut self, allow: bool) -> Self {
        self.allow_undefined_projection = allow;
        self
    }

    /// Set maximum pattern depth
    pub fn max_pattern_depth(mut self, depth: usize) -> Self {
        self.max_pattern_depth = Some(depth);
        self
    }

    /// Validate a query
    pub fn validate(&self, query: &Query) -> SparqlResult<()> {
        let errors = self.collect_errors(query);
        if errors.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            Err(SparqlError::validation(messages.join("; ")))
        }
    }

    /// Collect all validation errors
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
    pub fn is_valid(&self, query: &Query) -> bool {
        self.collect_errors(query).is_empty()
    }
}

/// Check if a variable might be an aggregate (heuristic)
fn is_aggregate_variable(_var: &Variable) -> bool {
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
}
