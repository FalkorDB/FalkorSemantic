//! RDF.DELETE Command Implementation
//!
//! Deletes RDF triples matching a pattern from a FalkorDB graph.
//!
//! Syntax:
//!   RDF.DELETE <graph_key> <subject> <predicate> <object> [GRAPH <named_graph>] [ORPHANS]
//!
//! Where subject, predicate, and object can be:
//!   - A full URI: <http://example.org/resource>
//!   - A prefixed name: foaf:Person (if namespace registered)
//!   - A literal (for objects): "value" or "value"@lang or "value"^^<datatype>
//!   - A wildcard: * (matches anything)
//!
//! Options:
//!   - GRAPH <named_graph>: Scope deletion to a specific named graph
//!   - ORPHANS: Also delete nodes that become orphaned (no connections)

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};

/// Redis key for tracking RDF graphs
const RDF_GRAPHS_SET: &str = "rdf:graphs";

/// A triple pattern component that can be either a specific value or a wildcard
#[derive(Debug, Clone, PartialEq)]
enum PatternComponent {
    /// Matches any value
    Wildcard,
    /// Matches a specific IRI
    Iri(String),
    /// Matches a specific literal value
    Literal {
        value: String,
        language: Option<String>,
        datatype: Option<String>,
    },
    /// Matches a blank node
    BlankNode(String),
}

impl PatternComponent {
    /// Parse a pattern component from a string
    fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        // Wildcard
        if s == "*" {
            return Ok(PatternComponent::Wildcard);
        }

        // Full IRI: <...>
        if s.starts_with('<') && s.ends_with('>') {
            let iri = &s[1..s.len() - 1];
            if iri.is_empty() {
                return Err("Empty IRI".into());
            }
            return Ok(PatternComponent::Iri(iri.to_string()));
        }

        // Blank node: _:...
        if s.starts_with("_:") {
            let label = &s[2..];
            if label.is_empty() {
                return Err("Empty blank node label".into());
            }
            return Ok(PatternComponent::BlankNode(format!("_:{}", label)));
        }

        // Literal with language tag: "value"@lang
        if s.starts_with('"') {
            // Find the closing quote
            if let Some(quote_end) = s[1..].find('"') {
                let value = s[1..quote_end + 1].to_string();
                let rest = &s[quote_end + 2..];

                if rest.starts_with('@') {
                    let lang = rest[1..].to_string();
                    return Ok(PatternComponent::Literal {
                        value,
                        language: Some(lang),
                        datatype: None,
                    });
                } else if rest.starts_with("^^") {
                    // Datatype: "value"^^<type>
                    let dtype_str = &rest[2..];
                    if dtype_str.starts_with('<') && dtype_str.ends_with('>') {
                        let datatype = dtype_str[1..dtype_str.len() - 1].to_string();
                        return Ok(PatternComponent::Literal {
                            value,
                            language: None,
                            datatype: Some(datatype),
                        });
                    }
                    return Err(format!("Invalid datatype format: {}", dtype_str));
                } else if rest.is_empty() {
                    return Ok(PatternComponent::Literal {
                        value,
                        language: None,
                        datatype: None,
                    });
                }
            }
            return Err(format!("Invalid literal format: {}", s));
        }

        // Prefixed name: prefix:localname (treat as IRI placeholder)
        if s.contains(':') && !s.starts_with(':') {
            // This is a prefixed name - will be expanded later if namespace exists
            return Ok(PatternComponent::Iri(s.to_string()));
        }

        Err(format!(
            "Invalid pattern component: {}. Use <uri>, \"literal\", _:blank, or *",
            s
        ))
    }

    /// Check if this is a wildcard
    fn is_wildcard(&self) -> bool {
        matches!(self, PatternComponent::Wildcard)
    }
}

/// Parsed delete arguments
struct DeleteArgs {
    graph_key: String,
    subject: PatternComponent,
    predicate: PatternComponent,
    object: PatternComponent,
    named_graph: Option<String>,
    delete_orphans: bool,
}

impl DeleteArgs {
    fn parse(args: &[RedisString]) -> Result<Self, RedisError> {
        // Minimum: command, graph, subject, predicate, object
        if args.len() < 5 {
            return Err(RedisError::String(
                "Usage: RDF.DELETE <graph> <subject> <predicate> <object> [GRAPH <name>] [ORPHANS]"
                    .into(),
            ));
        }

        let graph_key = args[1]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid graph key".into()))?
            .to_string();

        let subject_str = args[2]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid subject".into()))?;
        let subject = PatternComponent::parse(subject_str)
            .map_err(|e| RedisError::String(format!("Invalid subject: {}", e)))?;

        let predicate_str = args[3]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid predicate".into()))?;
        let predicate = PatternComponent::parse(predicate_str)
            .map_err(|e| RedisError::String(format!("Invalid predicate: {}", e)))?;

        let object_str = args[4]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid object".into()))?;
        let object = PatternComponent::parse(object_str)
            .map_err(|e| RedisError::String(format!("Invalid object: {}", e)))?;

        // Parse optional arguments
        let mut named_graph = None;
        let mut delete_orphans = false;
        let mut i = 5;

        while i < args.len() {
            let arg = args[i]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid argument".into()))?
                .to_uppercase();

            match arg.as_str() {
                "GRAPH" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("GRAPH requires a value".into()));
                    }
                    let graph_name = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid graph name".into()))?;
                    named_graph = Some(graph_name.to_string());
                }
                "ORPHANS" => {
                    delete_orphans = true;
                }
                _ => {
                    return Err(RedisError::String(format!("Unknown option: {}", arg)));
                }
            }
            i += 1;
        }

        Ok(DeleteArgs {
            graph_key,
            subject,
            predicate,
            object,
            named_graph,
            delete_orphans,
        })
    }
}

/// Check if a graph exists
fn graph_exists(ctx: &Context, graph_key: &str) -> Result<bool, RedisError> {
    let exists_result = ctx.call("SISMEMBER", &[RDF_GRAPHS_SET, graph_key])?;
    Ok(match exists_result {
        RedisValue::Integer(n) => n > 0,
        _ => false,
    })
}

/// Expand a prefixed name using registered namespaces
fn expand_prefix(ctx: &Context, graph_key: &str, prefixed: &str) -> Option<String> {
    if let Some(colon_pos) = prefixed.find(':') {
        let prefix = &prefixed[..colon_pos];
        let local = &prefixed[colon_pos + 1..];

        // Look up the namespace
        let ns_key = format!("rdf:ns:{}:{}", graph_key, prefix);
        if let Ok(RedisValue::SimpleString(uri)) = ctx.call("GET", &[&ns_key]) {
            return Some(format!("{}{}", uri, local));
        }
    }
    None
}

/// Escape a string for Cypher
fn escape_cypher_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Build the Cypher WHERE clause for a pattern component
fn build_where_condition(
    component: &PatternComponent,
    var_name: &str,
    property: &str,
    ctx: &Context,
    graph_key: &str,
) -> Option<String> {
    match component {
        PatternComponent::Wildcard => None,
        PatternComponent::Iri(iri) => {
            // Try to expand prefix if it looks like a prefixed name
            let expanded = if iri.contains(':') && !iri.starts_with("http") {
                expand_prefix(ctx, graph_key, iri).unwrap_or_else(|| iri.clone())
            } else {
                iri.clone()
            };
            Some(format!(
                "{}.{} = '{}'",
                var_name,
                property,
                escape_cypher_string(&expanded)
            ))
        }
        PatternComponent::BlankNode(id) => Some(format!(
            "{}.{} = '{}' AND {}.isBlank = true",
            var_name,
            property,
            escape_cypher_string(id),
            var_name
        )),
        PatternComponent::Literal { value, language, datatype } => {
            let mut conditions = vec![format!(
                "{}.value = '{}'",
                var_name,
                escape_cypher_string(value)
            )];

            if let Some(lang) = language {
                conditions.push(format!(
                    "{}.language = '{}'",
                    var_name,
                    escape_cypher_string(lang)
                ));
            }

            if let Some(dtype) = datatype {
                conditions.push(format!(
                    "{}.datatype = '{}'",
                    var_name,
                    escape_cypher_string(dtype)
                ));
            }

            Some(conditions.join(" AND "))
        }
    }
}

/// Generate the Cypher DELETE query for the pattern
fn generate_delete_query(args: &DeleteArgs, ctx: &Context) -> String {
    let mut match_clauses = Vec::new();
    let mut where_conditions = Vec::new();

    // Determine the match pattern based on what's specified
    let has_subject = !args.subject.is_wildcard();
    let has_predicate = !args.predicate.is_wildcard();
    let has_object = !args.object.is_wildcard();

    // Build MATCH clause
    match (has_subject, has_predicate, has_object) {
        // All wildcards - delete all edges
        (false, false, false) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
        }
        // Subject only
        (true, false, false) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let Some(cond) =
                build_where_condition(&args.subject, "s", "uri", ctx, &args.graph_key)
            {
                where_conditions.push(cond);
            }
        }
        // Predicate only
        (false, true, false) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let PatternComponent::Iri(pred) = &args.predicate {
                let expanded = if pred.contains(':') && !pred.starts_with("http") {
                    expand_prefix(ctx, &args.graph_key, pred).unwrap_or_else(|| pred.clone())
                } else {
                    pred.clone()
                };
                where_conditions.push(format!("r.predicate = '{}'", escape_cypher_string(&expanded)));
            }
        }
        // Object only
        (false, false, true) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            match &args.object {
                PatternComponent::Iri(iri) => {
                    let expanded = if iri.contains(':') && !iri.starts_with("http") {
                        expand_prefix(ctx, &args.graph_key, iri).unwrap_or_else(|| iri.clone())
                    } else {
                        iri.clone()
                    };
                    where_conditions.push(format!("o.uri = '{}'", escape_cypher_string(&expanded)));
                }
                PatternComponent::Literal { value, language, datatype } => {
                    where_conditions.push(format!("o.value = '{}'", escape_cypher_string(value)));
                    if let Some(lang) = language {
                        where_conditions.push(format!("o.language = '{}'", escape_cypher_string(lang)));
                    }
                    if let Some(dtype) = datatype {
                        where_conditions.push(format!("o.datatype = '{}'", escape_cypher_string(dtype)));
                    }
                }
                PatternComponent::BlankNode(id) => {
                    where_conditions.push(format!("o.uri = '{}' AND o.isBlank = true", escape_cypher_string(id)));
                }
                PatternComponent::Wildcard => {}
            }
        }
        // Subject and predicate
        (true, true, false) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let Some(cond) =
                build_where_condition(&args.subject, "s", "uri", ctx, &args.graph_key)
            {
                where_conditions.push(cond);
            }
            if let PatternComponent::Iri(pred) = &args.predicate {
                let expanded = if pred.contains(':') && !pred.starts_with("http") {
                    expand_prefix(ctx, &args.graph_key, pred).unwrap_or_else(|| pred.clone())
                } else {
                    pred.clone()
                };
                where_conditions.push(format!("r.predicate = '{}'", escape_cypher_string(&expanded)));
            }
        }
        // Subject and object
        (true, false, true) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let Some(cond) =
                build_where_condition(&args.subject, "s", "uri", ctx, &args.graph_key)
            {
                where_conditions.push(cond);
            }
            match &args.object {
                PatternComponent::Iri(iri) => {
                    let expanded = if iri.contains(':') && !iri.starts_with("http") {
                        expand_prefix(ctx, &args.graph_key, iri).unwrap_or_else(|| iri.clone())
                    } else {
                        iri.clone()
                    };
                    where_conditions.push(format!("o.uri = '{}'", escape_cypher_string(&expanded)));
                }
                PatternComponent::Literal { value, language, datatype } => {
                    where_conditions.push(format!("o.value = '{}'", escape_cypher_string(value)));
                    if let Some(lang) = language {
                        where_conditions.push(format!("o.language = '{}'", escape_cypher_string(lang)));
                    }
                    if let Some(dtype) = datatype {
                        where_conditions.push(format!("o.datatype = '{}'", escape_cypher_string(dtype)));
                    }
                }
                PatternComponent::BlankNode(id) => {
                    where_conditions.push(format!("o.uri = '{}' AND o.isBlank = true", escape_cypher_string(id)));
                }
                PatternComponent::Wildcard => {}
            }
        }
        // Predicate and object
        (false, true, true) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let PatternComponent::Iri(pred) = &args.predicate {
                let expanded = if pred.contains(':') && !pred.starts_with("http") {
                    expand_prefix(ctx, &args.graph_key, pred).unwrap_or_else(|| pred.clone())
                } else {
                    pred.clone()
                };
                where_conditions.push(format!("r.predicate = '{}'", escape_cypher_string(&expanded)));
            }
            match &args.object {
                PatternComponent::Iri(iri) => {
                    let expanded = if iri.contains(':') && !iri.starts_with("http") {
                        expand_prefix(ctx, &args.graph_key, iri).unwrap_or_else(|| iri.clone())
                    } else {
                        iri.clone()
                    };
                    where_conditions.push(format!("o.uri = '{}'", escape_cypher_string(&expanded)));
                }
                PatternComponent::Literal { value, language, datatype } => {
                    where_conditions.push(format!("o.value = '{}'", escape_cypher_string(value)));
                    if let Some(lang) = language {
                        where_conditions.push(format!("o.language = '{}'", escape_cypher_string(lang)));
                    }
                    if let Some(dtype) = datatype {
                        where_conditions.push(format!("o.datatype = '{}'", escape_cypher_string(dtype)));
                    }
                }
                PatternComponent::BlankNode(id) => {
                    where_conditions.push(format!("o.uri = '{}' AND o.isBlank = true", escape_cypher_string(id)));
                }
                PatternComponent::Wildcard => {}
            }
        }
        // All specified - most specific
        (true, true, true) => {
            match_clauses.push("MATCH (s)-[r]->(o)".to_string());
            if let Some(cond) =
                build_where_condition(&args.subject, "s", "uri", ctx, &args.graph_key)
            {
                where_conditions.push(cond);
            }
            if let PatternComponent::Iri(pred) = &args.predicate {
                let expanded = if pred.contains(':') && !pred.starts_with("http") {
                    expand_prefix(ctx, &args.graph_key, pred).unwrap_or_else(|| pred.clone())
                } else {
                    pred.clone()
                };
                where_conditions.push(format!("r.predicate = '{}'", escape_cypher_string(&expanded)));
            }
            match &args.object {
                PatternComponent::Iri(iri) => {
                    let expanded = if iri.contains(':') && !iri.starts_with("http") {
                        expand_prefix(ctx, &args.graph_key, iri).unwrap_or_else(|| iri.clone())
                    } else {
                        iri.clone()
                    };
                    where_conditions.push(format!("o.uri = '{}'", escape_cypher_string(&expanded)));
                }
                PatternComponent::Literal { value, language, datatype } => {
                    where_conditions.push(format!("o.value = '{}'", escape_cypher_string(value)));
                    if let Some(lang) = language {
                        where_conditions.push(format!("o.language = '{}'", escape_cypher_string(lang)));
                    }
                    if let Some(dtype) = datatype {
                        where_conditions.push(format!("o.datatype = '{}'", escape_cypher_string(dtype)));
                    }
                }
                PatternComponent::BlankNode(id) => {
                    where_conditions.push(format!("o.uri = '{}' AND o.isBlank = true", escape_cypher_string(id)));
                }
                PatternComponent::Wildcard => {}
            }
        }
    }

    // Add named graph filter if specified
    if let Some(ref named_graph) = args.named_graph {
        where_conditions.push(format!("r.graph = '{}'", escape_cypher_string(named_graph)));
    }

    // Build the query
    let mut query = match_clauses.join(" ");

    if !where_conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&where_conditions.join(" AND "));
    }

    // Delete the relationship
    query.push_str(" DELETE r");

    // Also delete literal nodes if object is a literal match or wildcard
    if matches!(args.object, PatternComponent::Literal { .. }) || args.object.is_wildcard() {
        query.push_str(" WITH o WHERE o:Literal AND NOT EXISTS { (x)-[]->(o) } DELETE o");
    }

    query
}

/// Generate query to delete orphaned nodes
fn generate_orphan_cleanup_query() -> &'static str {
    "MATCH (n) WHERE NOT EXISTS { (n)-[]-() } AND NOT EXISTS { ()-[]->(n) } DELETE n"
}

/// Extract deletion statistics from FalkorDB result
fn extract_delete_stats(result: &RedisValue) -> (i64, i64) {
    let mut deleted_rels = 0i64;
    let mut deleted_nodes = 0i64;

    if let RedisValue::Array(arr) = result {
        // Stats are typically the last element
        if let Some(RedisValue::Array(stats)) = arr.last() {
            for stat in stats {
                if let RedisValue::SimpleString(stat_str) = stat {
                    if stat_str.starts_with("Relationships deleted:") {
                        if let Some(num_str) = stat_str.strip_prefix("Relationships deleted:") {
                            deleted_rels = num_str.trim().parse().unwrap_or(0);
                        }
                    } else if stat_str.starts_with("Nodes deleted:") {
                        if let Some(num_str) = stat_str.strip_prefix("Nodes deleted:") {
                            deleted_nodes = num_str.trim().parse().unwrap_or(0);
                        }
                    }
                }
            }
        }
    }

    (deleted_rels, deleted_nodes)
}

/// RDF.DELETE command handler
///
/// Syntax:
///   RDF.DELETE <graph_key> <subject> <predicate> <object> [GRAPH <named_graph>] [ORPHANS]
///
/// Pattern components:
///   - <uri>: Full IRI
///   - prefix:name: Prefixed name (expanded using registered namespaces)
///   - "literal": Plain literal
///   - "literal"@lang: Language-tagged literal
///   - "literal"^^<datatype>: Typed literal
///   - _:id: Blank node
///   - *: Wildcard (matches anything)
///
/// Options:
///   - GRAPH <name>: Only delete from named graph
///   - ORPHANS: Also delete nodes that become orphaned
///
/// Returns:
///   - Integer: Number of relationships deleted
///   - Or array [relationships_deleted, nodes_deleted] if ORPHANS specified
pub fn rdf_delete(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let parsed = DeleteArgs::parse(&args)?;

    // Check if graph exists
    if !graph_exists(ctx, &parsed.graph_key)? {
        return Err(RedisError::String(format!(
            "Graph '{}' does not exist",
            parsed.graph_key
        )));
    }

    // Generate and execute the delete query
    let delete_query = generate_delete_query(&parsed, ctx);
    log::debug!("Executing delete query: {}", delete_query);

    let result = ctx.call("GRAPH.QUERY", &[&parsed.graph_key, &delete_query])?;
    let (deleted_rels, deleted_literal_nodes) = extract_delete_stats(&result);

    let mut total_deleted_nodes = deleted_literal_nodes;

    // If ORPHANS flag is set, clean up orphaned nodes
    if parsed.delete_orphans {
        let orphan_query = generate_orphan_cleanup_query();
        log::debug!("Cleaning up orphans: {}", orphan_query);

        let orphan_result = ctx.call("GRAPH.QUERY", &[&parsed.graph_key, orphan_query])?;
        let (_, orphan_nodes) = extract_delete_stats(&orphan_result);
        total_deleted_nodes += orphan_nodes;

        log::info!(
            "RDF.DELETE on '{}': {} relationships, {} nodes deleted",
            parsed.graph_key,
            deleted_rels,
            total_deleted_nodes
        );

        return Ok(RedisValue::Array(vec![
            RedisValue::Integer(deleted_rels),
            RedisValue::Integer(total_deleted_nodes),
        ]));
    }

    log::info!(
        "RDF.DELETE on '{}': {} relationships deleted",
        parsed.graph_key,
        deleted_rels
    );

    Ok(RedisValue::Integer(deleted_rels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_component_wildcard() {
        assert_eq!(PatternComponent::parse("*").unwrap(), PatternComponent::Wildcard);
    }

    #[test]
    fn test_pattern_component_iri() {
        assert_eq!(
            PatternComponent::parse("<http://example.org/test>").unwrap(),
            PatternComponent::Iri("http://example.org/test".to_string())
        );
    }

    #[test]
    fn test_pattern_component_blank_node() {
        assert_eq!(
            PatternComponent::parse("_:b1").unwrap(),
            PatternComponent::BlankNode("_:b1".to_string())
        );
    }

    #[test]
    fn test_pattern_component_plain_literal() {
        assert_eq!(
            PatternComponent::parse("\"hello\"").unwrap(),
            PatternComponent::Literal {
                value: "hello".to_string(),
                language: None,
                datatype: None,
            }
        );
    }

    #[test]
    fn test_pattern_component_lang_literal() {
        assert_eq!(
            PatternComponent::parse("\"hello\"@en").unwrap(),
            PatternComponent::Literal {
                value: "hello".to_string(),
                language: Some("en".to_string()),
                datatype: None,
            }
        );
    }

    #[test]
    fn test_pattern_component_typed_literal() {
        assert_eq!(
            PatternComponent::parse("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>").unwrap(),
            PatternComponent::Literal {
                value: "42".to_string(),
                language: None,
                datatype: Some("http://www.w3.org/2001/XMLSchema#integer".to_string()),
            }
        );
    }

    #[test]
    fn test_pattern_component_prefixed() {
        assert_eq!(
            PatternComponent::parse("foaf:Person").unwrap(),
            PatternComponent::Iri("foaf:Person".to_string())
        );
    }

    #[test]
    fn test_pattern_component_invalid() {
        assert!(PatternComponent::parse("").is_err());
        assert!(PatternComponent::parse("<>").is_err());
        assert!(PatternComponent::parse("_:").is_err());
    }

    #[test]
    fn test_is_wildcard() {
        assert!(PatternComponent::Wildcard.is_wildcard());
        assert!(!PatternComponent::Iri("test".to_string()).is_wildcard());
    }
}
