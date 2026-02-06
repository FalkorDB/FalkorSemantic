//! Binding and result set types for SPARQL results
//!
//! Represents variable bindings and result sets from query execution.

use super::Term;
use crate::rdf::Triple;
use std::collections::HashMap;

/// A single variable binding in a result
pub type Binding = HashMap<String, Term>;

/// A set of results from a SELECT query
#[derive(Debug, Clone, Default)]
pub struct SelectResults {
    /// The variables in the result set (column names)
    pub variables: Vec<String>,
    /// The result bindings (rows)
    pub bindings: Vec<Binding>,
}

impl SelectResults {
    /// Create a new empty result set
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a result set with the given variables
    #[must_use]
    pub const fn with_variables(variables: Vec<String>) -> Self {
        Self {
            variables,
            bindings: Vec::new(),
        }
    }

    /// Add a binding (row) to the result set
    pub fn add_binding(&mut self, binding: Binding) {
        self.bindings.push(binding);
    }

    /// Get the number of results
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Check if the result set is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Get the number of variables
    #[must_use]
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    /// Iterate over bindings
    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.iter()
    }

    /// Get a binding by index
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Binding> {
        self.bindings.get(index)
    }
}

/// Result from an ASK query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AskResult {
    /// Whether the pattern matched
    pub result: bool,
}

impl AskResult {
    /// Create a new ASK result
    #[must_use]
    pub const fn new(result: bool) -> Self {
        Self { result }
    }
}

impl From<bool> for AskResult {
    fn from(result: bool) -> Self {
        Self::new(result)
    }
}

/// Results from a CONSTRUCT or DESCRIBE query
#[derive(Debug, Clone, Default)]
pub struct ConstructResults {
    /// The constructed triples
    pub triples: Vec<Triple>,
}

impl ConstructResults {
    /// Create a new empty construct result
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with the given triples
    #[must_use]
    pub const fn with_triples(triples: Vec<Triple>) -> Self {
        Self { triples }
    }

    /// Add a triple to the results
    pub fn add_triple(&mut self, triple: Triple) {
        self.triples.push(triple);
    }

    /// Get the number of triples
    #[must_use]
    pub fn len(&self) -> usize {
        self.triples.len()
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    /// Iterate over triples
    pub fn iter(&self) -> impl Iterator<Item = &Triple> {
        self.triples.iter()
    }
}

/// Combined query results enum
#[derive(Debug, Clone)]
pub enum ResultSet {
    /// SELECT query results
    Select(SelectResults),
    /// ASK query result
    Ask(AskResult),
    /// CONSTRUCT/DESCRIBE query results
    Construct(ConstructResults),
}

impl ResultSet {
    /// Create SELECT results
    #[must_use]
    pub const fn select(variables: Vec<String>, bindings: Vec<Binding>) -> Self {
        Self::Select(SelectResults {
            variables,
            bindings,
        })
    }

    /// Create ASK result
    #[must_use]
    pub fn ask(result: bool) -> Self {
        Self::Ask(AskResult::new(result))
    }

    /// Create CONSTRUCT results
    #[must_use]
    pub fn construct(triples: Vec<Triple>) -> Self {
        Self::Construct(ConstructResults::with_triples(triples))
    }

    /// Check if this is a SELECT result
    #[must_use]
    pub const fn is_select(&self) -> bool {
        matches!(self, Self::Select(_))
    }

    /// Check if this is an ASK result
    #[must_use]
    pub const fn is_ask(&self) -> bool {
        matches!(self, Self::Ask(_))
    }

    /// Check if this is a CONSTRUCT result
    #[must_use]
    pub const fn is_construct(&self) -> bool {
        matches!(self, Self::Construct(_))
    }

    /// Get as SELECT results
    #[must_use]
    pub const fn as_select(&self) -> Option<&SelectResults> {
        match self {
            Self::Select(r) => Some(r),
            _ => None,
        }
    }

    /// Get as ASK result
    #[must_use]
    pub const fn as_ask(&self) -> Option<&AskResult> {
        match self {
            Self::Ask(r) => Some(r),
            _ => None,
        }
    }

    /// Get as CONSTRUCT results
    #[must_use]
    pub const fn as_construct(&self) -> Option<&ConstructResults> {
        match self {
            Self::Construct(r) => Some(r),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_results() {
        let mut results = SelectResults::with_variables(vec!["s".to_string(), "p".to_string()]);
        assert_eq!(results.num_variables(), 2);
        assert!(results.is_empty());

        let mut binding = Binding::new();
        binding.insert("s".to_string(), Term::iri("http://example.org/s1"));
        binding.insert("p".to_string(), Term::iri("http://example.org/p1"));
        results.add_binding(binding);

        assert_eq!(results.len(), 1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_ask_result() {
        let result = AskResult::new(true);
        assert!(result.result);

        let result: AskResult = false.into();
        assert!(!result.result);
    }

    #[test]
    fn test_construct_results() {
        use crate::rdf::{Iri, Object, Predicate, Subject};

        let mut results = ConstructResults::new();
        assert!(results.is_empty());

        let triple = Triple::new(
            Subject::Iri(Iri::new_unchecked("http://example.org/s")),
            Predicate::new_unchecked("http://example.org/p"),
            Object::Iri(Iri::new_unchecked("http://example.org/o")),
        );
        results.add_triple(triple);

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_result_set_enum() {
        let select = ResultSet::select(vec!["x".to_string()], vec![]);
        assert!(select.is_select());
        assert!(select.as_select().is_some());

        let ask = ResultSet::ask(true);
        assert!(ask.is_ask());
        assert!(ask.as_ask().unwrap().result);

        let construct = ResultSet::construct(vec![]);
        assert!(construct.is_construct());
    }
}
