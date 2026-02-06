//! RDF.QUERY Command Implementation
//!
//! Executes SPARQL queries against a `FalkorDB` graph and returns results
//! in various formats (JSON, XML, CSV, TSV).

use std::time::Duration;

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};

use falkorsemantic_mapper::query::{
    CypherExecutor, CypherResult, CypherValue, QueryConfig, QueryError, QueryExecutor, QueryResult,
    SparqlToCypher,
};
use falkorsemantic_parser::results::{
    ask_to_csv, ask_to_json, ask_to_tsv, ask_to_xml, select_to_csv, select_to_json, select_to_tsv,
    select_to_xml,
};
use falkorsemantic_parser::SparqlParser;

/// Supported output formats for SPARQL results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// SPARQL JSON Results Format (default)
    #[default]
    Json,
    /// SPARQL XML Results Format
    Xml,
    /// CSV Results Format
    Csv,
    /// TSV Results Format
    Tsv,
}

impl OutputFormat {
    /// Parse format from string
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            _ => None,
        }
    }
}

/// Parsed arguments for RDF.QUERY command
struct QueryArgs {
    /// Graph key
    graph_key: String,
    /// SPARQL query string
    query: String,
    /// Output format
    format: OutputFormat,
    /// Query timeout in milliseconds
    timeout_ms: Option<u64>,
}

impl QueryArgs {
    /// Parse arguments from Redis command
    fn parse(args: &[RedisString]) -> Result<Self, RedisError> {
        if args.len() < 3 {
            return Err(RedisError::WrongArity);
        }

        let graph_key = args[1].to_string_lossy();
        let query = args[2].to_string_lossy();
        let mut format = OutputFormat::Json;
        let mut timeout_ms = None;

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            let arg = args[i].to_string_lossy().to_uppercase();
            match arg.as_str() {
                "FORMAT" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("FORMAT requires a value".into()));
                    }
                    let fmt_str = args[i].to_string_lossy();
                    format = OutputFormat::from_str(&fmt_str).ok_or_else(|| {
                        RedisError::String(format!(
                            "Unknown format '{fmt_str}'. Use: json, xml, csv, tsv"
                        ))
                    })?;
                }
                "TIMEOUT" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("TIMEOUT requires a value".into()));
                    }
                    let timeout_str = args[i].to_string_lossy();
                    timeout_ms = Some(timeout_str.parse::<u64>().map_err(|_| {
                        RedisError::String(format!(
                            "Invalid timeout value '{timeout_str}'. Must be integer milliseconds"
                        ))
                    })?);
                }
                _ => {
                    return Err(RedisError::String(format!("Unknown argument: {arg}")));
                }
            }
            i += 1;
        }

        Ok(Self {
            graph_key,
            query,
            format,
            timeout_ms,
        })
    }
}

/// Redis-backed Cypher executor
struct RedisCypherExecutor<'a> {
    ctx: &'a Context,
    graph_key: &'a str,
}

impl<'a> RedisCypherExecutor<'a> {
    const fn new(ctx: &'a Context, graph_key: &'a str) -> Self {
        Self { ctx, graph_key }
    }

    /// Parse Redis result into `CypherResult`
    fn parse_result(&self, result: RedisValue) -> Result<CypherResult, QueryError> {
        // FalkorDB returns results as an array: [headers, data, stats]
        match result {
            RedisValue::Array(arr) => {
                if arr.is_empty() {
                    return Ok(CypherResult {
                        columns: vec![],
                        rows: vec![],
                        stats: None,
                    });
                }

                // First element is headers
                let columns = match arr.first() {
                    Some(RedisValue::Array(headers)) => headers
                        .iter()
                        .map(|h| match h {
                            RedisValue::SimpleString(s) | RedisValue::BulkString(s) => s.clone(),
                            _ => String::new(),
                        })
                        .collect(),
                    _ => vec![],
                };

                // Second element is data rows
                let rows = if arr.len() > 1 {
                    match &arr[1] {
                        RedisValue::Array(data_rows) => {
                            data_rows.iter().map(|row| self.parse_row(row)).collect()
                        }
                        _ => vec![],
                    }
                } else {
                    vec![]
                };

                Ok(CypherResult {
                    columns,
                    rows,
                    stats: None,
                })
            }
            _ => Err(QueryError {
                message: "Unexpected result format from FalkorDB".into(),
                code: Some("PARSE_ERROR".into()),
                query: None,
            }),
        }
    }

    /// Parse a single row from Redis result
    fn parse_row(&self, row: &RedisValue) -> Vec<CypherValue> {
        match row {
            RedisValue::Array(values) => values.iter().map(|v| self.parse_value(v)).collect(),
            _ => vec![],
        }
    }

    /// Parse a single value from Redis result
    fn parse_value(&self, value: &RedisValue) -> CypherValue {
        match value {
            RedisValue::Integer(i) => CypherValue::Integer(*i),
            RedisValue::Float(f) => CypherValue::Float(*f),
            RedisValue::SimpleString(s) | RedisValue::BulkString(s) => {
                CypherValue::String(s.clone())
            }
            RedisValue::Array(arr) => {
                // Could be a node, relationship, or list
                // Check for node/relationship structure (type indicator at index 0)
                if let Some(RedisValue::Integer(type_id)) = arr.first() {
                    // FalkorDB type indicators:
                    // 1 = Node, 2 = Relationship, 3 = Scalar, etc.
                    match type_id {
                        1 => return self.parse_node(arr),
                        2 => return self.parse_relationship(arr),
                        _ => {}
                    }
                }
                // Default: parse as list
                CypherValue::List(arr.iter().map(|v| self.parse_value(v)).collect())
            }
            RedisValue::Null => CypherValue::Null,
            _ => CypherValue::Null,
        }
    }

    /// Parse a `FalkorDB` node
    fn parse_node(&self, arr: &[RedisValue]) -> CypherValue {
        // FalkorDB node format: [type, id, labels, properties]
        let mut props = std::collections::HashMap::new();

        // Extract properties (usually at index 3)
        if let Some(RedisValue::Array(prop_arr)) = arr.get(3) {
            for chunk in prop_arr.chunks(2) {
                if let (
                    Some(RedisValue::SimpleString(k) | RedisValue::BulkString(k)),
                    Some(value),
                ) = (chunk.first(), chunk.get(1))
                {
                    props.insert(k.clone(), self.parse_value(value));
                }
            }
        }

        CypherValue::Node(props)
    }

    /// Parse a `FalkorDB` relationship
    fn parse_relationship(&self, arr: &[RedisValue]) -> CypherValue {
        // FalkorDB relationship format: [type, id, type_name, src, dest, properties]
        let mut props = std::collections::HashMap::new();

        // Extract relationship type (index 2)
        if let Some(RedisValue::SimpleString(rel_type) | RedisValue::BulkString(rel_type)) =
            arr.get(2)
        {
            props.insert(
                "predicate".to_string(),
                CypherValue::String(rel_type.clone()),
            );
        }

        // Extract properties (usually at index 5)
        if let Some(RedisValue::Array(prop_arr)) = arr.get(5) {
            for chunk in prop_arr.chunks(2) {
                if let (
                    Some(RedisValue::SimpleString(k) | RedisValue::BulkString(k)),
                    Some(value),
                ) = (chunk.first(), chunk.get(1))
                {
                    props.insert(k.clone(), self.parse_value(value));
                }
            }
        }

        CypherValue::Relationship(props)
    }
}

impl RedisCypherExecutor<'_> {
    /// Execute a GRAPH.QUERY command with the given arguments and handle errors
    fn execute_graph_query(&self, args: &[&str], query: &str) -> Result<CypherResult, QueryError> {
        let result = self.ctx.call("GRAPH.QUERY", args);

        match result {
            Ok(value) => self.parse_result(value),
            Err(e) => Err(QueryError {
                message: format!("FalkorDB error: {e:?}"),
                code: Some("EXECUTE_ERROR".into()),
                query: Some(query.to_string()),
            }),
        }
    }
}

impl CypherExecutor for RedisCypherExecutor<'_> {
    fn execute(&self, query: &str) -> Result<CypherResult, QueryError> {
        self.execute_graph_query(&[self.graph_key, query], query)
    }

    fn execute_with_timeout(
        &self,
        query: &str,
        timeout: Duration,
    ) -> Result<CypherResult, QueryError> {
        // FalkorDB supports TIMEOUT as a separate parameter to GRAPH.QUERY
        // Convert timeout to milliseconds, clamping to u64::MAX if needed
        let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
        let timeout_str = timeout_ms.to_string();

        self.execute_graph_query(&[self.graph_key, query, "TIMEOUT", &timeout_str], query)
    }
}

/// Format SELECT results to string
fn format_select_results(
    results: &falkorsemantic_parser::results::SelectResults,
    format: OutputFormat,
) -> Result<String, RedisError> {
    match format {
        OutputFormat::Json => select_to_json(results)
            .map_err(|e| RedisError::String(format!("JSON serialization error: {e}"))),
        OutputFormat::Xml => select_to_xml(results)
            .map_err(|e| RedisError::String(format!("XML serialization error: {e}"))),
        OutputFormat::Csv => select_to_csv(results)
            .map_err(|e| RedisError::String(format!("CSV serialization error: {e}"))),
        OutputFormat::Tsv => select_to_tsv(results)
            .map_err(|e| RedisError::String(format!("TSV serialization error: {e}"))),
    }
}

/// Format ASK results to string
fn format_ask_results(
    result: &falkorsemantic_parser::results::AskResult,
    format: OutputFormat,
) -> Result<String, RedisError> {
    match format {
        OutputFormat::Json => ask_to_json(result)
            .map_err(|e| RedisError::String(format!("JSON serialization error: {e}"))),
        OutputFormat::Xml => ask_to_xml(result)
            .map_err(|e| RedisError::String(format!("XML serialization error: {e}"))),
        OutputFormat::Csv => ask_to_csv(result)
            .map_err(|e| RedisError::String(format!("CSV serialization error: {e}"))),
        OutputFormat::Tsv => ask_to_tsv(result)
            .map_err(|e| RedisError::String(format!("TSV serialization error: {e}"))),
    }
}

/// RDF.QUERY command handler
///
/// Syntax: RDF.QUERY <`graph_key`> <`sparql_query`> [FORMAT json|xml|csv|tsv] [TIMEOUT ms]
///
/// Executes a SPARQL query against the specified graph and returns results
/// in the requested format.
///
/// # Arguments
/// * `graph_key` - The `FalkorDB` graph key
/// * `sparql_query` - The SPARQL query string
/// * `FORMAT` - Optional output format (default: json)
/// * `TIMEOUT` - Optional timeout in milliseconds
///
/// # Returns
/// * The query results in the specified format
///
/// # Example
/// ```
/// RDF.QUERY mygraph "SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10"
/// RDF.QUERY mygraph "SELECT * WHERE { ?s a <http://example.org/Person> }" FORMAT xml
/// RDF.QUERY mygraph "ASK { ?s ?p ?o }" FORMAT json TIMEOUT 5000
/// ```
pub fn rdf_query(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    // Parse arguments
    let query_args = QueryArgs::parse(&args)?;

    // Parse SPARQL query
    let parser = SparqlParser::new();
    let sparql_query = parser
        .parse(&query_args.query)
        .map_err(|e| RedisError::String(format!("SPARQL parse error: {e}")))?;

    // Translate to Cypher
    let translator = SparqlToCypher::new();
    let cypher_query = translator
        .translate(&sparql_query)
        .map_err(|e| RedisError::String(format!("Translation error: {e}")))?;

    // Set up executor with config
    let mut config = QueryConfig::default();
    if let Some(timeout_ms) = query_args.timeout_ms {
        config = config.with_timeout(Duration::from_millis(timeout_ms));
    }

    let executor = RedisCypherExecutor::new(ctx, &query_args.graph_key);
    let query_executor = QueryExecutor::with_config(executor, config);

    // Execute query
    let result = query_executor.execute(&cypher_query);

    // Format and return results
    match result {
        QueryResult::Select(results) => {
            let formatted = format_select_results(&results, query_args.format)?;
            Ok(RedisValue::BulkString(formatted))
        }
        QueryResult::Ask(result) => {
            let formatted = format_ask_results(&result, query_args.format)?;
            Ok(RedisValue::BulkString(formatted))
        }
        QueryResult::Error(err) => Err(RedisError::String(format!("Query error: {err}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("xml"), Some(OutputFormat::Xml));
        assert_eq!(OutputFormat::from_str("csv"), Some(OutputFormat::Csv));
        assert_eq!(OutputFormat::from_str("tsv"), Some(OutputFormat::Tsv));
        assert_eq!(OutputFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_output_format_default() {
        assert_eq!(OutputFormat::default(), OutputFormat::Json);
    }
}
