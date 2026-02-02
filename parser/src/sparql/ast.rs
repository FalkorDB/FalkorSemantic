//! SPARQL AST Wrapper Types
//!
//! Wrapper types around spargebra's AST for internal use.
//! These provide a cleaner API and additional functionality.

use std::collections::HashSet;
use std::fmt;

/// A parsed SPARQL query
#[derive(Debug, Clone)]
pub enum Query {
    /// SELECT query
    Select(SelectQuery),
    /// CONSTRUCT query
    Construct(ConstructQuery),
    /// ASK query
    Ask(AskQuery),
    /// DESCRIBE query
    Describe(DescribeQuery),
}

impl Query {
    /// Check if this is a SELECT query
    pub fn is_select(&self) -> bool {
        matches!(self, Query::Select(_))
    }

    /// Check if this is a CONSTRUCT query
    pub fn is_construct(&self) -> bool {
        matches!(self, Query::Construct(_))
    }

    /// Check if this is an ASK query
    pub fn is_ask(&self) -> bool {
        matches!(self, Query::Ask(_))
    }

    /// Check if this is a DESCRIBE query
    pub fn is_describe(&self) -> bool {
        matches!(self, Query::Describe(_))
    }

    /// Get as SELECT query
    pub fn as_select(&self) -> Option<&SelectQuery> {
        match self {
            Query::Select(q) => Some(q),
            _ => None,
        }
    }

    /// Get as CONSTRUCT query
    pub fn as_construct(&self) -> Option<&ConstructQuery> {
        match self {
            Query::Construct(q) => Some(q),
            _ => None,
        }
    }

    /// Get as ASK query
    pub fn as_ask(&self) -> Option<&AskQuery> {
        match self {
            Query::Ask(q) => Some(q),
            _ => None,
        }
    }

    /// Get as DESCRIBE query
    pub fn as_describe(&self) -> Option<&DescribeQuery> {
        match self {
            Query::Describe(q) => Some(q),
            _ => None,
        }
    }

    /// Get all variables used in the query
    pub fn variables(&self) -> HashSet<Variable> {
        match self {
            Query::Select(q) => q.variables(),
            Query::Construct(q) => q.variables(),
            Query::Ask(q) => q.variables(),
            Query::Describe(q) => q.variables(),
        }
    }

    /// Get the graph pattern (WHERE clause)
    pub fn pattern(&self) -> Option<&GraphPattern> {
        match self {
            Query::Select(q) => Some(&q.pattern),
            Query::Construct(q) => Some(&q.pattern),
            Query::Ask(q) => Some(&q.pattern),
            Query::Describe(q) => Some(&q.pattern),
        }
    }

    /// Get the dataset clause if any
    pub fn dataset(&self) -> Option<&QueryDataset> {
        match self {
            Query::Select(q) => q.dataset.as_ref(),
            Query::Construct(q) => q.dataset.as_ref(),
            Query::Ask(q) => q.dataset.as_ref(),
            Query::Describe(q) => q.dataset.as_ref(),
        }
    }

    /// Get the original spargebra query
    pub fn inner(&self) -> spargebra::Query {
        match self {
            Query::Select(q) => q.inner.clone(),
            Query::Construct(q) => q.inner.clone(),
            Query::Ask(q) => q.inner.clone(),
            Query::Describe(q) => q.inner.clone(),
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner())
    }
}

/// A SELECT query
#[derive(Debug, Clone)]
pub struct SelectQuery {
    /// The original spargebra query
    pub(crate) inner: spargebra::Query,
    /// Variables to project (None = SELECT *)
    pub projection: Option<Vec<Variable>>,
    /// Whether this is SELECT DISTINCT
    pub distinct: bool,
    /// Whether this is SELECT REDUCED
    pub reduced: bool,
    /// The WHERE clause pattern
    pub pattern: GraphPattern,
    /// Dataset clause (FROM/FROM NAMED)
    pub dataset: Option<QueryDataset>,
    /// ORDER BY clause
    pub order_by: Option<Vec<OrderCondition>>,
    /// LIMIT clause
    pub limit: Option<usize>,
    /// OFFSET clause
    pub offset: Option<usize>,
    /// GROUP BY clause
    pub group_by: Option<Vec<Variable>>,
    /// HAVING clause
    pub having: Option<Expression>,
}

impl SelectQuery {
    /// Check if this is SELECT *
    pub fn is_select_all(&self) -> bool {
        self.projection.is_none()
    }

    /// Get projected variables (returns owned values for consistency)
    pub fn projected_variables(&self) -> Vec<Variable> {
        match &self.projection {
            Some(vars) => vars.clone(),
            None => self.pattern.variables().into_iter().collect(),
        }
    }

    /// Get all variables used in the query
    pub fn variables(&self) -> HashSet<Variable> {
        let mut vars = self.pattern.variables();
        if let Some(proj) = &self.projection {
            for v in proj {
                vars.insert(v.clone());
            }
        }
        vars
    }
}

/// A CONSTRUCT query
#[derive(Debug, Clone)]
pub struct ConstructQuery {
    /// The original spargebra query
    pub(crate) inner: spargebra::Query,
    /// The template triples
    pub template: Vec<TriplePattern>,
    /// The WHERE clause pattern
    pub pattern: GraphPattern,
    /// Dataset clause
    pub dataset: Option<QueryDataset>,
}

impl ConstructQuery {
    /// Get all variables used in the query
    pub fn variables(&self) -> HashSet<Variable> {
        let mut vars = self.pattern.variables();
        for triple in &self.template {
            vars.extend(triple.variables());
        }
        vars
    }
}

/// An ASK query
#[derive(Debug, Clone)]
pub struct AskQuery {
    /// The original spargebra query
    pub(crate) inner: spargebra::Query,
    /// The WHERE clause pattern
    pub pattern: GraphPattern,
    /// Dataset clause
    pub dataset: Option<QueryDataset>,
}

impl AskQuery {
    /// Get all variables used in the query
    pub fn variables(&self) -> HashSet<Variable> {
        self.pattern.variables()
    }
}

/// A DESCRIBE query
#[derive(Debug, Clone)]
pub struct DescribeQuery {
    /// The original spargebra query
    pub(crate) inner: spargebra::Query,
    /// Resources to describe (variables or IRIs)
    pub resources: Vec<TermPattern>,
    /// The WHERE clause pattern
    pub pattern: GraphPattern,
    /// Dataset clause
    pub dataset: Option<QueryDataset>,
}

impl DescribeQuery {
    /// Get all variables used in the query
    pub fn variables(&self) -> HashSet<Variable> {
        let mut vars = self.pattern.variables();
        for resource in &self.resources {
            if let TermPattern::Variable(v) = resource {
                vars.insert(v.clone());
            }
        }
        vars
    }
}

/// A SPARQL variable
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Variable {
    /// The variable name (without the ? or $ prefix)
    pub name: String,
}

impl Variable {
    /// Create a new variable
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Get the variable name with ? prefix
    pub fn with_prefix(&self) -> String {
        format!("?{}", self.name)
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

impl From<spargebra::term::Variable> for Variable {
    fn from(v: spargebra::term::Variable) -> Self {
        Self {
            name: v.as_str().to_string(),
        }
    }
}

/// A named node (IRI)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamedNode {
    /// The IRI string
    pub iri: String,
}

impl NamedNode {
    /// Create a new named node
    pub fn new(iri: impl Into<String>) -> Self {
        Self { iri: iri.into() }
    }
}

impl fmt::Display for NamedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.iri)
    }
}

impl From<spargebra::term::NamedNode> for NamedNode {
    fn from(n: spargebra::term::NamedNode) -> Self {
        Self {
            iri: n.as_str().to_string(),
        }
    }
}

/// A term pattern (can be a variable, IRI, literal, or blank node)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TermPattern {
    Variable(Variable),
    NamedNode(NamedNode),
    Literal(String),
    BlankNode(String),
}

impl TermPattern {
    /// Check if this is a variable
    pub fn is_variable(&self) -> bool {
        matches!(self, TermPattern::Variable(_))
    }

    /// Get as variable if it is one
    pub fn as_variable(&self) -> Option<&Variable> {
        match self {
            TermPattern::Variable(v) => Some(v),
            _ => None,
        }
    }
}

/// A triple pattern in a graph pattern
#[derive(Debug, Clone)]
pub struct TriplePattern {
    /// Subject
    pub subject: TermPattern,
    /// Predicate
    pub predicate: TermPattern,
    /// Object
    pub object: TermPattern,
}

impl TriplePattern {
    /// Get all variables in this triple pattern
    pub fn variables(&self) -> HashSet<Variable> {
        let mut vars = HashSet::new();
        if let TermPattern::Variable(v) = &self.subject {
            vars.insert(v.clone());
        }
        if let TermPattern::Variable(v) = &self.predicate {
            vars.insert(v.clone());
        }
        if let TermPattern::Variable(v) = &self.object {
            vars.insert(v.clone());
        }
        vars
    }
}

/// A graph pattern (WHERE clause contents)
#[derive(Debug, Clone)]
pub struct GraphPattern {
    /// The underlying spargebra pattern
    pub(crate) inner: spargebra::algebra::GraphPattern,
}

impl GraphPattern {
    /// Get all variables in this pattern
    pub fn variables(&self) -> HashSet<Variable> {
        extract_variables(&self.inner)
    }

    /// Check if the pattern is empty (no patterns)
    pub fn is_empty(&self) -> bool {
        matches!(&self.inner, spargebra::algebra::GraphPattern::Bgp { patterns } if patterns.is_empty())
    }

    /// Get the underlying spargebra pattern for advanced translation
    pub fn inner(&self) -> &spargebra::algebra::GraphPattern {
        &self.inner
    }
}

/// Extract variables from a spargebra graph pattern
fn extract_variables(pattern: &spargebra::algebra::GraphPattern) -> HashSet<Variable> {
    use spargebra::algebra::GraphPattern as GP;
    use spargebra::term::TermPattern as TP;

    let mut vars = HashSet::new();

    fn add_term_pattern_var(tp: &TP, vars: &mut HashSet<Variable>) {
        if let TP::Variable(v) = tp {
            vars.insert(Variable::from(v.clone()));
        }
    }

    fn add_named_node_pattern_var(
        tp: &spargebra::term::NamedNodePattern,
        vars: &mut HashSet<Variable>,
    ) {
        if let spargebra::term::NamedNodePattern::Variable(v) = tp {
            vars.insert(Variable::from(v.clone()));
        }
    }

    match pattern {
        GP::Bgp { patterns } => {
            for triple in patterns {
                add_term_pattern_var(&triple.subject, &mut vars);
                add_named_node_pattern_var(&triple.predicate, &mut vars);
                add_term_pattern_var(&triple.object, &mut vars);
            }
        }
        GP::Path {
            subject, object, ..
        } => {
            add_term_pattern_var(subject, &mut vars);
            add_term_pattern_var(object, &mut vars);
        }
        GP::Join { left, right }
        | GP::LeftJoin { left, right, .. }
        | GP::Minus { left, right }
        | GP::Union { left, right } => {
            vars.extend(extract_variables(left));
            vars.extend(extract_variables(right));
        }
        GP::Filter { inner, .. } | GP::Graph { inner, .. } => {
            vars.extend(extract_variables(inner));
        }
        GP::Extend {
            inner, variable, ..
        } => {
            vars.extend(extract_variables(inner));
            vars.insert(Variable::from(variable.clone()));
        }
        GP::Group {
            inner, variables, ..
        } => {
            vars.extend(extract_variables(inner));
            for v in variables {
                vars.insert(Variable::from(v.clone()));
            }
        }
        GP::Values { variables, .. } => {
            for v in variables {
                vars.insert(Variable::from(v.clone()));
            }
        }
        GP::OrderBy { inner, .. } | GP::Distinct { inner } | GP::Reduced { inner } => {
            vars.extend(extract_variables(inner));
        }
        GP::Project { inner, variables } => {
            vars.extend(extract_variables(inner));
            for v in variables {
                vars.insert(Variable::from(v.clone()));
            }
        }
        GP::Slice { inner, .. } => {
            vars.extend(extract_variables(inner));
        }
        GP::Service { inner, .. } => {
            vars.extend(extract_variables(inner));
        }
    }

    vars
}

/// An expression in SPARQL
#[derive(Debug, Clone)]
pub struct Expression {
    #[allow(dead_code)]
    pub(crate) inner: spargebra::algebra::Expression,
}

/// An ordering condition
#[derive(Debug, Clone)]
pub struct OrderCondition {
    /// The expression to order by
    pub expression: Expression,
    /// Whether to order in descending order
    pub descending: bool,
}

/// Query dataset (FROM and FROM NAMED clauses)
#[derive(Debug, Clone, Default)]
pub struct QueryDataset {
    /// Default graphs (FROM)
    pub default_graphs: Vec<NamedNode>,
    /// Named graphs (FROM NAMED)
    pub named_graphs: Vec<NamedNode>,
}

impl QueryDataset {
    /// Check if the dataset is empty
    pub fn is_empty(&self) -> bool {
        self.default_graphs.is_empty() && self.named_graphs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable() {
        let v = Variable::new("x");
        assert_eq!(v.name, "x");
        assert_eq!(v.with_prefix(), "?x");
        assert_eq!(format!("{}", v), "?x");
    }

    #[test]
    fn test_named_node() {
        let n = NamedNode::new("http://example.org/foo");
        assert_eq!(format!("{}", n), "<http://example.org/foo>");
    }

    #[test]
    fn test_term_pattern() {
        let var = TermPattern::Variable(Variable::new("x"));
        assert!(var.is_variable());
        assert_eq!(var.as_variable().unwrap().name, "x");

        let iri = TermPattern::NamedNode(NamedNode::new("http://example.org/"));
        assert!(!iri.is_variable());
        assert!(iri.as_variable().is_none());
    }
}
