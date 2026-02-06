//! `FalkorSemantic` Redis Module
//!
//! This is the main Redis module that integrates the parser and mapper
//! to provide semantic graph capabilities.

mod commands;

use redis_module::{
    redis_module, Context, RedisError, RedisResult, RedisString, RedisValue, Status,
};

/// Initialize the module with Redis
fn init(ctx: &Context, _args: &[RedisString]) -> Status {
    // Try to initialize logging, ignore if it fails
    let _ = env_logger::try_init();

    // Check if FalkorDB module is loaded - this is required
    if !check_falkordb_module(ctx) {
        log::error!(
            "FalkorDB module not detected. FalkorSemantic requires FalkorDB to be loaded first."
        );
        return Status::Err;
    }

    log::info!("FalkorDB module detected. FalkorSemantic initialized successfully.");
    Status::Ok
}

/// Check if `FalkorDB` module is loaded
fn check_falkordb_module(ctx: &Context) -> bool {
    match ctx.call("MODULE", &["LIST"]) {
        Ok(RedisValue::Array(modules)) => {
            for module in modules {
                if let RedisValue::Array(module_info) = module {
                    // MODULE LIST returns pairs: ["name", "graph", "ver", 123, ...]
                    // We need to find the "name" key and check its value
                    let mut i = 0;
                    while i + 1 < module_info.len() {
                        // Extract key and value as strings (can be SimpleString or BulkString)
                        let key_str = match &module_info[i] {
                            RedisValue::SimpleString(s) | RedisValue::BulkString(s) => {
                                Some(s.as_str())
                            }
                            _ => None,
                        };

                        let value_str = match &module_info[i + 1] {
                            RedisValue::SimpleString(s) | RedisValue::BulkString(s) => {
                                Some(s.as_str())
                            }
                            _ => None,
                        };

                        if let (Some(key), Some(value)) = (key_str, value_str) {
                            if key == "name" {
                                let name_str = value.to_lowercase();
                                if name_str.contains("graph") || name_str.contains("falkor") {
                                    return true;
                                }
                            }
                        }
                        i += 2; // Move to next key-value pair
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Parse semantic data command
fn rdf_parse(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 2 {
        return Err(RedisError::WrongArity);
    }

    let input = args[1].try_as_str()?;

    // TODO: Implement actual parsing and mapping
    log::debug!("Parsing RDF data: {input}");

    Ok("OK".into())
}

redis_module! {
    name: "falkorsemantic",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    init: init,
    commands: [
        ["rdf.parse", rdf_parse, "write", 1, 1, 1],
        ["rdf.insert", commands::rdf_insert, "write", 2, -1, 1],
        ["rdf.bulk_insert", commands::rdf_bulk_insert, "write", 2, -1, 1],
        ["rdf.delete", commands::rdf_delete, "write", 2, -1, 1],
        ["rdf.namespaces", commands::rdf_namespaces, "write", 2, -1, 1],
        ["rdf.graph", commands::rdf_graph, "write", 1, -1, 1],
        ["rdf.query", commands::rdf_query, "readonly", 2, -1, 1],
    ],
}
