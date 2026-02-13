//! Query Executor
//!
//! Executes Cypher queries and converts results to SPARQL result format.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use falkorsemantic_parser::rdf::{BlankNode, Iri, Literal, Object, Subject, Triple};
use falkorsemantic_parser::results::{AskResult, Binding, ConstructResults, SelectResults, Term};

use super::translator::{CypherQuery, CypherQueryType, TemplateTerm};

/// Configuration for query execution
#[derive(Debug, Clone)]
pub struct QueryConfig {
    /// Query timeout
    pub timeout: Option<Duration>,
    /// Maximum number of results
    pub max_results: Option<usize>,
    /// Whether to include metadata in results
    pub include_metadata: bool,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            max_results: None,
            include_metadata: false,
        }
    }
}

impl QueryConfig {
    /// Create a new config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set query timeout
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set no timeout
    #[must_use]
    pub const fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Set maximum results
    #[must_use]
    pub const fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = Some(max);
        self
    }
}

/// Result of query execution
#[derive(Debug)]
pub enum QueryResult {
    /// SELECT query results
    Select(SelectResults),
    /// ASK query result
    Ask(AskResult),
    /// CONSTRUCT query results (RDF triples)
    Construct(ConstructResults),
    /// Error during execution
    Error(QueryError),
}

/// Query execution error
#[derive(Debug, Clone)]
pub struct QueryError {
    /// Error message
    pub message: String,
    /// Error code (if available)
    pub code: Option<String>,
    /// Query that caused the error
    pub query: Option<String>,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Query error: {}", self.message)
    }
}

impl std::error::Error for QueryError {}

/// Trait for executing Cypher queries
pub trait CypherExecutor {
    /// Execute a Cypher query and return raw results
    fn execute(&self, query: &str) -> Result<CypherResult, QueryError>;

    /// Execute with timeout
    fn execute_with_timeout(
        &self,
        query: &str,
        timeout: Duration,
    ) -> Result<CypherResult, QueryError>;
}

/// Raw Cypher query result
#[derive(Debug, Clone)]
pub struct CypherResult {
    /// Column headers
    pub columns: Vec<String>,
    /// Result rows
    pub rows: Vec<Vec<CypherValue>>,
    /// Execution statistics
    pub stats: Option<ExecutionStats>,
}

/// A value from Cypher results
#[derive(Debug, Clone)]
pub enum CypherValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// String value
    String(String),
    /// Node (with properties)
    Node(HashMap<String, Self>),
    /// Relationship (with properties)
    Relationship(HashMap<String, Self>),
    /// List of values
    List(Vec<Self>),
    /// Map of values
    Map(HashMap<String, Self>),
}

impl CypherValue {
    /// Convert to SPARQL Term
    #[must_use]
    pub fn to_term(&self) -> Option<Term> {
        match self {
            Self::Null => None,
            Self::Bool(b) => Some(Term::typed_literal(
                b.to_string(),
                "http://www.w3.org/2001/XMLSchema#boolean",
            )),
            Self::Integer(i) => Some(Term::typed_literal(
                i.to_string(),
                "http://www.w3.org/2001/XMLSchema#integer",
            )),
            Self::Float(f) => Some(Term::typed_literal(
                f.to_string(),
                "http://www.w3.org/2001/XMLSchema#double",
            )),
            Self::String(s) => {
                // Check if it's a URI
                if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("urn:") {
                    Some(Term::iri(s.clone()))
                } else if let Some(stripped) = s.strip_prefix("_:") {
                    Some(Term::blank_node(stripped))
                } else {
                    Some(Term::literal(s.clone()))
                }
            }
            Self::Node(props) => {
                // Extract URI from node properties
                if let Some(Self::String(uri)) = props.get("uri") {
                    if let Some(stripped) = uri.strip_prefix("_:") {
                        Some(Term::blank_node(stripped))
                    } else {
                        Some(Term::iri(uri.clone()))
                    }
                } else if let Some(Self::String(value)) = props.get("value") {
                    // Literal node
                    let datatype = props.get("datatype").and_then(|d| match d {
                        Self::String(s) => Some(s.clone()),
                        _ => None,
                    });
                    let language = props.get("language").and_then(|l| match l {
                        Self::String(s) => Some(s.clone()),
                        _ => None,
                    });
                    if let Some(lang) = language {
                        Some(Term::lang_literal(value.clone(), lang))
                    } else if let Some(dt) = datatype {
                        Some(Term::typed_literal(value.clone(), dt))
                    } else {
                        Some(Term::literal(value.clone()))
                    }
                } else {
                    None
                }
            }
            Self::Relationship(props) => {
                // Extract predicate from relationship properties
                if let Some(Self::String(pred)) = props.get("predicate") {
                    Some(Term::iri(pred.clone()))
                } else {
                    None
                }
            }
            Self::Map(map) => {
                // Check for uri or value properties
                if let Some(Self::String(uri)) = map.get("uri") {
                    Some(Term::iri(uri.clone()))
                } else if let Some(Self::String(value)) = map.get("value") {
                    let datatype = map.get("datatype").and_then(|d| match d {
                        Self::String(s) => Some(s.clone()),
                        _ => None,
                    });
                    let language = map.get("language").and_then(|l| match l {
                        Self::String(s) => Some(s.clone()),
                        _ => None,
                    });
                    if let Some(lang) = language {
                        Some(Term::lang_literal(value.clone(), lang))
                    } else if let Some(dt) = datatype {
                        Some(Term::typed_literal(value.clone(), dt))
                    } else {
                        Some(Term::literal(value.clone()))
                    }
                } else {
                    None
                }
            }
            Self::List(_) => None, // Lists not directly representable
        }
    }
}

/// Execution statistics
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// Query execution time
    pub execution_time: Duration,
    /// Number of nodes read
    pub nodes_read: usize,
    /// Number of relationships read
    pub relationships_read: usize,
}

/// Converts Cypher results to SPARQL results
pub struct ResultConverter;

impl ResultConverter {
    /// Convert Cypher result to SELECT results
    #[must_use]
    pub fn to_select_results(
        cypher_query: &CypherQuery,
        cypher_result: CypherResult,
    ) -> SelectResults {
        let variables = cypher_query.variables.clone();

        let mut results = SelectResults::with_variables(variables.clone());

        for row in cypher_result.rows {
            let mut binding = Binding::new();
            for (i, value) in row.into_iter().enumerate() {
                if i < variables.len() {
                    if let Some(term) = value.to_term() {
                        binding.insert(variables[i].clone(), term);
                    }
                }
            }
            results.add_binding(binding);
        }

        results
    }

    /// Convert Cypher result to CONSTRUCT results by instantiating the template
    ///
    /// `row_offset` is used to generate unique blank-node IDs across calls.
    #[must_use]
    pub fn to_construct_results(
        cypher_query: &CypherQuery,
        cypher_result: CypherResult,
        row_offset: usize,
    ) -> ConstructResults {
        let template = match &cypher_query.construct_template {
            Some(t) => t,
            None => return ConstructResults::new(),
        };

        let variables = &cypher_query.variables;
        let mut results = ConstructResults::new();

        for (row_idx, row) in cypher_result.rows.into_iter().enumerate() {
            // Build variable → Term map for this row
            let mut binding: HashMap<String, Term> = HashMap::new();
            for (i, val) in row.into_iter().enumerate() {
                if i < variables.len() {
                    if let Some(term) = val.to_term() {
                        binding.insert(variables[i].clone(), term);
                    }
                }
            }

            let global_row = row_offset + row_idx;

            for tmpl in template {
                let subj = Self::instantiate_subject(&tmpl.subject, &binding, global_row);
                let pred = Self::instantiate_predicate(&tmpl.predicate, &binding, global_row);
                let obj = Self::instantiate_object(&tmpl.object, &binding, global_row);

                if let (Some(s), Some(p), Some(o)) = (subj, pred, obj) {
                    results.add_triple(Triple::new(s, p, o));
                }
            }
        }

        results
    }

    /// Instantiate a subject term from a result binding
    fn instantiate_subject(
        term: &TemplateTerm,
        binding: &HashMap<String, Term>,
        row_id: usize,
    ) -> Option<Subject> {
        match term {
            TemplateTerm::Bound(col) => match binding.get(col)? {
                Term::Iri(iri) => Some(Subject::Iri(iri.clone())),
                Term::BlankNode(bn) => Some(Subject::BlankNode(bn.clone())),
                Term::Literal(_) => None, // literals cannot be subjects
            },
            TemplateTerm::ConstantIri(iri) => Some(Subject::Iri(Iri::new_unchecked(iri.clone()))),
            TemplateTerm::BlankNode(label) => Some(Subject::BlankNode(BlankNode::new(format!(
                "{label}_{row_id}"
            )))),
            TemplateTerm::ConstantLiteral { .. } => None, // literals cannot be subjects
        }
    }

    /// Instantiate a predicate term from a result binding (must resolve to an IRI)
    fn instantiate_predicate(
        term: &TemplateTerm,
        binding: &HashMap<String, Term>,
        _row_id: usize,
    ) -> Option<Iri> {
        match term {
            TemplateTerm::Bound(col) => match binding.get(col)? {
                Term::Iri(iri) => Some(iri.clone()),
                _ => None, // predicates must be IRIs
            },
            TemplateTerm::ConstantIri(iri) => Some(Iri::new_unchecked(iri.clone())),
            _ => None,
        }
    }

    /// Instantiate an object term from a result binding
    fn instantiate_object(
        term: &TemplateTerm,
        binding: &HashMap<String, Term>,
        row_id: usize,
    ) -> Option<Object> {
        match term {
            TemplateTerm::Bound(col) => match binding.get(col)? {
                Term::Iri(iri) => Some(Object::Iri(iri.clone())),
                Term::BlankNode(bn) => Some(Object::BlankNode(bn.clone())),
                Term::Literal(lit) => Some(Object::Literal(lit.clone())),
            },
            TemplateTerm::ConstantIri(iri) => Some(Object::Iri(Iri::new_unchecked(iri.clone()))),
            TemplateTerm::ConstantLiteral {
                value,
                datatype,
                language,
            } => {
                let literal = if let Some(lang) = language {
                    Literal::with_language(value.clone(), lang.clone())
                        .unwrap_or_else(|_| Literal::new(value.clone()))
                } else if let Some(datatype) = datatype {
                    Literal::with_datatype(value.clone(), Iri::new_unchecked(datatype.clone()))
                } else {
                    Literal::new(value.clone())
                };
                Some(Object::Literal(literal))
            }
            TemplateTerm::BlankNode(label) => Some(Object::BlankNode(BlankNode::new(format!(
                "{label}_{row_id}"
            )))),
        }
    }

    /// Convert Cypher result to ASK result
    #[must_use]
    pub fn to_ask_result(cypher_result: CypherResult) -> AskResult {
        // Look for the boolean result in first row, first column
        let result = cypher_result
            .rows
            .first()
            .and_then(|row| row.first())
            .is_some_and(|val| match val {
                CypherValue::Bool(b) => *b,
                CypherValue::Integer(i) => *i > 0,
                CypherValue::String(s) => s == "true" || s == "1",
                _ => false,
            });

        AskResult::new(result)
    }
}

/// Query executor that handles the full execution pipeline
pub struct QueryExecutor<E: CypherExecutor> {
    /// Cypher executor
    executor: E,
    /// Configuration
    config: QueryConfig,
}

impl<E: CypherExecutor> QueryExecutor<E> {
    /// Create a new query executor
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            config: QueryConfig::default(),
        }
    }

    /// Create with custom configuration
    pub const fn with_config(executor: E, config: QueryConfig) -> Self {
        Self { executor, config }
    }

    /// Execute a translated Cypher query
    pub fn execute(&self, cypher_query: &CypherQuery) -> QueryResult {
        let start = Instant::now();

        // Execute with timeout if configured
        let result = if let Some(timeout) = self.config.timeout {
            // Check if we've already exceeded timeout before executing
            if start.elapsed() > timeout {
                return QueryResult::Error(QueryError {
                    message: "Query timeout before execution".into(),
                    code: Some("TIMEOUT".into()),
                    query: Some(cypher_query.query.clone()),
                });
            }
            self.executor
                .execute_with_timeout(&cypher_query.query, timeout)
        } else {
            self.executor.execute(&cypher_query.query)
        };

        match result {
            Ok(cypher_result) => {
                // Check timeout after execution
                if let Some(timeout) = self.config.timeout {
                    if start.elapsed() > timeout {
                        return QueryResult::Error(QueryError {
                            message: "Query timeout during execution".into(),
                            code: Some("TIMEOUT".into()),
                            query: Some(cypher_query.query.clone()),
                        });
                    }
                }

                // Convert based on query type
                match cypher_query.query_type {
                    CypherQueryType::Select => {
                        let select_results =
                            ResultConverter::to_select_results(cypher_query, cypher_result);
                        QueryResult::Select(select_results)
                    }
                    CypherQueryType::Ask => {
                        let ask_result = ResultConverter::to_ask_result(cypher_result);
                        QueryResult::Ask(ask_result)
                    }
                    CypherQueryType::Construct => {
                        let construct_results =
                            ResultConverter::to_construct_results(cypher_query, cypher_result, 0);
                        QueryResult::Construct(construct_results)
                    }
                }
            }
            Err(e) => QueryResult::Error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor {
        result: CypherResult,
    }

    impl CypherExecutor for MockExecutor {
        fn execute(&self, _query: &str) -> Result<CypherResult, QueryError> {
            Ok(self.result.clone())
        }

        fn execute_with_timeout(
            &self,
            query: &str,
            _timeout: Duration,
        ) -> Result<CypherResult, QueryError> {
            self.execute(query)
        }
    }

    #[test]
    fn test_cypher_value_to_term() {
        let uri = CypherValue::String("http://example.org/test".to_string());
        let term = uri.to_term().unwrap();
        assert!(term.is_iri());

        let blank = CypherValue::String("_:b1".to_string());
        let term = blank.to_term().unwrap();
        assert!(term.is_blank_node());

        let literal = CypherValue::String("hello".to_string());
        let term = literal.to_term().unwrap();
        assert!(term.is_literal());

        let int = CypherValue::Integer(42);
        let term = int.to_term().unwrap();
        assert!(term.is_literal());
        assert_eq!(term.value(), "42");
    }

    #[test]
    fn test_node_to_term() {
        let mut props = HashMap::new();
        props.insert(
            "uri".to_string(),
            CypherValue::String("http://example.org/Alice".to_string()),
        );
        let node = CypherValue::Node(props);

        let term = node.to_term().unwrap();
        assert!(term.is_iri());
        assert_eq!(term.value(), "http://example.org/Alice");
    }

    #[test]
    fn test_select_result_conversion() {
        let cypher_query = CypherQuery {
            query: "MATCH ...".to_string(),
            variables: vec!["s".to_string(), "p".to_string(), "o".to_string()],
            query_type: CypherQueryType::Select,
            construct_template: None,
        };

        let cypher_result = CypherResult {
            columns: vec!["s".to_string(), "p".to_string(), "o".to_string()],
            rows: vec![vec![
                CypherValue::String("http://example.org/Alice".to_string()),
                CypherValue::String("http://example.org/knows".to_string()),
                CypherValue::String("http://example.org/Bob".to_string()),
            ]],
            stats: None,
        };

        let results = ResultConverter::to_select_results(&cypher_query, cypher_result);
        assert_eq!(results.bindings.len(), 1);
    }

    #[test]
    fn test_ask_result_conversion() {
        let cypher_result = CypherResult {
            columns: vec!["result".to_string()],
            rows: vec![vec![CypherValue::Bool(true)]],
            stats: None,
        };

        let result = ResultConverter::to_ask_result(cypher_result);
        assert!(result.result);
    }

    #[test]
    fn test_query_executor() {
        let mock_result = CypherResult {
            columns: vec!["s".to_string()],
            rows: vec![vec![CypherValue::String(
                "http://example.org/Alice".to_string(),
            )]],
            stats: None,
        };

        let executor = MockExecutor {
            result: mock_result,
        };
        let query_executor = QueryExecutor::new(executor);

        let cypher_query = CypherQuery {
            query: "MATCH (s) RETURN s".to_string(),
            variables: vec!["s".to_string()],
            query_type: CypherQueryType::Select,
            construct_template: None,
        };

        let result = query_executor.execute(&cypher_query);
        assert!(matches!(result, QueryResult::Select(_)));
    }

    #[test]
    fn test_query_config() {
        let config = QueryConfig::new()
            .with_timeout(Duration::from_secs(60))
            .with_max_results(100);

        assert_eq!(config.timeout, Some(Duration::from_secs(60)));
        assert_eq!(config.max_results, Some(100));
    }
}
