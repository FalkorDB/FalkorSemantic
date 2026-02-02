//! RDF.INSERT Command Implementation
//!
//! Inserts RDF data into a FalkorDB graph with support for multiple formats,
//! batch processing, and atomic transactions.

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};

use falkorsemantic_mapper::Mapper;
use falkorsemantic_parser::formats::NTriplesReader;
use falkorsemantic_parser::rdf::Triple;
use falkorsemantic_parser::TurtleParser;

/// Supported RDF formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RdfFormat {
    /// Turtle format
    Turtle,
    /// N-Triples format
    NTriples,
    /// N-Quads format
    NQuads,
    /// JSON-LD format (not yet implemented)
    JsonLd,
}

impl RdfFormat {
    /// Parse format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "turtle" | "ttl" => Some(RdfFormat::Turtle),
            "ntriples" | "nt" => Some(RdfFormat::NTriples),
            "nquads" | "nq" => Some(RdfFormat::NQuads),
            "jsonld" | "json-ld" => Some(RdfFormat::JsonLd),
            _ => None,
        }
    }

    /// Detect format from content
    fn detect(content: &str) -> Self {
        let trimmed = content.trim();

        // Check for JSON-LD (starts with { or [)
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return RdfFormat::JsonLd;
        }

        // Check for Turtle indicators (@prefix, @base, PREFIX, BASE)
        if trimmed.starts_with("@prefix")
            || trimmed.starts_with("@base")
            || trimmed.starts_with("PREFIX")
            || trimmed.starts_with("BASE")
        {
            return RdfFormat::Turtle;
        }

        // Check for prefixed names (common in Turtle)
        if trimmed.contains(":") && !trimmed.starts_with('<') {
            // Look for patterns like "prefix:local" that aren't in angle brackets
            let first_line = trimmed.lines().next().unwrap_or("");
            if first_line.contains(':') && !first_line.starts_with('<') {
                // Could be Turtle with prefixed names
                return RdfFormat::Turtle;
            }
        }

        // Check for N-Quads (4 components per line)
        for line in trimmed.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // N-Quads have 4 components: subject, predicate, object, graph
            // Count the number of '>' or '"' endings followed by spaces
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && line.ends_with('.') {
                // Check if 4th element looks like a graph (IRI or blank node)
                let fourth = parts.get(3).unwrap_or(&"");
                if fourth.starts_with('<') || fourth.starts_with("_:") {
                    return RdfFormat::NQuads;
                }
            }
            break; // Only check first non-empty line
        }

        // Default to N-Triples (simplest format)
        RdfFormat::NTriples
    }
}

/// Statistics from an RDF insert operation
#[derive(Debug, Default)]
struct InsertStats {
    /// Number of triples parsed
    triples_parsed: usize,
    /// Number of Cypher statements generated
    statements_generated: usize,
    /// Number of statements executed successfully
    statements_executed: usize,
    /// Number of errors encountered
    errors: usize,
    /// Error messages (if any)
    error_messages: Vec<String>,
}

/// Parse command arguments
struct InsertArgs<'a> {
    graph_key: &'a str,
    data: &'a str,
    format: Option<RdfFormat>,
    atomic: bool,
}

impl<'a> InsertArgs<'a> {
    fn parse(args: &'a [RedisString]) -> Result<Self, RedisError> {
        // args[0] is the command name
        if args.len() < 3 {
            return Err(RedisError::WrongArity);
        }

        let graph_key = args[1]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid graph key".into()))?;
        let data = args[2]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid data".into()))?;

        let mut format = None;
        let mut atomic = false;

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            let arg = args[i]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid argument".into()))?;

            match arg.to_uppercase().as_str() {
                "FORMAT" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String(
                            "FORMAT requires a value (turtle, ntriples, nquads, jsonld)".into(),
                        ));
                    }
                    let fmt_str = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid format value".into()))?;
                    format = Some(RdfFormat::from_str(fmt_str).ok_or_else(|| {
                        RedisError::String(format!(
                            "Unknown format '{}'. Use: turtle, ntriples, nquads, jsonld",
                            fmt_str
                        ))
                    })?);
                }
                "ATOMIC" => {
                    atomic = true;
                }
                _ => {
                    return Err(RedisError::String(format!("Unknown argument: {}", arg)));
                }
            }
            i += 1;
        }

        Ok(InsertArgs {
            graph_key,
            data,
            format,
            atomic,
        })
    }
}

/// Parse RDF data into triples
fn parse_rdf(data: &str, format: RdfFormat) -> Result<Vec<Triple>, String> {
    match format {
        RdfFormat::Turtle => {
            let mut parser = TurtleParser::new();
            parser.parse(data).map_err(|e| e.to_string())
        }
        RdfFormat::NTriples => {
            let reader = NTriplesReader::new();
            reader.parse_all_str(data).map_err(|e| e.to_string())
        }
        RdfFormat::NQuads => {
            // For now, parse as triples (ignoring graph component)
            // TODO: Implement proper N-Quads support with named graphs
            let reader = NTriplesReader::new();
            reader.parse_all_str(data).map_err(|e| e.to_string())
        }
        RdfFormat::JsonLd => Err("JSON-LD parsing not yet implemented".into()),
    }
}

/// Execute Cypher statements against FalkorDB
fn execute_cypher(
    ctx: &Context,
    graph_key: &str,
    statements: &[String],
    atomic: bool,
    triples_parsed: usize,
) -> InsertStats {
    let mut stats = InsertStats {
        triples_parsed,
        statements_generated: statements.len(),
        ..Default::default()
    };

    if statements.is_empty() {
        return stats;
    }

    // Build combined query for atomic execution or individual queries
    let queries: Vec<String> = if atomic {
        // Combine all statements into a single transaction
        let combined = statements.join("\n");
        vec![combined]
    } else {
        statements.to_vec()
    };

    for query in &queries {
        // Execute GRAPH.QUERY command
        let result = ctx.call("GRAPH.QUERY", &[graph_key, query.as_str()]);

        match result {
            Ok(_) => {
                if atomic {
                    stats.statements_executed = statements.len();
                } else {
                    stats.statements_executed += 1;
                }
            }
            Err(e) => {
                stats.errors += 1;
                stats.error_messages.push(format!("Query error: {:?}", e));
                if atomic {
                    // Stop on first error in atomic mode
                    break;
                }
            }
        }
    }

    stats
}

/// RDF.INSERT command handler
///
/// Syntax: RDF.INSERT <graph_key> <data> [FORMAT turtle|ntriples|nquads|jsonld] [ATOMIC]
///
/// Arguments:
/// - graph_key: The FalkorDB graph name to insert into
/// - data: RDF data as a string
/// - FORMAT: Optional format specifier (auto-detected if not provided)
/// - ATOMIC: Optional flag to execute all inserts as a single transaction
///
/// Returns: Array with [triples_parsed, statements_executed, errors]
pub fn rdf_insert(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    // Parse arguments
    let parsed_args = InsertArgs::parse(&args)?;

    log::debug!(
        "RDF.INSERT graph={} format={:?} atomic={}",
        parsed_args.graph_key,
        parsed_args.format,
        parsed_args.atomic
    );

    // Detect or use specified format
    let format = parsed_args
        .format
        .unwrap_or_else(|| RdfFormat::detect(parsed_args.data));

    log::debug!("Using format: {:?}", format);

    // Parse RDF data
    let triples = match parse_rdf(parsed_args.data, format) {
        Ok(t) => t,
        Err(e) => {
            return Err(RedisError::String(format!("Parse error: {}", e)));
        }
    };

    let triples_count = triples.len();
    log::debug!("Parsed {} triples", triples_count);

    if triples.is_empty() {
        // Return early if no triples
        return Ok(RedisValue::Array(vec![
            RedisValue::Integer(0),
            RedisValue::Integer(0),
            RedisValue::Integer(0),
        ]));
    }

    // Map triples to Cypher statements
    let mapper = Mapper::new();
    let statements = match mapper.map_triples(&triples) {
        Ok(s) => s,
        Err(e) => {
            return Err(RedisError::String(format!("Mapping error: {}", e)));
        }
    };

    log::debug!("Generated {} Cypher statements", statements.len());

    // Execute Cypher against FalkorDB
    let stats = execute_cypher(ctx, parsed_args.graph_key, &statements, parsed_args.atomic, triples_count);

    log::debug!(
        "Insert complete: parsed={}, generated={}, executed={}, errors={}",
        stats.triples_parsed,
        stats.statements_generated,
        stats.statements_executed,
        stats.errors
    );

    // Return statistics as array [triples_parsed, statements_executed, errors]
    Ok(RedisValue::Array(vec![
        RedisValue::Integer(triples_count as i64),
        RedisValue::Integer(stats.statements_executed as i64),
        RedisValue::Integer(stats.errors as i64),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_str() {
        assert_eq!(RdfFormat::from_str("turtle"), Some(RdfFormat::Turtle));
        assert_eq!(RdfFormat::from_str("TTL"), Some(RdfFormat::Turtle));
        assert_eq!(RdfFormat::from_str("ntriples"), Some(RdfFormat::NTriples));
        assert_eq!(RdfFormat::from_str("NT"), Some(RdfFormat::NTriples));
        assert_eq!(RdfFormat::from_str("nquads"), Some(RdfFormat::NQuads));
        assert_eq!(RdfFormat::from_str("jsonld"), Some(RdfFormat::JsonLd));
        assert_eq!(RdfFormat::from_str("json-ld"), Some(RdfFormat::JsonLd));
        assert_eq!(RdfFormat::from_str("unknown"), None);
    }

    #[test]
    fn test_format_detection_turtle() {
        let turtle = "@prefix ex: <http://example.org/> .\nex:s ex:p ex:o .";
        assert_eq!(RdfFormat::detect(turtle), RdfFormat::Turtle);

        let turtle2 = "PREFIX ex: <http://example.org/>\nex:s ex:p ex:o .";
        assert_eq!(RdfFormat::detect(turtle2), RdfFormat::Turtle);
    }

    #[test]
    fn test_format_detection_ntriples() {
        let nt = "<http://example.org/s> <http://example.org/p> <http://example.org/o> .";
        assert_eq!(RdfFormat::detect(nt), RdfFormat::NTriples);
    }

    #[test]
    fn test_format_detection_jsonld() {
        let jsonld = r#"{"@context": {}, "@id": "http://example.org/s"}"#;
        assert_eq!(RdfFormat::detect(jsonld), RdfFormat::JsonLd);

        let jsonld_array = r#"[{"@id": "http://example.org/s"}]"#;
        assert_eq!(RdfFormat::detect(jsonld_array), RdfFormat::JsonLd);
    }

    #[test]
    fn test_parse_turtle() {
        let turtle = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:predicate ex:object .
        "#;
        let triples = parse_rdf(turtle, RdfFormat::Turtle).unwrap();
        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn test_parse_ntriples() {
        let nt = "<http://example.org/s> <http://example.org/p> <http://example.org/o> .";
        let triples = parse_rdf(nt, RdfFormat::NTriples).unwrap();
        assert_eq!(triples.len(), 1);
    }
}
