//! Integration tests for FalkorSemantic
//! 
//! These tests require:
//! - Redis server with FalkorSemantic module loaded
//! - FalkorDB instance for graph operations
//! 
//! Run with: cargo test --test integration -- --ignored
//! or use docker-compose: docker-compose up -d && cargo test --test integration

use std::process::Command;

/// Check if Redis is available with the module loaded
fn redis_available() -> bool {
    Command::new("redis-cli")
        .args(&["ping"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if FalkorDB is available
fn falkordb_available() -> bool {
    Command::new("redis-cli")
        .args(&["-p", "6380", "ping"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore]
fn test_redis_connectivity() {
    assert!(redis_available(), "Redis server is not available");
}

#[test]
#[ignore]
fn test_falkordb_connectivity() {
    assert!(falkordb_available(), "FalkorDB server is not available");
}

#[test]
#[ignore]
fn test_module_loaded() {
    if !redis_available() {
        panic!("Redis server is not available");
    }

    let output = Command::new("redis-cli")
        .args(&["MODULE", "LIST"])
        .output()
        .expect("Failed to execute redis-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("falkorsemantic"),
        "FalkorSemantic module is not loaded"
    );
}

#[test]
#[ignore]
fn test_semantic_parse_command() {
    if !redis_available() {
        panic!("Redis server is not available");
    }

    let output = Command::new("redis-cli")
        .args(&["RDF.PARSE", "test data"])
        .output()
        .expect("Failed to execute redis-cli");

    assert!(
        output.status.success(),
        "RDF.PARSE command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "OK", "Expected 'OK' response");
}
