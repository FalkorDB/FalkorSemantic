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
        let result: RedisResult<Vec<redis::Value>> = redis::cmd("MODULE").arg("LIST").query(&mut con);
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

        let result: RedisResult<String> = redis::cmd("SEMANTIC.PARSE")
            .arg("test data")
            .query(ctx.conn());

        assert!(result.is_ok(), "SEMANTIC.PARSE should succeed");
        assert_eq!(result.unwrap(), "OK");
    }

    #[test]
    #[ignore]
    fn test_parse_ntriples_data() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "SEMANTIC.PARSE with N-Triples should succeed");
        assert_eq!(result.unwrap(), "OK");
    }

    #[test]
    #[ignore]
    fn test_parse_with_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/person/1> <http://example.org/name> "John Doe" ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(result.is_ok(), "SEMANTIC.PARSE with literal should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_with_typed_literal() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/person/1> <http://example.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with typed literal should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_with_language_tag() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/book/1> <http://example.org/title> "Le Petit Prince"@fr ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with language tag should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_multiple_triples() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/s1> <http://example.org/p> <http://example.org/o1> .
<http://example.org/s2> <http://example.org/p> <http://example.org/o2> .
<http://example.org/s3> <http://example.org/p> "value" ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with multiple triples should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_blank_nodes() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"_:b1 <http://example.org/p> _:b2 .
_:b2 <http://example.org/p> <http://example.org/o> ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with blank nodes should succeed"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_no_args_returns_error() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<String> = redis::cmd("SEMANTIC.PARSE").query(ctx.conn());

        assert!(
            result.is_err(),
            "SEMANTIC.PARSE without arguments should fail"
        );
    }

    #[test]
    #[ignore]
    fn test_parse_empty_string() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let result: RedisResult<String> = redis::cmd("SEMANTIC.PARSE").arg("").query(ctx.conn());

        // Empty string should be valid (no triples to parse)
        assert!(result.is_ok(), "SEMANTIC.PARSE with empty string should succeed");
    }

    #[test]
    #[ignore]
    fn test_parse_unicode_content() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        let ntriples = r#"<http://example.org/s> <http://example.org/p> "日本語テスト" ."#;

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with unicode should succeed"
        );
    }
}

mod command_help {
    use super::*;

    #[test]
    #[ignore]
    fn test_command_exists() {
        let mut ctx = TestContext::new().expect("Failed to create test context");

        // Check that SEMANTIC.PARSE is a known command
        let result: RedisResult<Vec<redis::Value>> =
            redis::cmd("COMMAND").arg("INFO").arg("SEMANTIC.PARSE").query(ctx.conn());

        assert!(result.is_ok(), "COMMAND INFO SEMANTIC.PARSE should succeed");
        
        let info = result.unwrap();
        assert!(!info.is_empty(), "SEMANTIC.PARSE should be a registered command");
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
                    let result: RedisResult<String> = redis::cmd("SEMANTIC.PARSE")
                        .arg(&ntriples)
                        .query(guard.conn());

                    result.is_ok()
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert!(
            results.iter().all(|&r| r),
            "All concurrent SEMANTIC.PARSE commands should succeed"
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

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(&ntriples).query(ctx.conn());

        assert!(result.is_ok(), "SEMANTIC.PARSE with 1000 triples should succeed");
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

        let result: RedisResult<String> =
            redis::cmd("SEMANTIC.PARSE").arg(&ntriples).query(ctx.conn());

        assert!(
            result.is_ok(),
            "SEMANTIC.PARSE with large literal should succeed"
        );
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
    assert!(ctx.is_ok(), "Should be able to create test context: {:?}", ctx.err());
    
    let mut ctx = ctx.unwrap();
    let pong: RedisResult<String> = redis::cmd("PING").query(ctx.conn());
    assert_eq!(pong.unwrap(), "PONG");
}
