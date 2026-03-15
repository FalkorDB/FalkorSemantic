//! End-to-End Tests for FalkorSemantic Redis Module
//!
//! These tests verify the FalkorSemantic module works correctly when
//! loaded into Redis. They test the full stack from Redis commands
//! through to the parser and back.
//!
//! # Running Tests
//!
//! ## Option 1: Manual Redis Setup
//! ```bash
//! # Build the module
//! cargo build -p falkorsemantic-module
//!
//! # Start Redis with the module (in another terminal)
//! redis-server --loadmodule ./target/debug/libfalkorsemantic_module.so --port 6399
//!
//! # Run the tests
//! TEST_REDIS_PORT=6399 cargo test --test e2e -- --test-threads=1
//! ```
//!
//! ## Option 2: Automatic (tests manage Redis lifecycle)
//! ```bash
//! cargo build -p falkorsemantic-module
//! cargo test --test e2e -- --test-threads=1
//! ```
//!
//! Note: Tests are ignored by default. Use `--ignored` to run them:
//! ```bash
//! cargo test --test e2e -- --ignored --test-threads=1
//! ```

use std::env;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use redis::{Connection, RedisResult};

/// Default port for test Redis instance
const DEFAULT_TEST_PORT: u16 = 6399;

/// How long to wait for Redis to start
const REDIS_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Get the test Redis port from environment or use default
fn get_test_port() -> u16 {
    env::var("TEST_REDIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_TEST_PORT)
}

/// Get the path to the compiled module
fn get_module_path() -> PathBuf {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(manifest_dir);

    // Try debug first, then release
    let debug_path = path.join("target/debug/libfalkorsemantic_module.so");
    if debug_path.exists() {
        return debug_path;
    }

    let release_path = path.join("target/release/libfalkorsemantic_module.so");
    if release_path.exists() {
        return release_path;
    }

    // Return debug path anyway - will fail with helpful message
    debug_path
}

/// Check if Redis is available on the test port
fn redis_is_available(port: u16) -> bool {
    TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
}

/// Wait for Redis to become available
fn wait_for_redis(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if redis_is_available(port) {
            // Give it a moment to fully initialize
            thread::sleep(Duration::from_millis(100));
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Managed Redis server for testing
struct TestRedisServer {
    process: Option<Child>,
    port: u16,
}

impl TestRedisServer {
    /// Start a new Redis server with the FalkorSemantic module
    fn start() -> Result<Self, String> {
        let port = get_test_port();

        // Check if Redis is already running on this port
        if redis_is_available(port) {
            // Verify it has our module loaded
            if Self::verify_module_loaded(port) {
                return Ok(Self {
                    process: None,
                    port,
                });
            } else {
                return Err(format!(
                    "Redis is running on port {} but FalkorSemantic module is not loaded",
                    port
                ));
            }
        }

        let module_path = get_module_path();
        if !module_path.exists() {
            return Err(format!(
                "Module not found at {:?}. Run 'cargo build -p falkorsemantic-module' first.",
                module_path
            ));
        }

        // Start Redis with the module
        let mut child = Command::new("redis-server")
            .args([
                "--port",
                &port.to_string(),
                "--loadmodule",
                module_path.to_str().unwrap(),
                "--daemonize",
                "no",
                "--loglevel",
                "warning",
                "--save",
                "", // Disable RDB persistence
                "--appendonly",
                "no", // Disable AOF persistence
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start redis-server: {}", e))?;

        // Wait for Redis to start
        if !wait_for_redis(port, REDIS_STARTUP_TIMEOUT) {
            // Try to get error output
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                let errors: Vec<String> = reader.lines().take(10).filter_map(|l| l.ok()).collect();
                child.kill().ok();
                return Err(format!(
                    "Redis failed to start within {:?}. Errors:\n{}",
                    REDIS_STARTUP_TIMEOUT,
                    errors.join("\n")
                ));
            }
            child.kill().ok();
            return Err(format!(
                "Redis failed to start within {:?}",
                REDIS_STARTUP_TIMEOUT
            ));
        }

        Ok(Self {
            process: Some(child),
            port,
        })
    }

    /// Verify that the FalkorSemantic module is loaded
    fn verify_module_loaded(port: u16) -> bool {
        let client = match redis::Client::open(format!("redis://127.0.0.1:{}/", port)) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut con = match client.get_connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        // MODULE LIST returns an array of arrays with key-value pairs
        let result: RedisResult<Vec<redis::Value>> =
            redis::cmd("MODULE").arg("LIST").query(&mut con);
        match result {
            Ok(modules) => {
                for module in modules {
                    if let redis::Value::Array(fields) = module {
                        // Check each field pair for "name" -> "falkorsemantic"
                        let mut iter = fields.iter();
                        while let Some(key) = iter.next() {
                            if let redis::Value::BulkString(k) = key {
                                if k == b"name" {
                                    if let Some(redis::Value::BulkString(v)) = iter.next() {
                                        if v == b"falkorsemantic" {
                                            return true;
                                        }
                                    }
                                } else {
                                    // Skip the value for other keys
                                    iter.next();
                                }
                            }
                        }
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Get a Redis connection
    fn connection(&self) -> Result<Connection, String> {
        let client = redis::Client::open(format!("redis://127.0.0.1:{}/", self.port))
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;
        client
            .get_connection()
            .map_err(|e| format!("Failed to connect to Redis: {}", e))
    }

    /// Get the port
    fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for TestRedisServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            // Try graceful shutdown first
            let _ = Command::new("redis-cli")
                .args(["-p", &self.port.to_string(), "SHUTDOWN", "NOSAVE"])
                .output();

            // Wait a bit for graceful shutdown
            thread::sleep(Duration::from_millis(500));

            // Force kill if still running
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// Test fixture that provides a Redis connection
struct TestContext {
    server: TestRedisServer,
    conn: Connection,
}

impl TestContext {
    fn new() -> Result<Self, String> {
        let server = TestRedisServer::start()?;
        let conn = server.connection()?;
        Ok(Self { server, conn })
    }

    fn conn(&mut self) -> &mut Connection {
        &mut self.conn
    }

    #[allow(dead_code)]
    fn port(&self) -> u16 {
        self.server.port()
    }
}

// ============================================================================
// Tests
// ============================================================================

mod module_loading {
    use super::*;

    #[test]
    #[ignore]
    fn test_module_is_loaded() {
        let ctx = TestContext::new().expect("Failed to create test context");
        assert!(
            TestRedisServer::verify_module_loaded(ctx.port()),
            "FalkorSemantic module should be loaded"
        );
    }

    #[test]
    #[ignore]
    fn test_module_version() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<Vec<redis::Value>> =
            redis::cmd("MODULE").arg("LIST").query(ctx.conn());

        let modules = result.expect("MODULE LIST should succeed");

        // Find our module and check version
        let mut found = false;
        for module in modules {
            if let redis::Value::Array(fields) = module {
                let fields_str: Vec<String> = fields
                    .iter()
                    .filter_map(|v| match v {
                        redis::Value::BulkString(b) => String::from_utf8(b.clone()).ok(),
                        redis::Value::Int(i) => Some(i.to_string()),
                        _ => None,
                    })
                    .collect();

                if fields_str.contains(&"falkorsemantic".to_string()) {
                    found = true;
                    // Version should be 1
                    assert!(
                        fields_str.contains(&"1".to_string()),
                        "Module version should be 1"
                    );
                }
            }
        }
        assert!(found, "falkorsemantic module should be in MODULE LIST");
    }
}

mod semantic_parse_command {
    use super::*;

    #[test]
    #[ignore]
    fn test_parse_returns_ok() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<String> =
            redis::cmd("RDF.PARSE").arg("test data").query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE should succeed");
        assert_eq!(result.unwrap(), "OK");
    }

    #[test]
    #[ignore]
    fn test_parse_ntriples_data() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with N-Triples should succeed");
        assert_eq!(result.unwrap(), "OK");
    }

    #[test]
    #[ignore]
    fn test_parse_with_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/person/1> <http://example.org/name> "John Doe" ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with literal should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_with_typed_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/person/1> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.PARSE with typed literal should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_with_language_tag() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples =
            r#"<http://example.org/book/1> <http://example.org/title> "Le Petit Prince"@fr ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with language tag should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_multiple_triples() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> .
<http://example.org/s3> <http://example.org/p> "value" ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.PARSE with multiple triples should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_blank_nodes() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"_:b1 <http://example.org/p> _:b2 .
_:b2 <http://example.org/p> <http://example.org/o> ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with blank nodes should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_no_args_returns_error() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").query(ctx.conn());

        assert!(result.is_err(), "RDF.PARSE without arguments should fail");
    }

    #[test]
    #[ignore]
    fn test_parse_empty_string() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg("").query(ctx.conn());

        // Empty string should be valid (no triples to parse)
        assert!(result.is_ok(), "RDF.PARSE with empty string should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_unicode_content() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/s> <http://example.org/p> "日本語テスト" ."#;

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with unicode should succeed");
    }
}

mod command_help {
    use super::*;

    #[test]
    #[ignore]
    fn test_command_exists() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Check that RDF.PARSE is a known command
        let result: RedisResult<Vec<redis::Value>> = redis::cmd("COMMAND")
            .arg("INFO")
            .arg("RDF.PARSE")
            .query(ctx.conn());

        assert!(result.is_ok(), "COMMAND INFO RDF.PARSE should succeed");

        let info = result.unwrap();
        assert!(!info.is_empty(), "RDF.PARSE should be a registered command");
    }
}

mod concurrent_access {
    use super::*;
    use std::sync::Arc;

    #[test]
    #[ignore]
    fn test_concurrent_parse_commands() {
        let ctx = Arc::new(std::sync::Mutex::new(
            TestContext::new().expect("Failed to create test context"),
        ));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let ctx = Arc::clone(&ctx);
                thread::spawn(move || {
                    let ntriples = format!(
                        "<http://example.org/s{}> <http://example.org/p> <http://example.org/o{}> .",
                        i, i
                    );

                    let mut guard = ctx.lock().unwrap();
                    let result: RedisResult<String> = redis::cmd("RDF.PARSE")
                        .arg(&ntriples)
                        .query(guard.conn());

                    result.is_ok()
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert!(
            results.iter().all(|&r| r),
            "All concurrent RDF.PARSE commands should succeed"
        );
    }
}

mod large_data {
    use super::*;

    #[test]
    #[ignore]
    fn test_parse_many_triples() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Generate 1000 triples
        let triples: Vec<String> = (0..1000)
            .map(|i| {
                format!(
                    "<http://example.org/s{}> <http://example.org/p> <http://example.org/o{}> .",
                    i, i
                )
            })
            .collect();

        let ntriples = triples.join("\n");

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(&ntriples).query(ctx.conn());

        assert!(result.is_ok(), "RDF.PARSE with 1000 triples should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_large_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Create a large literal (100KB)
        let large_text = "x".repeat(100_000);
        let ntriples = format!(
            r#"<http://example.org/s> <http://example.org/p> "{}" ."#,
            large_text
        );

        let result: RedisResult<String> = redis::cmd("RDF.PARSE").arg(&ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.PARSE with large literal should succeed"
        );
    }
}

// ============================================================================
// RDF.INSERT Tests
// ============================================================================

mod rdf_insert {
    use super::*;

    #[test]
    #[ignore]
    fn test_insert_small_dataset() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Small N-Triples dataset
        let ntriples = r#"<http://example.org/person/1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/1> <http://example.org/name> "Alice" .
<http://example.org/person/1> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/2> <http://example.org/name> "Bob" .
<http://example.org/person/2> <http://example.org/knows> <http://example.org/person/1> ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph")
            .arg(ntriples)
            .arg("FORMAT")
            .arg("ntriples")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_turtle_format() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let turtle = r#"@prefix ex: <http://example.org/> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

ex:person/1 rdf:type ex:Person ;
            ex:name "Charlie" ;
            ex:email "charlie@example.org" ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph_turtle")
            .arg(turtle)
            .arg("FORMAT")
            .arg("turtle")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT with Turtle should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_format_autodetect() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // N-Triples format should be auto-detected
        let ntriples = r#"<http://example.org/s> <http://example.org/p> <http://example.org/o> ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph_auto")
            .arg(ntriples)
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT with auto-detect should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_with_blank_nodes() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"_:b1 <http://example.org/name> "Anonymous" .
_:b1 <http://example.org/knows> _:b2 .
_:b2 <http://example.org/name> "Secret Friend" ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph_blank")
            .arg(ntriples)
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT with blank nodes should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_with_language_tags() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/book/1> <http://example.org/title> "The Little Prince"@en .
<http://example.org/book/1> <http://example.org/title> "Le Petit Prince"@fr .
<http://example.org/book/1> <http://example.org/title> "Der kleine Prinz"@de ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph_lang")
            .arg(ntriples)
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT with language tags should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_atomic() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph_atomic")
            .arg(ntriples)
            .arg("ATOMIC")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT with ATOMIC should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_insert_missing_graph_key() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT").query(ctx.conn());

        assert!(result.is_err(), "RDF.INSERT without graph key should fail");
    }

    #[test]
    #[ignore]
    fn test_insert_invalid_format() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("test_graph")
            .arg("some data")
            .arg("FORMAT")
            .arg("invalid_format")
            .query(ctx.conn());

        assert!(
            result.is_err(),
            "RDF.INSERT with invalid format should fail"
        );
    }
}

// ============================================================================
// RDF.BULK_INSERT Tests (Large Dataset)
// ============================================================================

mod rdf_bulk_insert {
    use super::*;

    #[test]
    #[ignore]
    fn test_bulk_insert_1000_triples() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Generate 1000 triples
        let triples: Vec<String> = (0..1000)
            .map(|i| {
                format!(
                    "<http://example.org/item/{}> <http://example.org/value> \"{}\" .",
                    i,
                    i * 10
                )
            })
            .collect();

        let ntriples = triples.join("\n");

        let start = std::time::Instant::now();
        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("bulk_test_1k")
            .arg(&ntriples)
            .query(ctx.conn());

        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "RDF.INSERT with 1000 triples should succeed: {:?}",
            result.err()
        );
        println!("Inserted 1000 triples in {:?}", elapsed);
    }

    #[test]
    #[ignore]
    fn test_bulk_insert_10000_triples() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Generate 10000 triples
        let triples: Vec<String> = (0..10000)
            .map(|i| {
                format!(
                    "<http://example.org/entity/{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Entity> .",
                    i
                )
            })
            .collect();

        let ntriples = triples.join("\n");

        let start = std::time::Instant::now();
        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("bulk_test_10k")
            .arg(&ntriples)
            .query(ctx.conn());

        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "RDF.INSERT with 10000 triples should succeed: {:?}",
            result.err()
        );
        println!("Inserted 10000 triples in {:?}", elapsed);
    }

    #[test]
    #[ignore]
    fn test_bulk_insert_complex_graph() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Generate a more complex graph with relationships
        let mut triples = Vec::new();

        // Create 100 people
        for i in 0..100 {
            triples.push(format!(
                "<http://example.org/person/{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .",
                i
            ));
            triples.push(format!(
                "<http://example.org/person/{}> <http://example.org/name> \"Person {}\" .",
                i, i
            ));
            triples.push(format!(
                "<http://example.org/person/{}> <http://example.org/age> \"{}\"^^<http://www.w3.org/2001/XMLSchema#integer> .",
                i, 20 + (i % 50)
            ));
        }

        // Create knows relationships (each person knows 5 others)
        for i in 0..100 {
            for j in 1..=5 {
                let knows = (i + j * 17) % 100;
                triples.push(format!(
                    "<http://example.org/person/{}> <http://example.org/knows> <http://example.org/person/{}> .",
                    i, knows
                ));
            }
        }

        let ntriples = triples.join("\n");

        let start = std::time::Instant::now();
        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg("bulk_test_complex")
            .arg(&ntriples)
            .query(ctx.conn());

        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "RDF.INSERT with complex graph should succeed: {:?}",
            result.err()
        );
        println!(
            "Inserted {} triples (complex graph) in {:?}",
            triples.len(),
            elapsed
        );
    }
}

// ============================================================================
// RDF.NAMESPACES Tests
// ============================================================================

mod rdf_namespaces {
    use super::*;

    #[test]
    #[ignore]
    fn test_namespaces_add() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_graph")
            .arg("ADD")
            .arg("ex")
            .arg("http://example.org/")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.NAMESPACES ADD should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_namespaces_add_multiple() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Add multiple namespaces
        let namespaces = [
            ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
            ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
            ("xsd", "http://www.w3.org/2001/XMLSchema#"),
            ("foaf", "http://xmlns.com/foaf/0.1/"),
        ];

        for (prefix, uri) in namespaces {
            let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
                .arg("test_ns_multi")
                .arg("ADD")
                .arg(prefix)
                .arg(uri)
                .query(ctx.conn());

            assert!(
                result.is_ok(),
                "RDF.NAMESPACES ADD {} should succeed: {:?}",
                prefix,
                result.err()
            );
        }
    }

    #[test]
    #[ignore]
    fn test_namespaces_list() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // First add some namespaces
        let _ = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_list")
            .arg("ADD")
            .arg("ex")
            .arg("http://example.org/")
            .query::<redis::Value>(ctx.conn());

        let _ = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_list")
            .arg("ADD")
            .arg("foaf")
            .arg("http://xmlns.com/foaf/0.1/")
            .query::<redis::Value>(ctx.conn());

        // List namespaces
        let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_list")
            .arg("LIST")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.NAMESPACES LIST should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_namespaces_remove() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Add a namespace
        let _ = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_remove")
            .arg("ADD")
            .arg("temp")
            .arg("http://temp.example.org/")
            .query::<redis::Value>(ctx.conn());

        // Remove it
        let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_remove")
            .arg("REMOVE")
            .arg("temp")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.NAMESPACES REMOVE should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_namespaces_invalid_prefix() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Invalid prefix (starts with number)
        let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_invalid")
            .arg("ADD")
            .arg("123prefix")
            .arg("http://example.org/")
            .query(ctx.conn());

        assert!(
            result.is_err(),
            "RDF.NAMESPACES with invalid prefix should fail"
        );
    }

    #[test]
    #[ignore]
    fn test_namespaces_invalid_uri() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Invalid URI (no scheme)
        let result: RedisResult<redis::Value> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_invalid_uri")
            .arg("ADD")
            .arg("ex")
            .arg("not-a-valid-uri")
            .query(ctx.conn());

        assert!(
            result.is_err(),
            "RDF.NAMESPACES with invalid URI should fail"
        );
    }

    #[test]
    #[ignore]
    fn test_namespaces_persistence() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Add a namespace
        let _ = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_persist")
            .arg("ADD")
            .arg("persistent")
            .arg("http://persistent.example.org/")
            .query::<redis::Value>(ctx.conn());

        // List should include it
        let result: RedisResult<Vec<redis::Value>> = redis::cmd("RDF.NAMESPACES")
            .arg("test_ns_persist")
            .arg("LIST")
            .query(ctx.conn());

        assert!(result.is_ok(), "RDF.NAMESPACES LIST should succeed");

        // Verify the namespace is present (implementation-specific check)
        let values = result.unwrap();
        assert!(
            !values.is_empty(),
            "Namespace list should not be empty after adding"
        );
    }
}

// ============================================================================
// RDF.DELETE Command Tests
// ============================================================================

mod rdf_delete {
    use super::*;

    /// Helper to setup test data
    fn setup_test_data(ctx: &mut TestContext, graph: &str) {
        // Create the graph
        let _ = redis::cmd("RDF.GRAPH")
            .arg("CREATE")
            .arg(graph)
            .query::<redis::Value>(ctx.conn());

        // Insert test data
        let turtle_data = r#"
            @prefix ex: <http://example.org/> .
            @prefix foaf: <http://xmlns.com/foaf/0.1/> .
            
            ex:alice a foaf:Person ;
                foaf:name "Alice" ;
                foaf:age "30" ;
                foaf:knows ex:bob, ex:charlie .
            
            ex:bob a foaf:Person ;
                foaf:name "Bob" ;
                foaf:knows ex:charlie .
            
            ex:charlie a foaf:Person ;
                foaf:name "Charlie" .
        "#;

        let _ = redis::cmd("RDF.INSERT")
            .arg(graph)
            .arg(turtle_data)
            .query::<redis::Value>(ctx.conn());
    }

    #[test]
    #[ignore]
    fn test_delete_specific_triple() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_specific";
        setup_test_data(&mut ctx, graph);

        // Delete Alice's age
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("<http://example.org/alice>")
            .arg("<http://xmlns.com/foaf/0.1/age>")
            .arg("\"30\"")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE should succeed: {:?}",
            result.err()
        );
        let deleted = result.unwrap();
        assert!(
            deleted >= 0,
            "Should return number of deleted relationships"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_by_subject_wildcard() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_subject";
        setup_test_data(&mut ctx, graph);

        // Delete all triples about Alice
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("<http://example.org/alice>")
            .arg("*")
            .arg("*")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE should succeed: {:?}",
            result.err()
        );
        let deleted = result.unwrap();
        assert!(
            deleted > 0,
            "Should delete multiple relationships for alice"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_by_predicate_wildcard() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_predicate";
        setup_test_data(&mut ctx, graph);

        // Delete all "knows" relationships
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("*")
            .arg("<http://xmlns.com/foaf/0.1/knows>")
            .arg("*")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE should succeed: {:?}",
            result.err()
        );
        let deleted = result.unwrap();
        assert!(deleted >= 3, "Should delete all knows relationships");
    }

    #[test]
    #[ignore]
    fn test_delete_by_object_wildcard() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_object";
        setup_test_data(&mut ctx, graph);

        // Delete all relationships pointing to Charlie
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("*")
            .arg("*")
            .arg("<http://example.org/charlie>")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE should succeed: {:?}",
            result.err()
        );
        let deleted = result.unwrap();
        assert!(
            deleted >= 2,
            "Should delete relationships pointing to charlie"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_with_orphans() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_orphans";
        setup_test_data(&mut ctx, graph);

        // Delete all relationships, keeping orphaned nodes
        let result: RedisResult<Vec<i64>> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("*")
            .arg("*")
            .arg("*")
            .arg("ORPHANS")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE with ORPHANS should succeed: {:?}",
            result.err()
        );
        let stats = result.unwrap();
        assert_eq!(
            stats.len(),
            2,
            "Should return [rels_deleted, nodes_deleted]"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_literal_with_language() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_lang";

        // Create graph and insert data with language tags
        let _ = redis::cmd("RDF.GRAPH")
            .arg("CREATE")
            .arg(graph)
            .query::<redis::Value>(ctx.conn());

        let turtle_data = r#"
            @prefix ex: <http://example.org/> .
            @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
            
            ex:paris rdfs:label "Paris"@en ;
                     rdfs:label "Paris"@fr .
        "#;

        let _ = redis::cmd("RDF.INSERT")
            .arg(graph)
            .arg(turtle_data)
            .query::<redis::Value>(ctx.conn());

        // Delete only the English label
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("<http://example.org/paris>")
            .arg("<http://www.w3.org/2000/01/rdf-schema#label>")
            .arg("\"Paris\"@en")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE with language tag should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_delete_typed_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_typed";

        // Create graph and insert data with typed literals
        let _ = redis::cmd("RDF.GRAPH")
            .arg("CREATE")
            .arg(graph)
            .query::<redis::Value>(ctx.conn());

        let turtle_data = r#"
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
            
            ex:product1 ex:price "29.99"^^xsd:decimal ;
                        ex:quantity "100"^^xsd:integer .
        "#;

        let _ = redis::cmd("RDF.INSERT")
            .arg(graph)
            .arg(turtle_data)
            .query::<redis::Value>(ctx.conn());

        // Delete the price
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("<http://example.org/product1>")
            .arg("<http://example.org/price>")
            .arg("\"29.99\"^^<http://www.w3.org/2001/XMLSchema#decimal>")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE with typed literal should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_delete_nonexistent_graph() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg("nonexistent_graph_delete")
            .arg("<http://example.org/s>")
            .arg("<http://example.org/p>")
            .arg("<http://example.org/o>")
            .query(ctx.conn());

        assert!(
            result.is_err(),
            "RDF.DELETE on nonexistent graph should fail"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_invalid_pattern() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_invalid";

        let _ = redis::cmd("RDF.GRAPH")
            .arg("CREATE")
            .arg(graph)
            .query::<redis::Value>(ctx.conn());

        // Invalid URI format
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("invalid-uri")
            .arg("<http://example.org/p>")
            .arg("<http://example.org/o>")
            .query(ctx.conn());

        assert!(
            result.is_err(),
            "RDF.DELETE with invalid pattern should fail"
        );
    }

    #[test]
    #[ignore]
    fn test_delete_no_matches() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph = "test_delete_nomatch";
        setup_test_data(&mut ctx, graph);

        // Delete a triple that doesn't exist
        let result: RedisResult<i64> = redis::cmd("RDF.DELETE")
            .arg(graph)
            .arg("<http://example.org/nonexistent>")
            .arg("<http://example.org/nonexistent>")
            .arg("<http://example.org/nonexistent>")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.DELETE with no matches should succeed: {:?}",
            result.err()
        );
        let deleted = result.unwrap();
        assert_eq!(deleted, 0, "Should delete 0 relationships when no match");
    }
}

// ============================================================================
// Test Runner Helper
// ============================================================================

/// Main test that can be run to verify the test infrastructure works
#[test]
#[ignore]
fn test_infrastructure_works() {
    // This test just verifies we can start Redis and connect
    let ctx = TestContext::new();
    assert!(
        ctx.is_ok(),
        "Should be able to create test context: {:?}",
        ctx.err()
    );

    let mut ctx = ctx.unwrap();
    let pong: RedisResult<String> = redis::cmd("PING").query(ctx.conn());
    assert_eq!(pong.unwrap(), "PONG");
}

// ============================================================================
// RDF.QUERY E2E Tests - Arithmetic & NOT IN (#65, #66)
// ============================================================================

mod rdf_query_arithmetic {
    use super::*;

    /// Insert test data and return the graph key used
    fn setup_test_data(ctx: &mut TestContext, suffix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let graph_key = format!("test_query_arithmetic_{suffix}_{nanos}");
        let ntriples = r#"<http://example.org/person/1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/1> <http://example.org/name> "Alice" .
<http://example.org/person/1> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/1> <http://example.org/score> "85"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/2> <http://example.org/name> "Bob" .
<http://example.org/person/2> <http://example.org/age> "25"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/2> <http://example.org/score> "92"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/3> <http://example.org/name> "Charlie" .
<http://example.org/person/3> <http://example.org/age> "35"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/3> <http://example.org/score> "78"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/3> <http://example.org/status> "inactive" ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg(&graph_key)
            .arg(ntriples)
            .arg("FORMAT")
            .arg("ntriples")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT setup should succeed: {:?}",
            result.err()
        );
        graph_key
    }

    #[test]
    #[ignore]
    fn test_query_addition_filter() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "addition");

        // SPARQL: Find persons where age + 10 > 40
        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            ?s <http://example.org/age> ?age .
            FILTER(?age + 10 > 40)
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with addition filter should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        // Should return Charlie (35+10=45>40) and Alice (30+10=40, not >40)
        assert!(
            json.contains("Charlie"),
            "Should find Charlie (age 35 + 10 > 40), got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_multiplication_filter() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "multiplication");

        // SPARQL: Find persons where score * 2 > 170
        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            ?s <http://example.org/score> ?score .
            FILTER(?score * 2 > 170)
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with multiplication should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        // Bob has score 92*2=184>170, Alice has 85*2=170 (not >170)
        assert!(
            json.contains("Bob"),
            "Should find Bob (score 92*2 > 170), got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_not_in_filter() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "not_in");

        // SPARQL: Find persons whose name is NOT IN ("Alice", "Charlie")
        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER(?name NOT IN ("Alice", "Charlie"))
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with NOT IN should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Bob"),
            "Should find Bob (not in Alice/Charlie), got: {}",
            json
        );
        assert!(
            !json.contains("Alice"),
            "Should NOT find Alice, got: {}",
            json
        );
    }
}

// ============================================================================
// RDF.QUERY E2E Tests - String & Type-Checking Functions (#73, #74)
// ============================================================================

mod rdf_query_string_functions {
    use super::*;

    fn setup_test_data(ctx: &mut TestContext, suffix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let graph_key = format!("test_query_strings_{suffix}_{nanos}");
        let ntriples = r#"<http://example.org/person/1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/1> <http://example.org/name> "Alice Smith" .
<http://example.org/person/1> <http://example.org/label> "Hello World"@en .
<http://example.org/person/2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/2> <http://example.org/name> "Bob Jones" .
<http://example.org/person/2> <http://example.org/label> "Bonjour"@fr .
<http://example.org/person/3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
<http://example.org/person/3> <http://example.org/name> "Charlie Brown" .
<http://example.org/person/3> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg(&graph_key)
            .arg(ntriples)
            .arg("FORMAT")
            .arg("ntriples")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT setup should succeed: {:?}",
            result.err()
        );
        graph_key
    }

    #[test]
    #[ignore]
    fn test_query_substr() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "substr");

        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER(SUBSTR(?name, 1, 5) = "Alice")
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with SUBSTR should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Alice"),
            "Should find Alice Smith, got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_concat() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "concat");

        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER(CONTAINS(CONCAT(?name, "!"), "Alice"))
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with CONCAT should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_query_replace() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "replace");

        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER(REPLACE(?name, "Smith", "Johnson") = "Alice Johnson")
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with REPLACE should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Alice"),
            "Should find Alice Smith via REPLACE, got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_lang_filter() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "lang");

        let sparql = r#"SELECT ?label WHERE {
            ?s <http://example.org/label> ?label .
            FILTER(LANG(?label) = "en")
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with LANG filter should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Hello World"),
            "Should find English label, got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_is_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "is_literal");

        let sparql = r#"SELECT ?o WHERE {
            ?s ?p ?o .
            FILTER(isLiteral(?o))
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with isLiteral should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    #[ignore]
    fn test_query_is_iri() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "is_iri");

        let sparql = r#"SELECT ?o WHERE {
            ?s ?p ?o .
            FILTER(isIRI(?o))
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with isIRI should succeed: {:?}",
            result.err()
        );
    }
}

// ============================================================================
// RDF.QUERY E2E Tests - EXISTS / NOT EXISTS (#67)
// ============================================================================

mod rdf_query_exists {
    use super::*;

    fn setup_test_data(ctx: &mut TestContext, suffix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let graph_key = format!("test_query_exists_{suffix}_{nanos}");
        let ntriples = r#"<http://example.org/person/1> <http://example.org/name> "Alice" .
<http://example.org/person/1> <http://example.org/email> "alice@example.org" .
<http://example.org/person/2> <http://example.org/name> "Bob" .
<http://example.org/person/3> <http://example.org/name> "Charlie" .
<http://example.org/person/3> <http://example.org/email> "charlie@example.org" ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg(&graph_key)
            .arg(ntriples)
            .arg("FORMAT")
            .arg("ntriples")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT setup should succeed: {:?}",
            result.err()
        );
        graph_key
    }

    #[test]
    #[ignore]
    fn test_query_filter_exists() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "exists");

        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER EXISTS { ?s <http://example.org/email> ?email }
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with EXISTS should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Alice") && json.contains("Charlie"),
            "Should find Alice and Charlie (both have email), got: {}",
            json
        );
        assert!(
            !json.contains("Bob"),
            "Should NOT find Bob (no email), got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_filter_not_exists() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "not_exists");

        let sparql = r#"SELECT ?name WHERE {
            ?s <http://example.org/name> ?name .
            FILTER NOT EXISTS { ?s <http://example.org/email> ?email }
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with NOT EXISTS should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("Bob"),
            "Should find Bob (no email), got: {}",
            json
        );
        assert!(
            !json.contains("Alice"),
            "Should NOT find Alice (has email), got: {}",
            json
        );
    }
}

// ============================================================================
// RDF.QUERY E2E Tests - BIND / Extend (#69)
// ============================================================================

mod rdf_query_bind {
    use super::*;

    fn setup_test_data(ctx: &mut TestContext, suffix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let graph_key = format!("test_query_bind_{suffix}_{nanos}");
        let ntriples = r#"<http://example.org/person/1> <http://example.org/name> "alice" .
<http://example.org/person/1> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.org/person/2> <http://example.org/name> "bob" .
<http://example.org/person/2> <http://example.org/age> "25"^^<http://www.w3.org/2001/XMLSchema#integer> ."#;

        let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg(&graph_key)
            .arg(ntriples)
            .arg("FORMAT")
            .arg("ntriples")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.INSERT setup should succeed: {:?}",
            result.err()
        );
        graph_key
    }

    #[test]
    #[ignore]
    fn test_query_bind_ucase() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "ucase");

        let sparql = r#"SELECT ?name ?upper WHERE {
            ?s <http://example.org/name> ?name .
            BIND(UCASE(?name) AS ?upper)
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with BIND(UCASE) should succeed: {:?}",
            result.err()
        );
        let json = result.unwrap();
        assert!(
            json.contains("ALICE"),
            "Expected UCASE transformation to produce ALICE, got: {}",
            json
        );
    }

    #[test]
    #[ignore]
    fn test_query_bind_str_function() {
        let mut ctx = TestContext::new().expect("Failed to create test context");
        let graph_key = setup_test_data(&mut ctx, "str");

        let sparql = r#"SELECT ?s ?label WHERE {
            ?s <http://example.org/name> ?name .
            BIND(STR(?name) AS ?label)
        }"#;

        let result: RedisResult<String> = redis::cmd("RDF.QUERY")
            .arg(&graph_key)
            .arg(sparql)
            .arg("FORMAT")
            .arg("json")
            .query(ctx.conn());

        assert!(
            result.is_ok(),
            "RDF.QUERY with BIND(STR) should succeed: {:?}",
            result.err()
        );
    }
}
