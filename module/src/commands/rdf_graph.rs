//! RDF.GRAPH Command Implementation
//!
//! Manages RDF graphs in FalkorDB.
//! Supports CREATE, DROP, LIST, and CLEAR subcommands.

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};

/// Redis key for tracking RDF graphs
const RDF_GRAPHS_SET: &str = "rdf:graphs";

/// Subcommands for RDF.GRAPH
#[derive(Debug, Clone, Copy, PartialEq)]
enum Subcommand {
    Create,
    Drop,
    List,
    Clear,
}

impl Subcommand {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CREATE" => Some(Subcommand::Create),
            "DROP" | "DELETE" => Some(Subcommand::Drop),
            "LIST" => Some(Subcommand::List),
            "CLEAR" | "EMPTY" => Some(Subcommand::Clear),
            _ => None,
        }
    }
}

/// Validate a graph name
///
/// Graph names must:
/// - Be non-empty
/// - Not contain spaces or control characters
/// - Start with a letter, digit, or underscore
fn validate_graph_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Graph name cannot be empty".into());
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_alphanumeric() && first_char != '_' {
        return Err("Graph name must start with a letter, digit, or underscore".into());
    }

    for ch in name.chars() {
        if ch.is_control() || ch == ' ' {
            return Err(format!("Graph name contains invalid character: {:?}", ch));
        }
    }

    Ok(())
}

/// Create a new RDF graph
fn create_graph(ctx: &Context, graph_name: &str) -> RedisResult {
    validate_graph_name(graph_name)
        .map_err(|e| RedisError::String(format!("Invalid graph name: {}", e)))?;

    // Check if graph already exists
    let exists_result = ctx.call("SISMEMBER", &[RDF_GRAPHS_SET, graph_name])?;
    let exists = match exists_result {
        RedisValue::Integer(n) => n > 0,
        _ => false,
    };

    if exists {
        return Err(RedisError::String(format!(
            "Graph '{}' already exists",
            graph_name
        )));
    }

    // Initialize the graph in FalkorDB by creating an empty node and deleting it
    // This ensures the graph exists in FalkorDB
    let init_query = "CREATE (n:__RDF_INIT__) DELETE n";
    let query_result = ctx.call("GRAPH.QUERY", &[graph_name, init_query]);

    if let Err(e) = query_result {
        log::warn!("Failed to initialize graph in FalkorDB: {:?}", e);
        // Continue anyway - the graph tracking is the important part
    }

    // Register the graph in our tracking set
    ctx.call("SADD", &[RDF_GRAPHS_SET, graph_name])?;

    log::info!("Created RDF graph: {}", graph_name);

    Ok(RedisValue::SimpleStringStatic("OK"))
}

/// Drop (delete) an RDF graph
fn drop_graph(ctx: &Context, graph_name: &str) -> RedisResult {
    // Check if graph exists in our tracking
    let exists_result = ctx.call("SISMEMBER", &[RDF_GRAPHS_SET, graph_name])?;
    let exists = match exists_result {
        RedisValue::Integer(n) => n > 0,
        _ => false,
    };

    if !exists {
        return Err(RedisError::String(format!(
            "Graph '{}' does not exist",
            graph_name
        )));
    }

    // Delete the graph from FalkorDB
    let delete_result = ctx.call("GRAPH.DELETE", &[graph_name]);

    if let Err(e) = delete_result {
        log::warn!("Failed to delete graph from FalkorDB: {:?}", e);
        // Continue to remove from tracking anyway
    }

    // Remove from our tracking set
    ctx.call("SREM", &[RDF_GRAPHS_SET, graph_name])?;

    // Also clean up any namespace mappings for this graph
    let ns_pattern = format!("rdf:ns:{}:*", graph_name);
    let keys_result = ctx.call("KEYS", &[&ns_pattern]);

    if let Ok(RedisValue::Array(keys)) = keys_result {
        for key in keys {
            if let RedisValue::SimpleString(key_str) = key {
                let _ = ctx.call("DEL", &[key_str.as_str()]);
            }
        }
    }

    log::info!("Dropped RDF graph: {}", graph_name);

    Ok(RedisValue::SimpleStringStatic("OK"))
}

/// List all RDF graphs
fn list_graphs(ctx: &Context) -> RedisResult {
    let members_result = ctx.call("SMEMBERS", &[RDF_GRAPHS_SET])?;

    match members_result {
        RedisValue::Array(arr) => {
            // Return graph names with additional info
            let mut graphs: Vec<RedisValue> = Vec::new();

            for member in arr {
                if let RedisValue::SimpleString(graph_name) = member {
                    // Try to get node count from FalkorDB
                    let count_query = "MATCH (n) RETURN count(n) as count";
                    let count_result = ctx.call("GRAPH.QUERY", &[graph_name.as_str(), count_query]);

                    let node_count = match count_result {
                        Ok(RedisValue::Array(ref result_arr)) if result_arr.len() >= 2 => {
                            // Result format: [headers, [data rows], stats]
                            if let Some(RedisValue::Array(ref rows)) = result_arr.get(1) {
                                if let Some(RedisValue::Array(ref first_row)) = rows.first() {
                                    if let Some(RedisValue::Integer(count)) = first_row.first() {
                                        *count
                                    } else {
                                        -1
                                    }
                                } else {
                                    0
                                }
                            } else {
                                -1
                            }
                        }
                        _ => -1,
                    };

                    graphs.push(RedisValue::Array(vec![
                        RedisValue::SimpleString(graph_name),
                        RedisValue::Integer(node_count),
                    ]));
                }
            }

            Ok(RedisValue::Array(graphs))
        }
        _ => Ok(RedisValue::Array(vec![])),
    }
}

/// Clear all data from an RDF graph (keep the graph, remove all nodes/edges)
fn clear_graph(ctx: &Context, graph_name: &str) -> RedisResult {
    // Check if graph exists in our tracking
    let exists_result = ctx.call("SISMEMBER", &[RDF_GRAPHS_SET, graph_name])?;
    let exists = match exists_result {
        RedisValue::Integer(n) => n > 0,
        _ => false,
    };

    if !exists {
        return Err(RedisError::String(format!(
            "Graph '{}' does not exist",
            graph_name
        )));
    }

    // Delete all nodes and relationships from the graph
    // Using MATCH (n) DETACH DELETE n to remove everything
    let clear_query = "MATCH (n) DETACH DELETE n";
    let result = ctx.call("GRAPH.QUERY", &[graph_name, clear_query]);

    match result {
        Ok(RedisValue::Array(ref arr)) => {
            // Try to extract the number of deleted nodes from stats
            let mut deleted_nodes = 0i64;
            let mut deleted_relationships = 0i64;

            // Stats are typically the last element in the result array
            if let Some(RedisValue::Array(stats)) = arr.last() {
                for stat in stats {
                    if let RedisValue::SimpleString(stat_str) = stat {
                        if stat_str.starts_with("Nodes deleted:") {
                            if let Some(num_str) = stat_str.strip_prefix("Nodes deleted:") {
                                deleted_nodes = num_str.trim().parse().unwrap_or(0);
                            }
                        } else if stat_str.starts_with("Relationships deleted:") {
                            if let Some(num_str) = stat_str.strip_prefix("Relationships deleted:") {
                                deleted_relationships = num_str.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
            }

            log::info!(
                "Cleared RDF graph '{}': {} nodes, {} relationships deleted",
                graph_name,
                deleted_nodes,
                deleted_relationships
            );

            Ok(RedisValue::Array(vec![
                RedisValue::Integer(deleted_nodes),
                RedisValue::Integer(deleted_relationships),
            ]))
        }
        Ok(_) => {
            log::info!("Cleared RDF graph: {}", graph_name);
            Ok(RedisValue::Array(vec![
                RedisValue::Integer(0),
                RedisValue::Integer(0),
            ]))
        }
        Err(e) => Err(RedisError::String(format!(
            "Failed to clear graph '{}': {:?}",
            graph_name, e
        ))),
    }
}

/// RDF.GRAPH command handler
///
/// Syntax:
/// - RDF.GRAPH CREATE <graph_name>
/// - RDF.GRAPH DROP <graph_name>
/// - RDF.GRAPH LIST
/// - RDF.GRAPH CLEAR <graph_name>
///
/// Returns:
/// - CREATE: "OK" on success
/// - DROP: "OK" on success
/// - LIST: Array of [graph_name, node_count] pairs
/// - CLEAR: [nodes_deleted, relationships_deleted]
pub fn rdf_graph(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    // Minimum args: command, subcommand
    if args.len() < 2 {
        return Err(RedisError::WrongArity);
    }

    let subcommand_str = args[1]
        .try_as_str()
        .map_err(|_| RedisError::String("Invalid subcommand".into()))?;

    let subcommand = Subcommand::from_str(subcommand_str).ok_or_else(|| {
        RedisError::String(format!(
            "Unknown subcommand '{}'. Use: CREATE, DROP, LIST, CLEAR",
            subcommand_str
        ))
    })?;

    match subcommand {
        Subcommand::Create => {
            if args.len() != 3 {
                return Err(RedisError::String(
                    "Usage: RDF.GRAPH CREATE <graph_name>".into(),
                ));
            }
            let graph_name = args[2]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid graph name".into()))?;
            create_graph(ctx, graph_name)
        }
        Subcommand::Drop => {
            if args.len() != 3 {
                return Err(RedisError::String(
                    "Usage: RDF.GRAPH DROP <graph_name>".into(),
                ));
            }
            let graph_name = args[2]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid graph name".into()))?;
            drop_graph(ctx, graph_name)
        }
        Subcommand::List => {
            if args.len() != 2 {
                return Err(RedisError::String("Usage: RDF.GRAPH LIST".into()));
            }
            list_graphs(ctx)
        }
        Subcommand::Clear => {
            if args.len() != 3 {
                return Err(RedisError::String(
                    "Usage: RDF.GRAPH CLEAR <graph_name>".into(),
                ));
            }
            let graph_name = args[2]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid graph name".into()))?;
            clear_graph(ctx, graph_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subcommand_parsing() {
        assert_eq!(Subcommand::from_str("CREATE"), Some(Subcommand::Create));
        assert_eq!(Subcommand::from_str("create"), Some(Subcommand::Create));
        assert_eq!(Subcommand::from_str("DROP"), Some(Subcommand::Drop));
        assert_eq!(Subcommand::from_str("DELETE"), Some(Subcommand::Drop));
        assert_eq!(Subcommand::from_str("LIST"), Some(Subcommand::List));
        assert_eq!(Subcommand::from_str("CLEAR"), Some(Subcommand::Clear));
        assert_eq!(Subcommand::from_str("EMPTY"), Some(Subcommand::Clear));
        assert_eq!(Subcommand::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_validate_graph_name_valid() {
        assert!(validate_graph_name("mygraph").is_ok());
        assert!(validate_graph_name("my_graph").is_ok());
        assert!(validate_graph_name("MyGraph123").is_ok());
        assert!(validate_graph_name("_private").is_ok());
        assert!(validate_graph_name("123graph").is_ok());
        assert!(validate_graph_name("graph-name").is_ok());
    }

    #[test]
    fn test_validate_graph_name_invalid() {
        assert!(validate_graph_name("").is_err());
        assert!(validate_graph_name("my graph").is_err());
        assert!(validate_graph_name("-graph").is_err());
        assert!(validate_graph_name(".graph").is_err());
    }
}
