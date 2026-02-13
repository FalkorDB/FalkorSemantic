//! SPARQL Query Parser
//!
//! Main parser implementation that wraps spargebra.

use oxiri::Iri;

use super::ast::{
    AskQuery, ConstructQuery, DescribeQuery, Expression, GraphPattern, LiteralPattern, NamedNode,
    OrderCondition, Query, QueryDataset, SelectQuery, TermPattern, TriplePattern, Variable,
};
use super::error::{SparqlError, SparqlResult};
use super::prefixes::PrefixMap;
use super::validation::QueryValidator;

/// SPARQL query parser
///
/// Parses SPARQL query strings into structured Query objects.
#[derive(Debug, Default)]
pub struct SparqlParser {
    /// Base IRI for resolving relative references
    base_iri: Option<String>,
    /// Whether to validate queries after parsing
    validate: bool,
    /// Custom validator
    validator: Option<QueryValidator>,
}

impl SparqlParser {
    /// Create a new SPARQL parser
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base IRI for resolving relative references
    pub fn with_base_iri(mut self, base: impl Into<String>) -> Self {
        self.base_iri = Some(base.into());
        self
    }

    /// Enable query validation after parsing
    #[must_use]
    pub const fn with_validation(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    /// Set a custom validator
    #[must_use]
    pub const fn with_validator(mut self, validator: QueryValidator) -> Self {
        self.validator = Some(validator);
        self.validate = true;
        self
    }

    /// Parse a SPARQL query string
    pub fn parse(&self, query: &str) -> SparqlResult<Query> {
        let base = self.base_iri.as_deref();
        self.parse_impl(query, base)
    }

    /// Parse a SPARQL query with an explicit base IRI
    pub fn parse_with_base(&self, query: &str, base: &str) -> SparqlResult<Query> {
        self.parse_impl(query, Some(base))
    }

    /// Internal parse implementation
    fn parse_impl(&self, query: &str, base: Option<&str>) -> SparqlResult<Query> {
        // Parse with spargebra using the new API
        let spargebra_query = if let Some(base_str) = base {
            let base_iri = Iri::parse(base_str)
                .map_err(|e| SparqlError::parse(format!("Invalid base IRI: {e}")))?;
            spargebra::SparqlParser::new()
                .with_base_iri(base_iri.as_str())
                .map_err(|e| SparqlError::parse(format!("Invalid base IRI: {e}")))?
                .parse_query(query)
                .map_err(SparqlError::from)?
        } else {
            spargebra::SparqlParser::new()
                .parse_query(query)
                .map_err(SparqlError::from)?
        };

        // Convert to our AST
        let query = convert_query(spargebra_query)?;

        // Validate if enabled
        if self.validate {
            let validator = self.validator.clone().unwrap_or_default();
            validator.validate(&query)?;
        }

        Ok(query)
    }

    /// Parse and extract prefixes from query prologue
    pub fn parse_with_prefixes(&self, query: &str) -> SparqlResult<(Query, PrefixMap)> {
        let parsed_query = self.parse(query)?;
        let prefixes = extract_prefixes_from_query(query);
        Ok((parsed_query, prefixes))
    }

    /// Check if a query string is syntactically valid
    #[must_use]
    pub fn is_valid(&self, query: &str) -> bool {
        self.parse(query).is_ok()
    }

    /// Get the query type without full parsing
    #[must_use]
    pub fn query_type(query: &str) -> Option<QueryType> {
        let trimmed = query.trim_start();

        // Skip prefixes and base declarations
        let mut pos = 0;
        let lower = trimmed.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();

        while pos < chars.len() {
            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }

            // Check for PREFIX or BASE
            let remaining = &lower[pos..];
            if remaining.starts_with("prefix") || remaining.starts_with("base") {
                // Skip to end of line or next declaration
                while pos < chars.len() && chars[pos] != '\n' {
                    if chars[pos] == '>' {
                        pos += 1;
                        break;
                    }
                    pos += 1;
                }
            } else {
                break;
            }
        }

        let remaining = &lower[pos..];
        if remaining.starts_with("select") {
            Some(QueryType::Select)
        } else if remaining.starts_with("construct") {
            Some(QueryType::Construct)
        } else if remaining.starts_with("ask") {
            Some(QueryType::Ask)
        } else if remaining.starts_with("describe") {
            Some(QueryType::Describe)
        } else {
            None
        }
    }
}

/// Query type for quick identification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    Select,
    Construct,
    Ask,
    Describe,
}

/// Convert a spargebra Query to our Query type
fn convert_query(query: spargebra::Query) -> SparqlResult<Query> {
    match &query {
        spargebra::Query::Select {
            pattern,
            dataset,
            base_iri: _,
        } => {
            let (
                projection,
                distinct,
                reduced,
                order_by,
                limit,
                offset,
                group_by,
                having,
                inner_pattern,
            ) = extract_select_modifiers(pattern);

            Ok(Query::Select(SelectQuery {
                inner: query.clone(),
                projection,
                distinct,
                reduced,
                pattern: GraphPattern {
                    inner: inner_pattern,
                },
                dataset: convert_dataset(dataset),
                order_by,
                limit,
                offset,
                group_by,
                having,
            }))
        }
        spargebra::Query::Construct {
            template,
            pattern,
            dataset,
            base_iri: _,
        } => Ok(Query::Construct(ConstructQuery {
            inner: query.clone(),
            template: convert_template(template),
            pattern: GraphPattern {
                inner: pattern.clone(),
            },
            dataset: convert_dataset(dataset),
        })),
        spargebra::Query::Ask {
            pattern,
            dataset,
            base_iri: _,
        } => Ok(Query::Ask(AskQuery {
            inner: query.clone(),
            pattern: GraphPattern {
                inner: pattern.clone(),
            },
            dataset: convert_dataset(dataset),
        })),
        spargebra::Query::Describe {
            pattern,
            dataset,
            base_iri: _,
        } => {
            let (resources, inner_pattern) = extract_describe_resources(pattern);
            Ok(Query::Describe(DescribeQuery {
                inner: query.clone(),
                resources,
                pattern: GraphPattern {
                    inner: inner_pattern,
                },
                dataset: convert_dataset(dataset),
            }))
        }
    }
}

/// Return type for extracted SELECT modifiers
type SelectModifiers = (
    Option<Vec<Variable>>,
    bool,
    bool,
    Option<Vec<OrderCondition>>,
    Option<usize>,
    Option<usize>,
    Option<Vec<Variable>>,
    Option<Expression>,
    spargebra::algebra::GraphPattern,
);

/// Extract SELECT modifiers from a graph pattern
fn extract_select_modifiers(pattern: &spargebra::algebra::GraphPattern) -> SelectModifiers {
    use spargebra::algebra::GraphPattern as GP;

    let mut projection = None;
    let mut distinct = false;
    let mut reduced = false;
    let mut order_by = None;
    let mut limit = None;
    let mut offset = None;
    let mut group_by = None;
    #[allow(unused_mut)]
    let mut having = None;
    let mut current = pattern.clone();

    loop {
        match &current {
            GP::Project { inner, variables } => {
                projection = Some(
                    variables
                        .iter()
                        .map(|v| Variable::from(v.clone()))
                        .collect(),
                );
                current = (**inner).clone();
            }
            GP::Distinct { inner } => {
                distinct = true;
                current = (**inner).clone();
            }
            GP::Reduced { inner } => {
                reduced = true;
                current = (**inner).clone();
            }
            GP::OrderBy { inner, expression } => {
                order_by = Some(
                    expression
                        .iter()
                        .map(|cond| match cond {
                            spargebra::algebra::OrderExpression::Asc(e) => OrderCondition {
                                expression: Expression { inner: e.clone() },
                                descending: false,
                            },
                            spargebra::algebra::OrderExpression::Desc(e) => OrderCondition {
                                expression: Expression { inner: e.clone() },
                                descending: true,
                            },
                        })
                        .collect(),
                );
                current = (**inner).clone();
            }
            GP::Slice {
                inner,
                start,
                length,
            } => {
                if *start > 0 {
                    offset = Some(*start);
                }
                limit = *length;
                current = (**inner).clone();
            }
            GP::Group {
                inner,
                variables,
                aggregates,
            } => {
                group_by = Some(
                    variables
                        .iter()
                        .map(|v| Variable::from(v.clone()))
                        .collect(),
                );
                // Check for HAVING (would be in aggregates)
                let _ = aggregates; // TODO: extract HAVING clause
                current = (**inner).clone();
            }
            _ => break,
        }
    }

    (
        projection, distinct, reduced, order_by, limit, offset, group_by, having, current,
    )
}

/// Extract DESCRIBE resources
fn extract_describe_resources(
    pattern: &spargebra::algebra::GraphPattern,
) -> (Vec<TermPattern>, spargebra::algebra::GraphPattern) {
    use spargebra::algebra::GraphPattern as GP;

    // DESCRIBE wraps resources in Project
    if let GP::Project { inner, variables } = pattern {
        let resources = variables
            .iter()
            .map(|v| TermPattern::Variable(Variable::from(v.clone())))
            .collect();
        return (resources, (**inner).clone());
    }

    // Fallback: no explicit resources
    (vec![], pattern.clone())
}

/// Convert spargebra dataset to our type
fn convert_dataset(dataset: &Option<spargebra::algebra::QueryDataset>) -> Option<QueryDataset> {
    // For now, just check if dataset is present
    // Full conversion would require more complex handling
    if dataset.is_some() {
        Some(QueryDataset::default())
    } else {
        None
    }
}

/// Convert CONSTRUCT template
fn convert_template(template: &[spargebra::term::TriplePattern]) -> Vec<TriplePattern> {
    template
        .iter()
        .map(|t| TriplePattern {
            subject: convert_term_pattern(&t.subject),
            predicate: convert_named_node_pattern(&t.predicate),
            object: convert_term_pattern(&t.object),
        })
        .collect()
}

/// Convert a term pattern
fn convert_term_pattern(tp: &spargebra::term::TermPattern) -> TermPattern {
    match tp {
        spargebra::term::TermPattern::Variable(v) => {
            TermPattern::Variable(Variable::from(v.clone()))
        }
        spargebra::term::TermPattern::NamedNode(n) => {
            TermPattern::NamedNode(NamedNode::from(n.clone()))
        }
        spargebra::term::TermPattern::BlankNode(b) => {
            TermPattern::BlankNode(b.as_str().to_string())
        }
        spargebra::term::TermPattern::Literal(l) => {
            let lexical = l.value().to_string();
            let language = l.language().map(std::string::ToString::to_string);
            let datatype = if language.is_none() && l.to_string().contains("^^<") {
                Some(l.datatype().as_str().to_string())
            } else {
                None
            };
            TermPattern::Literal(LiteralPattern {
                value: lexical,
                datatype,
                language,
            })
        }
    }
}

/// Convert a named node pattern
fn convert_named_node_pattern(np: &spargebra::term::NamedNodePattern) -> TermPattern {
    match np {
        spargebra::term::NamedNodePattern::Variable(v) => {
            TermPattern::Variable(Variable::from(v.clone()))
        }
        spargebra::term::NamedNodePattern::NamedNode(n) => {
            TermPattern::NamedNode(NamedNode::from(n.clone()))
        }
    }
}

/// Extract prefixes from query string
fn extract_prefixes_from_query(query: &str) -> PrefixMap {
    let mut prefixes = PrefixMap::new();
    let lower = query.to_lowercase();

    for line in query.lines() {
        let trimmed = line.trim();
        let lower_line = trimmed.to_lowercase();

        if lower_line.starts_with("prefix") {
            // Parse PREFIX declaration
            if let Some(rest) = trimmed.get(6..) {
                let rest = rest.trim();
                if let Some(colon_pos) = rest.find(':') {
                    let prefix = rest[..colon_pos].trim();
                    let after_colon = rest[colon_pos + 1..].trim();
                    if after_colon.starts_with('<') && after_colon.ends_with('>') {
                        let iri = &after_colon[1..after_colon.len() - 1];
                        prefixes.add(prefix, iri);
                    }
                }
            }
        } else if lower_line.starts_with("base") {
            // Parse BASE declaration
            if let Some(rest) = trimmed.get(4..) {
                let rest = rest.trim();
                if rest.starts_with('<') && rest.ends_with('>') {
                    let iri = &rest[1..rest.len() - 1];
                    prefixes.set_base(iri);
                }
            }
        }
    }

    // Also check for inline prefix patterns
    let _ = lower; // Use to avoid warning

    prefixes
}

/// Convenience function to parse a SPARQL query
pub fn parse_sparql(query: &str) -> SparqlResult<Query> {
    SparqlParser::new().parse(query)
}

/// Convenience function to parse a SPARQL query with base IRI
pub fn parse_sparql_with_base(query: &str, base: &str) -> SparqlResult<Query> {
    SparqlParser::new().parse_with_base(query, base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        assert!(parsed.is_select());
        let select = parsed.as_select().unwrap();
        assert!(!select.is_select_all());

        let vars: Vec<_> = select.projected_variables();
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_parse_select_star() {
        let query = "SELECT * WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        let select = parsed.as_select().unwrap();
        // spargebra normalizes SELECT * to include all variables explicitly
        // so we check that all pattern variables are projected
        let vars = select.projected_variables();
        assert!(vars.len() >= 3); // At least ?s, ?p, ?o
    }

    #[test]
    fn test_parse_select_distinct() {
        let query = "SELECT DISTINCT ?s WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        let select = parsed.as_select().unwrap();
        assert!(select.distinct);
        assert!(!select.reduced);
    }

    #[test]
    fn test_parse_select_with_limit_offset() {
        let query = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 OFFSET 5";
        let parsed = parse_sparql(query).unwrap();

        let select = parsed.as_select().unwrap();
        assert_eq!(select.limit, Some(10));
        assert_eq!(select.offset, Some(5));
    }

    #[test]
    fn test_parse_select_with_order_by() {
        let query = "SELECT ?s ?p WHERE { ?s ?p ?o } ORDER BY ?s DESC(?p)";
        let parsed = parse_sparql(query).unwrap();

        let select = parsed.as_select().unwrap();
        let order_by = select.order_by.as_ref().unwrap();
        assert_eq!(order_by.len(), 2);
        assert!(!order_by[0].descending);
        assert!(order_by[1].descending);
    }

    #[test]
    fn test_parse_construct() {
        let query = "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        assert!(parsed.is_construct());
        let construct = parsed.as_construct().unwrap();
        assert_eq!(construct.template.len(), 1);
    }

    #[test]
    fn test_parse_construct_literal_metadata() {
        let query = r#"CONSTRUCT { ?s <http://example.org/p> "42"^^<http://www.w3.org/2001/XMLSchema#integer> ; <http://example.org/label> "hello"@en } WHERE { ?s ?p ?o }"#;
        let parsed = parse_sparql(query).unwrap();
        let construct = parsed.as_construct().unwrap();
        assert_eq!(construct.template.len(), 2);

        match &construct.template[0].object {
            TermPattern::Literal(lit) => {
                assert_eq!(lit.value, "42");
                assert_eq!(
                    lit.datatype.as_deref(),
                    Some("http://www.w3.org/2001/XMLSchema#integer")
                );
                assert!(lit.language.is_none());
            }
            other => panic!("Expected literal object in first template triple, got: {other:?}"),
        }

        match &construct.template[1].object {
            TermPattern::Literal(lit) => {
                assert_eq!(lit.value, "hello");
                assert_eq!(lit.language.as_deref(), Some("en"));
                assert!(lit.datatype.is_none());
            }
            other => panic!("Expected literal object in second template triple, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_ask() {
        let query = "ASK { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        assert!(parsed.is_ask());
    }

    #[test]
    fn test_parse_describe() {
        let query = "DESCRIBE ?s WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        assert!(parsed.is_describe());
    }

    #[test]
    fn test_parse_with_prefixes() {
        let query = r#"
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            SELECT ?name WHERE { ?s foaf:name ?name }
        "#;

        let parser = SparqlParser::new();
        let (parsed, prefixes) = parser.parse_with_prefixes(query).unwrap();

        assert!(parsed.is_select());
        assert!(prefixes.contains("foaf"));
        assert!(prefixes.contains("rdf"));
    }

    #[test]
    fn test_parse_with_base() {
        let query = "SELECT * WHERE { <subject> ?p ?o }";
        let parsed = parse_sparql_with_base(query, "http://example.org/").unwrap();

        assert!(parsed.is_select());
    }

    #[test]
    fn test_parse_error() {
        let query = "SLECT ?s WHERE { ?s ?p ?o }"; // Typo in SELECT
        let result = parse_sparql(query);

        assert!(result.is_err());
    }

    #[test]
    fn test_query_type_detection() {
        assert_eq!(
            SparqlParser::query_type("SELECT ?s WHERE { ?s ?p ?o }"),
            Some(QueryType::Select)
        );
        assert_eq!(
            SparqlParser::query_type("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }"),
            Some(QueryType::Construct)
        );
        assert_eq!(
            SparqlParser::query_type("ASK { ?s ?p ?o }"),
            Some(QueryType::Ask)
        );
        assert_eq!(
            SparqlParser::query_type("DESCRIBE ?s WHERE { ?s ?p ?o }"),
            Some(QueryType::Describe)
        );
        assert_eq!(SparqlParser::query_type("invalid query"), None);
    }

    #[test]
    fn test_query_type_with_prefixes() {
        let query = "PREFIX ex: <http://example.org/>\nSELECT ?s WHERE { ?s ?p ?o }";
        assert_eq!(SparqlParser::query_type(query), Some(QueryType::Select));
    }

    #[test]
    fn test_query_variables() {
        let query = "SELECT ?s ?p WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        let vars = parsed.variables();
        assert!(vars.iter().any(|v| v.name == "s"));
        assert!(vars.iter().any(|v| v.name == "p"));
        assert!(vars.iter().any(|v| v.name == "o"));
    }

    #[test]
    fn test_query_display() {
        let query = "SELECT ?s WHERE { ?s ?p ?o }";
        let parsed = parse_sparql(query).unwrap();

        // Should be able to serialize back
        let serialized = format!("{}", parsed);
        assert!(serialized.contains("SELECT"));
    }

    #[test]
    fn test_complex_query() {
        let query = r#"
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            SELECT DISTINCT ?name ?age
            WHERE {
                ?person foaf:name ?name .
                ?person foaf:age ?age .
                FILTER (?age > 18)
            }
            ORDER BY ?age
            LIMIT 100
        "#;

        let parsed = parse_sparql(query).unwrap();
        let select = parsed.as_select().unwrap();

        assert!(select.distinct);
        assert_eq!(select.limit, Some(100));
        assert!(select.order_by.is_some());
    }

    #[test]
    fn test_optional_pattern() {
        let query = r#"
            SELECT ?name ?email
            WHERE {
                ?s <http://example.org/name> ?name .
                OPTIONAL { ?s <http://example.org/email> ?email }
            }
        "#;

        let parsed = parse_sparql(query).unwrap();
        assert!(parsed.is_select());
    }

    #[test]
    fn test_union_pattern() {
        let query = r#"
            SELECT ?name
            WHERE {
                { ?s <http://example.org/firstName> ?name }
                UNION
                { ?s <http://example.org/lastName> ?name }
            }
        "#;

        let parsed = parse_sparql(query).unwrap();
        assert!(parsed.is_select());
    }

    #[test]
    fn test_values_clause() {
        let query = r#"
            SELECT ?s ?name
            WHERE {
                VALUES ?s { <http://example.org/1> <http://example.org/2> }
                ?s <http://example.org/name> ?name
            }
        "#;

        let parsed = parse_sparql(query).unwrap();
        assert!(parsed.is_select());
    }

    #[test]
    fn test_subquery() {
        let query = r#"
            SELECT ?name ?count
            WHERE {
                {
                    SELECT ?person (COUNT(?friend) AS ?count)
                    WHERE { ?person <http://example.org/knows> ?friend }
                    GROUP BY ?person
                }
                ?person <http://example.org/name> ?name
            }
        "#;

        let parsed = parse_sparql(query).unwrap();
        assert!(parsed.is_select());
    }
}
