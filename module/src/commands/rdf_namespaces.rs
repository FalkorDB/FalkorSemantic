//! RDF.NAMESPACES Command Implementation
//!
//! Manages namespace prefix mappings for RDF data.
//! Supports LIST, ADD, and REMOVE subcommands.

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};

/// Redis key prefix for storing namespace mappings
const NAMESPACE_KEY_PREFIX: &str = "rdf:ns:";

/// Subcommands for RDF.NAMESPACES
#[derive(Debug, Clone, Copy, PartialEq)]
enum Subcommand {
    List,
    Add,
    Remove,
}

impl Subcommand {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "LIST" => Some(Subcommand::List),
            "ADD" => Some(Subcommand::Add),
            "REMOVE" | "DELETE" | "DEL" => Some(Subcommand::Remove),
            _ => None,
        }
    }
}

/// Validate a namespace prefix
///
/// Prefixes must:
/// - Be non-empty
/// - Start with a letter or underscore
/// - Contain only letters, digits, underscores, and hyphens
fn validate_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() {
        return Err("Prefix cannot be empty".into());
    }

    let first_char = prefix.chars().next().unwrap();
    if !first_char.is_alphabetic() && first_char != '_' {
        return Err("Prefix must start with a letter or underscore".into());
    }

    for ch in prefix.chars() {
        if !ch.is_alphanumeric() && ch != '_' && ch != '-' {
            return Err(format!(
                "Prefix contains invalid character: '{}'. Only letters, digits, underscores, and hyphens are allowed",
                ch
            ));
        }
    }

    Ok(())
}

/// Validate a namespace URI
///
/// URIs must:
/// - Be non-empty
/// - Contain a scheme (have a colon)
/// - Not contain spaces or control characters
/// - Typically end with # or / (warning if not)
fn validate_uri(uri: &str) -> Result<(), String> {
    if uri.is_empty() {
        return Err("URI cannot be empty".into());
    }

    if !uri.contains(':') {
        return Err("URI must contain a scheme (no ':' found)".into());
    }

    for ch in uri.chars() {
        if ch.is_control() || ch == ' ' {
            return Err(format!("URI contains invalid character: {:?}", ch));
        }
    }

    // Warn but don't fail if URI doesn't end with # or /
    // This is just a convention, not a requirement
    if !uri.ends_with('#') && !uri.ends_with('/') {
        log::warn!(
            "Namespace URI '{}' does not end with '#' or '/'. This may cause issues with IRI resolution.",
            uri
        );
    }

    Ok(())
}

/// Get the Redis key for a namespace prefix
fn namespace_key(graph_key: &str, prefix: &str) -> String {
    format!("{}{}:{}", NAMESPACE_KEY_PREFIX, graph_key, prefix)
}

/// Get the Redis key pattern for all namespaces in a graph
fn namespace_pattern(graph_key: &str) -> String {
    format!("{}{}:*", NAMESPACE_KEY_PREFIX, graph_key)
}

/// List all namespace prefixes for a graph
fn list_namespaces(ctx: &Context, graph_key: &str) -> RedisResult {
    let pattern = namespace_pattern(graph_key);

    // Use SCAN to find namespace keys (non-blocking, production-safe)
    let keys = super::utils::scan_keys(ctx, &pattern);

    let mut namespaces: Vec<RedisValue> = Vec::new();
    let prefix_offset = format!("{}{}:", NAMESPACE_KEY_PREFIX, graph_key).len();

    for key_str in keys {
        // Extract prefix from key
        if key_str.len() > prefix_offset {
            let prefix = &key_str[prefix_offset..];

            // Get the URI value
            let uri_result = ctx.call("GET", &[key_str.as_str()])?;

            if let RedisValue::SimpleString(uri) = uri_result {
                namespaces.push(RedisValue::Array(vec![
                    RedisValue::SimpleString(prefix.to_string()),
                    RedisValue::SimpleString(uri),
                ]));
            }
        }
    }

    Ok(RedisValue::Array(namespaces))
}

/// Add a namespace prefix mapping
fn add_namespace(ctx: &Context, graph_key: &str, prefix: &str, uri: &str) -> RedisResult {
    // Validate inputs
    validate_prefix(prefix).map_err(|e| RedisError::String(format!("Invalid prefix: {}", e)))?;
    validate_uri(uri).map_err(|e| RedisError::String(format!("Invalid URI: {}", e)))?;

    let key = namespace_key(graph_key, prefix);

    // Store the mapping
    ctx.call("SET", &[&key, uri])?;

    log::debug!(
        "Added namespace: {} -> {} (graph: {})",
        prefix,
        uri,
        graph_key
    );

    Ok(RedisValue::SimpleStringStatic("OK"))
}

/// Remove a namespace prefix mapping
fn remove_namespace(ctx: &Context, graph_key: &str, prefix: &str) -> RedisResult {
    let key = namespace_key(graph_key, prefix);

    // Check if the key exists
    let exists_result = ctx.call("EXISTS", &[&key])?;
    let exists = match exists_result {
        RedisValue::Integer(n) => n > 0,
        _ => false,
    };

    if !exists {
        return Err(RedisError::String(format!(
            "Namespace prefix '{}' not found in graph '{}'",
            prefix, graph_key
        )));
    }

    // Delete the mapping
    ctx.call("DEL", &[&key])?;

    log::debug!("Removed namespace: {} (graph: {})", prefix, graph_key);

    Ok(RedisValue::SimpleStringStatic("OK"))
}

/// RDF.NAMESPACES command handler
///
/// Syntax:
/// - RDF.NAMESPACES <graph_key> LIST
/// - RDF.NAMESPACES <graph_key> ADD <prefix> <uri>
/// - RDF.NAMESPACES <graph_key> REMOVE <prefix>
///
/// Arguments:
/// - graph_key: The FalkorDB graph name
/// - LIST: List all registered namespace prefixes
/// - ADD: Add a new namespace prefix mapping
/// - REMOVE: Remove an existing namespace prefix mapping
///
/// Returns:
/// - LIST: Array of [prefix, uri] pairs
/// - ADD: "OK" on success
/// - REMOVE: "OK" on success
pub fn rdf_namespaces(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    // Minimum args: command, graph_key, subcommand
    if args.len() < 3 {
        return Err(RedisError::WrongArity);
    }

    let graph_key = args[1]
        .try_as_str()
        .map_err(|_| RedisError::String("Invalid graph key".into()))?;

    let subcommand_str = args[2]
        .try_as_str()
        .map_err(|_| RedisError::String("Invalid subcommand".into()))?;

    let subcommand = Subcommand::from_str(subcommand_str).ok_or_else(|| {
        RedisError::String(format!(
            "Unknown subcommand '{}'. Use: LIST, ADD, REMOVE",
            subcommand_str
        ))
    })?;

    match subcommand {
        Subcommand::List => {
            if args.len() != 3 {
                return Err(RedisError::String(
                    "Usage: RDF.NAMESPACES <graph_key> LIST".into(),
                ));
            }
            list_namespaces(ctx, graph_key)
        }
        Subcommand::Add => {
            if args.len() != 5 {
                return Err(RedisError::String(
                    "Usage: RDF.NAMESPACES <graph_key> ADD <prefix> <uri>".into(),
                ));
            }
            let prefix = args[3]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid prefix".into()))?;
            let uri = args[4]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid URI".into()))?;
            add_namespace(ctx, graph_key, prefix, uri)
        }
        Subcommand::Remove => {
            if args.len() != 4 {
                return Err(RedisError::String(
                    "Usage: RDF.NAMESPACES <graph_key> REMOVE <prefix>".into(),
                ));
            }
            let prefix = args[3]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid prefix".into()))?;
            remove_namespace(ctx, graph_key, prefix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subcommand_parsing() {
        assert_eq!(Subcommand::from_str("LIST"), Some(Subcommand::List));
        assert_eq!(Subcommand::from_str("list"), Some(Subcommand::List));
        assert_eq!(Subcommand::from_str("ADD"), Some(Subcommand::Add));
        assert_eq!(Subcommand::from_str("add"), Some(Subcommand::Add));
        assert_eq!(Subcommand::from_str("REMOVE"), Some(Subcommand::Remove));
        assert_eq!(Subcommand::from_str("DELETE"), Some(Subcommand::Remove));
        assert_eq!(Subcommand::from_str("DEL"), Some(Subcommand::Remove));
        assert_eq!(Subcommand::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_validate_prefix_valid() {
        assert!(validate_prefix("rdf").is_ok());
        assert!(validate_prefix("rdfs").is_ok());
        assert!(validate_prefix("ex").is_ok());
        assert!(validate_prefix("my_prefix").is_ok());
        assert!(validate_prefix("my-prefix").is_ok());
        assert!(validate_prefix("_private").is_ok());
        assert!(validate_prefix("prefix123").is_ok());
    }

    #[test]
    fn test_validate_prefix_invalid() {
        assert!(validate_prefix("").is_err());
        assert!(validate_prefix("123prefix").is_err());
        assert!(validate_prefix("-prefix").is_err());
        assert!(validate_prefix("pre fix").is_err());
        assert!(validate_prefix("pre:fix").is_err());
    }

    #[test]
    fn test_validate_uri_valid() {
        assert!(validate_uri("http://example.org/").is_ok());
        assert!(validate_uri("http://www.w3.org/1999/02/22-rdf-syntax-ns#").is_ok());
        assert!(validate_uri("https://example.com/ontology#").is_ok());
        assert!(validate_uri("urn:isbn:0451450523").is_ok());
    }

    #[test]
    fn test_validate_uri_invalid() {
        assert!(validate_uri("").is_err());
        assert!(validate_uri("no-scheme").is_err());
        assert!(validate_uri("http://example.org/with space").is_err());
    }

    #[test]
    fn test_namespace_key() {
        assert_eq!(namespace_key("mygraph", "rdf"), "rdf:ns:mygraph:rdf");
        assert_eq!(namespace_key("test", "ex"), "rdf:ns:test:ex");
    }

    #[test]
    fn test_namespace_pattern() {
        assert_eq!(namespace_pattern("mygraph"), "rdf:ns:mygraph:*");
    }
}
