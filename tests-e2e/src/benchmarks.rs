//! Performance Benchmarks for FalkorSemantic
//!
//! These benchmarks measure the performance of RDF operations including:
//! - Bulk insert at various scales (1M, 10M, 100M triples)
//! - SPARQL query performance (SELECT, JOIN, property paths, aggregates)
//! - Comparison with native Cypher equivalents
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Build the module in release mode for accurate benchmarks
//! cargo build -p falkorsemantic-module --release
//!
//! # Start Redis with the module
//! redis-server --loadmodule ./target/release/libfalkorsemantic_module.so --port 6399
//!
//! # Run benchmarks
//! TEST_REDIS_PORT=6399 cargo test --test benchmarks -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Note: Benchmarks are ignored by default and require significant memory for large tests.

use std::env;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use redis::{Connection, RedisResult};

/// Default port for test Redis instance
const DEFAULT_TEST_PORT: u16 = 6399;

/// How long to wait for Redis to start
const REDIS_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Benchmark result
#[derive(Debug, Clone)]
struct BenchmarkResult {
    name: String,
    duration: Duration,
    operations: usize,
    ops_per_sec: f64,
    items_per_sec: Option<f64>,
}

impl BenchmarkResult {
    fn new(name: &str, duration: Duration, operations: usize) -> Self {
        let ops_per_sec = operations as f64 / duration.as_secs_f64();
        Self {
            name: name.to_string(),
            duration,
            operations,
            ops_per_sec,
            items_per_sec: None,
        }
    }

    fn with_items(mut self, items: usize) -> Self {
        self.items_per_sec = Some(items as f64 / self.duration.as_secs_f64());
        self
    }

    fn print(&self) {
        println!("┌─────────────────────────────────────────────────────────────");
        println!("│ Benchmark: {}", self.name);
        println!("├─────────────────────────────────────────────────────────────");
        println!("│ Duration:     {:>12.3?}", self.duration);
        println!("│ Operations:   {:>12}", self.operations);
        println!("│ Ops/sec:      {:>12.2}", self.ops_per_sec);
        if let Some(items_per_sec) = self.items_per_sec {
            println!("│ Items/sec:    {:>12.2}", items_per_sec);
        }
        println!("└─────────────────────────────────────────────────────────────");
    }
}

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

    // Try release first for benchmarks, then debug
    let release_path = path.join("target/release/libfalkorsemantic_module.so");
    if release_path.exists() {
        return release_path;
    }

    let debug_path = path.join("target/debug/libfalkorsemantic_module.so");
    if debug_path.exists() {
        eprintln!("WARNING: Using debug build for benchmarks. Results may not be representative.");
        return debug_path;
    }

    release_path
}

/// Check if Redis is available on the test port
fn redis_is_available(port: u16) -> bool {
    TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
}

/// Wait for Redis to become available
fn wait_for_redis(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if redis_is_available(port) {
            thread::sleep(Duration::from_millis(100));
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Managed Redis server for benchmarking
struct BenchRedisServer {
    process: Option<Child>,
    port: u16,
}

impl BenchRedisServer {
    fn start() -> Result<Self, String> {
        let port = get_test_port();

        if redis_is_available(port) {
            if Self::verify_module_loaded(port) {
                return Ok(Self { process: None, port });
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
                "Module not found at {:?}. Run 'cargo build -p falkorsemantic-module --release' first.",
                module_path
            ));
        }

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
                "",
                "--appendonly",
                "no",
                "--maxmemory",
                "8gb",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start redis-server: {}", e))?;

        if !wait_for_redis(port, REDIS_STARTUP_TIMEOUT) {
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
            return Err(format!("Redis failed to start within {:?}", REDIS_STARTUP_TIMEOUT));
        }

        Ok(Self {
            process: Some(child),
            port,
        })
    }

    fn verify_module_loaded(port: u16) -> bool {
        let client = match redis::Client::open(format!("redis://127.0.0.1:{}/", port)) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut con = match client.get_connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let result: RedisResult<Vec<redis::Value>> = redis::cmd("MODULE").arg("LIST").query(&mut con);
        match result {
            Ok(modules) => {
                for module in modules {
                    if let redis::Value::Array(fields) = module {
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

    fn connection(&self) -> Result<Connection, String> {
        let client = redis::Client::open(format!("redis://127.0.0.1:{}/", self.port))
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;
        client
            .get_connection()
            .map_err(|e| format!("Failed to connect to Redis: {}", e))
    }
}

impl Drop for BenchRedisServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = Command::new("redis-cli")
                .args(["-p", &self.port.to_string(), "SHUTDOWN", "NOSAVE"])
                .output();
            thread::sleep(Duration::from_millis(500));
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// Benchmark context
struct BenchContext {
    server: BenchRedisServer,
    conn: Connection,
}

impl BenchContext {
    fn new() -> Result<Self, String> {
        let server = BenchRedisServer::start()?;
        let conn = server.connection()?;
        Ok(Self { server, conn })
    }

    fn conn(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Clean up a graph before/after benchmarks
    fn cleanup_graph(&mut self, graph_key: &str) {
        let _ = redis::cmd("DEL").arg(graph_key).query::<i64>(self.conn());
        // Also try to delete any FalkorDB graph
        let _ = redis::cmd("GRAPH.DELETE").arg(graph_key).query::<String>(self.conn());
    }
}

/// Generate N-Triples data for a given number of entities
fn generate_ntriples(entity_count: usize, properties_per_entity: usize, relationships_per_entity: usize) -> String {
    let mut triples = Vec::with_capacity(entity_count * (1 + properties_per_entity + relationships_per_entity));

    for i in 0..entity_count {
        // Type triple
        triples.push(format!(
            "<http://example.org/entity/{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Entity> .",
            i
        ));

        // Property triples
        for p in 0..properties_per_entity {
            triples.push(format!(
                "<http://example.org/entity/{}> <http://example.org/prop{}> \"value_{}_{}\""  ,
                i, p, i, p
            ));
        }

        // Relationship triples
        for r in 0..relationships_per_entity {
            let target = (i + r * 17 + 1) % entity_count;
            triples.push(format!(
                "<http://example.org/entity/{}> <http://example.org/rel{}> <http://example.org/entity/{}> .",
                i, r, target
            ));
        }
    }

    triples.join("\n")
}

/// Generate a social network graph
fn generate_social_network(person_count: usize, avg_friends: usize) -> String {
    let mut triples = Vec::new();

    for i in 0..person_count {
        // Person type and properties
        triples.push(format!(
            "<http://example.org/person/{}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://xmlns.com/foaf/0.1/Person> .",
            i
        ));
        triples.push(format!(
            "<http://example.org/person/{}> <http://xmlns.com/foaf/0.1/name> \"Person {}\" .",
            i, i
        ));
        triples.push(format!(
            "<http://example.org/person/{}> <http://xmlns.com/foaf/0.1/age> \"{}\"^^<http://www.w3.org/2001/XMLSchema#integer> .",
            i, 18 + (i % 62)
        ));

        // Knows relationships
        for j in 0..avg_friends {
            let friend = (i + j * 13 + 1) % person_count;
            if friend != i {
                triples.push(format!(
                    "<http://example.org/person/{}> <http://xmlns.com/foaf/0.1/knows> <http://example.org/person/{}> .",
                    i, friend
                ));
            }
        }
    }

    triples.join("\n")
}

// ============================================================================
// Bulk Insert Benchmarks
// ============================================================================

mod bulk_insert_benchmarks {
    use super::*;

    /// Benchmark helper for bulk insert
    fn benchmark_bulk_insert(ctx: &mut BenchContext, name: &str, triple_count: usize, batch_size: usize) -> BenchmarkResult {
        let graph_key = format!("bench_{}", name.replace(" ", "_").to_lowercase());
        ctx.cleanup_graph(&graph_key);

        // Calculate entities needed for the target triple count
        // Each entity generates ~5 triples (1 type + 2 props + 2 rels)
        let entity_count = triple_count / 5;
        let data = generate_ntriples(entity_count, 2, 2);
        let actual_triple_count = data.lines().count();

        println!("\n=== {} ===", name);
        println!("Generating {} triples ({} entities)...", actual_triple_count, entity_count);

        let start = Instant::now();

        if batch_size > 0 && actual_triple_count > batch_size {
            // Insert in batches
            let lines: Vec<&str> = data.lines().collect();
            let mut inserted = 0;

            for chunk in lines.chunks(batch_size) {
                let batch_data = chunk.join("\n");
                let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
                    .arg(&graph_key)
                    .arg(&batch_data)
                    .query(ctx.conn());

                if result.is_err() {
                    println!("Error inserting batch: {:?}", result.err());
                    break;
                }
                inserted += chunk.len();

                if inserted % 100_000 == 0 {
                    println!("  Inserted {} triples...", inserted);
                }
            }
        } else {
            // Insert all at once
            let result: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
                .arg(&graph_key)
                .arg(&data)
                .query(ctx.conn());

            if result.is_err() {
                println!("Error: {:?}", result.err());
            }
        }

        let duration = start.elapsed();
        let result = BenchmarkResult::new(name, duration, 1).with_items(actual_triple_count);
        result.print();

        ctx.cleanup_graph(&graph_key);
        result
    }

    #[test]
    #[ignore]
    fn bench_insert_100k_triples() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        benchmark_bulk_insert(&mut ctx, "Insert 100K triples", 100_000, 10_000);
    }

    #[test]
    #[ignore]
    fn bench_insert_1m_triples() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        benchmark_bulk_insert(&mut ctx, "Insert 1M triples", 1_000_000, 50_000);
    }

    #[test]
    #[ignore]
    fn bench_insert_10m_triples() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        benchmark_bulk_insert(&mut ctx, "Insert 10M triples", 10_000_000, 100_000);
    }

    #[test]
    #[ignore]
    fn bench_insert_100m_triples() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        benchmark_bulk_insert(&mut ctx, "Insert 100M triples", 100_000_000, 500_000);
    }

    #[test]
    #[ignore]
    fn bench_insert_batch_sizes() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");

        println!("\n========================================");
        println!("Batch Size Comparison (1M triples)");
        println!("========================================");

        let batch_sizes = [1_000, 10_000, 50_000, 100_000];
        let mut results = Vec::new();

        for &batch_size in &batch_sizes {
            let name = format!("1M triples, batch={}", batch_size);
            let result = benchmark_bulk_insert(&mut ctx, &name, 1_000_000, batch_size);
            results.push((batch_size, result));
        }

        println!("\n=== Batch Size Summary ===");
        for (batch_size, result) in &results {
            println!(
                "Batch {}: {:.2} triples/sec ({:.2?})",
                batch_size,
                result.items_per_sec.unwrap_or(0.0),
                result.duration
            );
        }
    }
}

// ============================================================================
// Query Benchmarks
// ============================================================================

mod query_benchmarks {
    use super::*;

    /// Set up a test graph for query benchmarks
    fn setup_query_benchmark_graph(ctx: &mut BenchContext, graph_key: &str, person_count: usize) {
        ctx.cleanup_graph(graph_key);

        let data = generate_social_network(person_count, 10);
        let triple_count = data.lines().count();

        println!("Setting up graph with {} people ({} triples)...", person_count, triple_count);

        let start = Instant::now();

        // Insert in batches for large graphs
        let lines: Vec<&str> = data.lines().collect();
        for chunk in lines.chunks(50_000) {
            let batch = chunk.join("\n");
            let _: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
                .arg(graph_key)
                .arg(&batch)
                .query(ctx.conn());
        }

        println!("Graph setup complete in {:?}", start.elapsed());
    }

    /// Run a query multiple times and return average duration
    fn benchmark_query(ctx: &mut BenchContext, name: &str, graph_key: &str, query: &str, iterations: usize) -> BenchmarkResult {
        println!("\n=== {} ===", name);
        println!("Query: {}", query);
        println!("Iterations: {}", iterations);

        // Warmup
        for _ in 0..3 {
            let _: RedisResult<redis::Value> = redis::cmd("RDF.QUERY")
                .arg(graph_key)
                .arg(query)
                .query(ctx.conn());
        }

        let start = Instant::now();
        let mut success_count = 0;

        for _ in 0..iterations {
            let result: RedisResult<redis::Value> = redis::cmd("RDF.QUERY")
                .arg(graph_key)
                .arg(query)
                .query(ctx.conn());

            if result.is_ok() {
                success_count += 1;
            }
        }

        let duration = start.elapsed();
        let result = BenchmarkResult::new(name, duration, iterations);
        result.print();
        println!("Success rate: {}/{}", success_count, iterations);

        result
    }

    #[test]
    #[ignore]
    fn bench_simple_select() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_query_simple";

        setup_query_benchmark_graph(&mut ctx, graph_key, 10_000);

        // Simple SELECT - get all people
        benchmark_query(
            &mut ctx,
            "Simple SELECT (all people)",
            graph_key,
            "SELECT ?p WHERE { ?p a <http://xmlns.com/foaf/0.1/Person> } LIMIT 100",
            100,
        );

        // Simple SELECT with filter
        benchmark_query(
            &mut ctx,
            "Simple SELECT with FILTER",
            graph_key,
            "SELECT ?p ?name WHERE { ?p <http://xmlns.com/foaf/0.1/name> ?name . FILTER(CONTAINS(?name, '5')) } LIMIT 100",
            100,
        );

        ctx.cleanup_graph(graph_key);
    }

    #[test]
    #[ignore]
    fn bench_join_queries() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_query_join";

        setup_query_benchmark_graph(&mut ctx, graph_key, 10_000);

        // 2-way join
        benchmark_query(
            &mut ctx,
            "2-way JOIN (person knows person)",
            graph_key,
            "SELECT ?p1 ?p2 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows> ?p2 } LIMIT 1000",
            50,
        );

        // 3-way join (friend of friend)
        benchmark_query(
            &mut ctx,
            "3-way JOIN (friend of friend)",
            graph_key,
            "SELECT ?p1 ?p2 ?p3 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows> ?p2 . ?p2 <http://xmlns.com/foaf/0.1/knows> ?p3 } LIMIT 1000",
            30,
        );

        // 4-way join
        benchmark_query(
            &mut ctx,
            "4-way JOIN (3-hop path)",
            graph_key,
            "SELECT ?p1 ?p4 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows> ?p2 . ?p2 <http://xmlns.com/foaf/0.1/knows> ?p3 . ?p3 <http://xmlns.com/foaf/0.1/knows> ?p4 } LIMIT 100",
            20,
        );

        ctx.cleanup_graph(graph_key);
    }

    #[test]
    #[ignore]
    fn bench_property_path_queries() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_query_path";

        setup_query_benchmark_graph(&mut ctx, graph_key, 5_000);

        // Transitive closure (0 or more hops)
        benchmark_query(
            &mut ctx,
            "Property Path (knows*)",
            graph_key,
            "SELECT ?p1 ?p2 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows>* ?p2 } LIMIT 100",
            20,
        );

        // One or more hops
        benchmark_query(
            &mut ctx,
            "Property Path (knows+)",
            graph_key,
            "SELECT ?p1 ?p2 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows>+ ?p2 } LIMIT 100",
            20,
        );

        // Bounded path (1-3 hops)
        benchmark_query(
            &mut ctx,
            "Property Path (knows{1,3})",
            graph_key,
            "SELECT ?p1 ?p2 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows>{1,3} ?p2 } LIMIT 100",
            20,
        );

        ctx.cleanup_graph(graph_key);
    }

    #[test]
    #[ignore]
    fn bench_aggregate_queries() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_query_agg";

        setup_query_benchmark_graph(&mut ctx, graph_key, 10_000);

        // COUNT
        benchmark_query(
            &mut ctx,
            "Aggregate COUNT",
            graph_key,
            "SELECT (COUNT(?p) AS ?count) WHERE { ?p a <http://xmlns.com/foaf/0.1/Person> }",
            50,
        );

        // COUNT with GROUP BY
        benchmark_query(
            &mut ctx,
            "Aggregate COUNT with GROUP BY",
            graph_key,
            "SELECT ?age (COUNT(?p) AS ?count) WHERE { ?p <http://xmlns.com/foaf/0.1/age> ?age } GROUP BY ?age",
            50,
        );

        // AVG
        benchmark_query(
            &mut ctx,
            "Aggregate AVG",
            graph_key,
            "SELECT (AVG(?age) AS ?avgAge) WHERE { ?p <http://xmlns.com/foaf/0.1/age> ?age }",
            50,
        );

        // MAX/MIN
        benchmark_query(
            &mut ctx,
            "Aggregate MAX/MIN",
            graph_key,
            "SELECT (MAX(?age) AS ?max) (MIN(?age) AS ?min) WHERE { ?p <http://xmlns.com/foaf/0.1/age> ?age }",
            50,
        );

        ctx.cleanup_graph(graph_key);
    }
}

// ============================================================================
// Cypher Comparison Benchmarks
// ============================================================================

mod cypher_comparison {
    use super::*;

    /// Set up the same data in both RDF and native Cypher formats
    fn setup_comparison_graph(ctx: &mut BenchContext, rdf_graph: &str, cypher_graph: &str, person_count: usize) {
        ctx.cleanup_graph(rdf_graph);
        ctx.cleanup_graph(cypher_graph);

        // Set up RDF graph
        let rdf_data = generate_social_network(person_count, 5);
        let triple_count = rdf_data.lines().count();
        println!("Setting up RDF graph ({} triples)...", triple_count);

        let lines: Vec<&str> = rdf_data.lines().collect();
        for chunk in lines.chunks(10_000) {
            let batch = chunk.join("\n");
            let _: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
                .arg(rdf_graph)
                .arg(&batch)
                .query(ctx.conn());
        }

        // Set up native Cypher graph with equivalent data
        println!("Setting up native Cypher graph...");

        // Create people
        for i in 0..person_count {
            let cypher = format!(
                "CREATE (p:Person {{id: {}, name: 'Person {}', age: {}}})",
                i, i, 18 + (i % 62)
            );
            let _: RedisResult<redis::Value> = redis::cmd("GRAPH.QUERY")
                .arg(cypher_graph)
                .arg(&cypher)
                .query(ctx.conn());
        }

        // Create relationships in batches
        let mut rels = Vec::new();
        for i in 0..person_count {
            for j in 0..5 {
                let friend = (i + j * 13 + 1) % person_count;
                if friend != i {
                    rels.push((i, friend));
                }
            }
        }

        for chunk in rels.chunks(1000) {
            let merges: Vec<String> = chunk
                .iter()
                .map(|(from, to)| format!(
                    "MATCH (a:Person {{id: {}}}), (b:Person {{id: {}}}) CREATE (a)-[:KNOWS]->(b)",
                    from, to
                ))
                .collect();

            for merge in merges {
                let _: RedisResult<redis::Value> = redis::cmd("GRAPH.QUERY")
                    .arg(cypher_graph)
                    .arg(&merge)
                    .query(ctx.conn());
            }
        }

        println!("Both graphs ready for comparison.");
    }

    /// Benchmark a SPARQL query
    fn bench_sparql(ctx: &mut BenchContext, graph_key: &str, query: &str, iterations: usize) -> Duration {
        let start = Instant::now();
        for _ in 0..iterations {
            let _: RedisResult<redis::Value> = redis::cmd("RDF.QUERY")
                .arg(graph_key)
                .arg(query)
                .query(ctx.conn());
        }
        start.elapsed()
    }

    /// Benchmark a Cypher query
    fn bench_cypher(ctx: &mut BenchContext, graph_key: &str, query: &str, iterations: usize) -> Duration {
        let start = Instant::now();
        for _ in 0..iterations {
            let _: RedisResult<redis::Value> = redis::cmd("GRAPH.QUERY")
                .arg(graph_key)
                .arg(query)
                .query(ctx.conn());
        }
        start.elapsed()
    }

    #[test]
    #[ignore]
    fn bench_sparql_vs_cypher() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let rdf_graph = "bench_cmp_rdf";
        let cypher_graph = "bench_cmp_cypher";

        setup_comparison_graph(&mut ctx, rdf_graph, cypher_graph, 1_000);

        println!("\n========================================");
        println!("SPARQL vs Cypher Comparison");
        println!("========================================");

        // Test 1: Simple match
        let iterations = 50;

        let sparql_simple = "SELECT ?p WHERE { ?p a <http://xmlns.com/foaf/0.1/Person> } LIMIT 100";
        let cypher_simple = "MATCH (p:Person) RETURN p LIMIT 100";

        let sparql_time = bench_sparql(&mut ctx, rdf_graph, sparql_simple, iterations);
        let cypher_time = bench_cypher(&mut ctx, cypher_graph, cypher_simple, iterations);

        println!("\n--- Simple Match (LIMIT 100) ---");
        println!("SPARQL: {:?} ({:.2} queries/sec)", sparql_time, iterations as f64 / sparql_time.as_secs_f64());
        println!("Cypher: {:?} ({:.2} queries/sec)", cypher_time, iterations as f64 / cypher_time.as_secs_f64());
        println!("Ratio: {:.2}x", sparql_time.as_secs_f64() / cypher_time.as_secs_f64());

        // Test 2: Join (friend of friend)
        let sparql_join = "SELECT ?p1 ?p3 WHERE { ?p1 <http://xmlns.com/foaf/0.1/knows> ?p2 . ?p2 <http://xmlns.com/foaf/0.1/knows> ?p3 } LIMIT 100";
        let cypher_join = "MATCH (p1:Person)-[:KNOWS]->(p2:Person)-[:KNOWS]->(p3:Person) RETURN p1, p3 LIMIT 100";

        let sparql_time = bench_sparql(&mut ctx, rdf_graph, sparql_join, iterations);
        let cypher_time = bench_cypher(&mut ctx, cypher_graph, cypher_join, iterations);

        println!("\n--- 2-hop Join (friend of friend, LIMIT 100) ---");
        println!("SPARQL: {:?} ({:.2} queries/sec)", sparql_time, iterations as f64 / sparql_time.as_secs_f64());
        println!("Cypher: {:?} ({:.2} queries/sec)", cypher_time, iterations as f64 / cypher_time.as_secs_f64());
        println!("Ratio: {:.2}x", sparql_time.as_secs_f64() / cypher_time.as_secs_f64());

        // Test 3: Aggregate
        let sparql_agg = "SELECT (COUNT(?p) AS ?count) WHERE { ?p a <http://xmlns.com/foaf/0.1/Person> }";
        let cypher_agg = "MATCH (p:Person) RETURN COUNT(p)";

        let sparql_time = bench_sparql(&mut ctx, rdf_graph, sparql_agg, iterations);
        let cypher_time = bench_cypher(&mut ctx, cypher_graph, cypher_agg, iterations);

        println!("\n--- COUNT Aggregate ---");
        println!("SPARQL: {:?} ({:.2} queries/sec)", sparql_time, iterations as f64 / sparql_time.as_secs_f64());
        println!("Cypher: {:?} ({:.2} queries/sec)", cypher_time, iterations as f64 / cypher_time.as_secs_f64());
        println!("Ratio: {:.2}x", sparql_time.as_secs_f64() / cypher_time.as_secs_f64());

        ctx.cleanup_graph(rdf_graph);
        ctx.cleanup_graph(cypher_graph);
    }
}

// ============================================================================
// Export/Roundtrip Benchmarks
// ============================================================================

mod export_benchmarks {
    use super::*;

    #[test]
    #[ignore]
    fn bench_export_formats() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_export";

        // Set up graph
        let data = generate_social_network(1_000, 5);
        let triple_count = data.lines().count();
        println!("Setting up graph ({} triples)...", triple_count);

        let _: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
            .arg(graph_key)
            .arg(&data)
            .query(ctx.conn());

        let formats = ["ntriples", "turtle", "jsonld"];
        let iterations = 20;

        println!("\n========================================");
        println!("Export Format Benchmarks");
        println!("========================================");

        for format in &formats {
            let start = Instant::now();

            for _ in 0..iterations {
                let _: RedisResult<redis::Value> = redis::cmd("RDF.EXPORT")
                    .arg(graph_key)
                    .arg("FORMAT")
                    .arg(*format)
                    .query(ctx.conn());
            }

            let duration = start.elapsed();
            println!(
                "\n{}: {:?} avg ({:.2} exports/sec)",
                format,
                duration / iterations as u32,
                iterations as f64 / duration.as_secs_f64()
            );
        }

        ctx.cleanup_graph(graph_key);
    }

    #[test]
    #[ignore]
    fn bench_roundtrip() {
        let mut ctx = BenchContext::new().expect("Failed to create benchmark context");
        let graph_key = "bench_roundtrip";

        // Generate data
        let original_data = generate_social_network(500, 5);
        let triple_count = original_data.lines().count();

        println!("\n========================================");
        println!("Roundtrip Benchmark ({} triples)", triple_count);
        println!("========================================");

        let iterations = 10;
        let start = Instant::now();

        for i in 0..iterations {
            ctx.cleanup_graph(graph_key);

            // Import
            let _: RedisResult<redis::Value> = redis::cmd("RDF.INSERT")
                .arg(graph_key)
                .arg(&original_data)
                .query(ctx.conn());

            // Export
            let _exported: RedisResult<String> = redis::cmd("RDF.EXPORT")
                .arg(graph_key)
                .arg("FORMAT")
                .arg("ntriples")
                .query(ctx.conn());

            if (i + 1) % 5 == 0 {
                println!("  Completed {} roundtrips...", i + 1);
            }
        }

        let duration = start.elapsed();
        println!(
            "\nTotal: {:?} ({:.2} roundtrips/sec, {:.2} triples/sec)",
            duration,
            iterations as f64 / duration.as_secs_f64(),
            (iterations * triple_count) as f64 / duration.as_secs_f64()
        );

        ctx.cleanup_graph(graph_key);
    }
}

// ============================================================================
// Summary
// ============================================================================

#[test]
#[ignore]
fn bench_summary() {
    println!("
========================================
FalkorSemantic Performance Benchmarks
========================================

Available benchmark tests:

BULK INSERT:
  - bench_insert_100k_triples    : Insert 100,000 triples
  - bench_insert_1m_triples      : Insert 1,000,000 triples
  - bench_insert_10m_triples     : Insert 10,000,000 triples
  - bench_insert_100m_triples    : Insert 100,000,000 triples
  - bench_insert_batch_sizes     : Compare different batch sizes

QUERY PERFORMANCE:
  - bench_simple_select          : Simple SELECT queries
  - bench_join_queries           : 2-4 way JOIN queries
  - bench_property_path_queries  : Property path queries (*, +, {{n,m}})
  - bench_aggregate_queries      : COUNT, AVG, MAX, MIN, GROUP BY

COMPARISON:
  - bench_sparql_vs_cypher       : Compare SPARQL with native Cypher

EXPORT:
  - bench_export_formats         : Compare N-Triples, Turtle, JSON-LD export
  - bench_roundtrip              : Full import/export cycle

Run with:
  TEST_REDIS_PORT=6399 cargo test --test benchmarks -- --ignored --nocapture --test-threads=1

For specific benchmarks:
  cargo test --test benchmarks bench_insert_1m_triples -- --ignored --nocapture
");
}
