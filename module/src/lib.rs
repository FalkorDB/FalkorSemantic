//! FalkorSemantic Redis Module
//!
//! This is the main Redis module that integrates the parser and mapper
//! to provide semantic graph capabilities.

mod commands;

use redis_module::{redis_module, Context, RedisError, RedisResult, RedisString, Status};

/// Initialize the module with Redis
fn init(_ctx: &Context, _args: &[RedisString]) -> Status {
    // Try to initialize logging, ignore if it fails
    let _ = env_logger::try_init();

    Status::Ok
}

/// Parse semantic data command
fn rdf_parse(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 2 {
        return Err(RedisError::WrongArity);
    }

    let input = args[1].try_as_str()?;

    // TODO: Implement actual parsing and mapping
    log::debug!("Parsing RDF data: {}", input);

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
