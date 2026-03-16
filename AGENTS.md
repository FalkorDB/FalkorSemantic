# FalkorSemantic - Agent Documentation

This document provides comprehensive information about the FalkorSemantic project structure, commands, and architecture for AI agents and developers.

## Project Overview

**License**: GNU AFFERO GENERAL PUBLIC LICENSE v3

**Description**: A high-performance Redis module that extends FalkorDB with RDF and SPARQL capabilities, bridging the property graph and semantic web worlds. It provides RDF data support, SPARQL queries, multiple format support (Turtle, N-Triples, JSON-LD, N-Quads), and SPARQL 1.1 query support.

## Cargo Workspace Structure

### Workspace Configuration
- **Edition**: 2021
- **Version**: 0.1.0
- **Rust Minimum Version**: 1.75
- **Authors**: FalkorDB Team
- **Repository**: https://github.com/FalkorDB/FalkorSemantic

### Workspace Members (4 crates + 3 test crates)
1. **parser** - Parses RDF formats (Turtle, N-Triples, JSON-LD) and SPARQL queries
2. **mapper** - Maps RDF triples to FalkorDB graph structures and translates SPARQL to Cypher
3. **storage** - Dictionary and namespace storage for efficient IRI handling
4. **module** - Redis module that exposes RDF/SPARQL commands (compiles to cdylib)
5. **tests-e2e** - End-to-end tests (requires Redis module loaded)
6. **tests-compliance** - W3C compliance tests for RDF and SPARQL parsing
7. **tests** - Integration tests

### Core Dependencies
- **Redis Module Development**: redis-module 2.0
- **Async Runtime**: tokio 1.50 (full features)
- **Serialization**: serde 1.0 + serde_json 1.0
- **Error Handling**: thiserror 2.0, anyhow 1.0
- **Logging**: log 0.4, env_logger 0.11
- **JSON-LD & RDF Types**: json-ld 0.21, rdf-types 0.22, iref 3.2
- **RDF Parsing**: rio_turtle 0.8, rio_api 0.8
- **SPARQL Parsing**: spargebra 0.4, oxiri 0.2, oxrdf 0.3

## Available Make Targets

```
Building:
  make build          - Build all crates
  make module         - Build Redis module in release mode
  make release        - Build optimized release version

Testing:
  make test           - Run unit tests
  make test-integration - Run integration tests (requires Redis + FalkorDB)
  make test-all       - Run all tests

Code Quality:
  make fmt            - Format code with rustfmt
  make fmt-check      - Check code formatting
  make lint           - Run clippy linter
  make audit          - Run security audit
  make check          - Run fmt-check + lint + test (combined)

Development:
  make dev-up         - Start development environment (Docker)
  make dev-down       - Stop development environment
  make dev-logs       - View development logs
  make dev-restart    - Full dev cycle (down + module build + up)
  make clean          - Clean build artifacts

Utilities:
  make install-tools  - Install cargo-watch, cargo-audit, cargo-expand, rustfmt, clippy
  make redis-cli      - Open Redis CLI (port 6379)
  make falkordb-cli   - Open FalkorDB CLI (port 6380)

Documentation:
  make docs           - Generate documentation
  make docs-open      - Generate and open documentation
```

## Redis Commands (7 Registered Commands)

All commands are registered in `module/src/lib.rs` and implemented in `module/src/commands/`.

### Core RDF Commands

| Command | Type | Args | Line | File | Description |
|---------|------|------|------|------|-------------|
| `rdf.parse` | write | 1-1 | 93 | lib.rs | Parse RDF data (basic implementation, TODO: full parsing) |
| `rdf.insert` | write | 2 to ∞ | 94 | lib.rs | Insert RDF data with format support (Turtle, N-Triples, N-Quads, TriG, JSON-LD) |
| `rdf.bulk_insert` | write | 2 to ∞ | 95 | lib.rs | Bulk insert from files with streaming & batch processing |
| `rdf.delete` | write | 2 to ∞ | 96 | lib.rs | Delete triples matching pattern with wildcard support |
| `rdf.namespaces` | write | 2 to ∞ | 97 | lib.rs | Manage namespace prefix mappings (LIST/ADD/REMOVE) |
| `rdf.graph` | write | 1 to ∞ | 98 | lib.rs | Manage RDF graphs (CREATE/DROP/LIST/CLEAR) |
| `rdf.query` | readonly | 2 to ∞ | 99 | lib.rs | Execute SPARQL queries (JSON/XML/CSV/TSV output) |

### Command Syntax Summary

**rdf.insert**: `RDF.INSERT <graph_key> <format> <data> [graph_key format data ...]`
- Supported formats: Turtle, N-Triples, N-Quads, TriG, JSON-LD
- Supports batch processing and atomic transactions

**rdf.bulk_insert**: `RDF.BULK_INSERT <graph_key> <file_path> [FILE <file_path> ...]`
- Streams from files with batch processing
- Security: Path traversal prevention via canonicalization
- Env var: `RDF_BULK_INSERT_ALLOWED_DIR` for directory restriction
- Default batch size: 1000

**rdf.delete**: `RDF.DELETE <graph_key> <subject> <predicate> <object> [GRAPH <named_graph>] [ORPHANS]`
- Subject/predicate/object can be: full URI, prefixed name, literal, or wildcard (*)
- Options: GRAPH for scoping, ORPHANS to delete disconnected nodes

**rdf.query**: `RDF.QUERY <graph_key> <sparql_query> [FORMAT json|xml|csv|tsv]`
- Default format: JSON Results Format
- Returns SELECT or ASK results

**rdf.namespaces**: `RDF.NAMESPACES <subcommand> [args]`
- Subcommands: LIST, ADD, REMOVE (aliases: DELETE, DEL)

**rdf.graph**: `RDF.GRAPH <subcommand> [graph_key]`
- Subcommands: CREATE, DROP (aliases: DELETE), LIST, CLEAR (aliases: EMPTY)

## Module Command Files

| File | Lines | Purpose |
|------|-------|---------|
| mod.rs | 17 | Command module exports |
| rdf_insert.rs | 400+ | RDF data insertion with format parsing |
| rdf_bulk_insert.rs | 600+ | Bulk file ingestion with streaming |
| rdf_query.rs | 300+ | SPARQL query execution |
| rdf_delete.rs | 700+ | Triple pattern deletion |
| rdf_namespaces.rs | 200+ | Namespace prefix management |
| rdf_graph.rs | 300+ | Graph lifecycle management |
| utils.rs | 50+ | Helper utilities |

## Docker Services

Three services defined in `docker-compose.yml`:

1. **redis** (port 6379)
   - Image: redis:7-alpine
   - Loads FalkorSemantic module from `./target/release/libfalkorsemantic_module.so`
   - Health check: redis-cli ping
   - Volume: redis_data, module mount

2. **falkordb** (port 6380)
   - Image: falkordb/falkordb:latest
   - Health check: redis-cli -p 6379 ping
   - Volume: falkordb_data

3. **dev** (development container)
   - Builds from Dockerfile.dev
   - Mounts workspace at /workspace
   - Depends on: redis (healthy) + falkordb (healthy)
   - Provides full Rust development environment

## SPARQL to Cypher Translation

File: `mapper/src/query/translator.rs` (2062 lines)

### Implemented Features (SUPPORTED)
- ✅ **Query Types**: SELECT, ASK
- ✅ **Graph Patterns**: BGP (Basic Graph Patterns), Join, Left Join (OPTIONAL)
- ✅ **Set Operations**: UNION (top-level only, with DISTINCT/ALL distinction)
- ✅ **Filtering**: FILTER expressions with comprehensive operator support
- ✅ **Variable Binding**: Extend patterns with derived variables
- ✅ **Projections**: SELECT * and specific variable projections
- ✅ **Ordering**: ORDER BY with ASC/DESC
- ✅ **Pagination**: LIMIT and OFFSET (SKIP in Cypher)
- ✅ **Expressions**: Comparison operators (=, !=, <, >, <=, >=), boolean logic (&&, ||, !), arithmetic, string functions, type checking
- ✅ **Functions**: STR(), LANG(), LANGMATCHES(), NOT EXISTS, COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT, SAMPLE, data type functions
- ✅ **Pattern Matching**: Full triple pattern translation with IRI/literal handling
- ✅ **Negation**: MINUS patterns (translated to NOT EXISTS)
- ✅ **VALUES**: VALUES clause for inline data

### NOT Implemented (UNSUPPORTED)
- ❌ **CONSTRUCT queries** - Graph construction not supported
- ❌ **DESCRIBE queries** - Resource description not supported
- ❌ **SERVICE clause** - SPARQL federation not supported
- ❌ **Nested UNION** - UNION can only be top-level pattern (Cypher limitation)
- ❌ **GROUP BY aggregation** - Groups processed but not full aggregate support
- ❌ **Window functions** - Not implemented

### Translation Strategy
1. SELECT queries → Cypher MATCH/WHERE/RETURN
2. ASK queries → COUNT(*) > 0 with result boolean
3. UNION → Multiple Cypher queries combined with UNION/UNION ALL
4. OPTIONAL → Cypher OPTIONAL MATCH
5. FILTER → WHERE clauses
6. Triple patterns → Cypher node/relationship patterns

## Test Infrastructure

### 1. Unit/Integration Tests (`tests/integration.rs`)
- **Location**: `/tests/integration.rs` (82 lines)
- **Setup Required**: Redis server + FalkorDB running
- **Run**: `cargo test --test integration -- --ignored`
- **Tests**:
  - Redis connectivity check
  - FalkorDB connectivity check
  - Module loaded verification
  - RDF.PARSE command test

### 2. E2E Tests (`tests-e2e/`)
- **Location**: `/tests-e2e/src/`
- **Files**: 
  - `e2e.rs` - End-to-end module integration tests
  - `benchmarks.rs` - Performance benchmarks
- **Requirements**: Redis with FalkorSemantic module loaded
- **Test Infrastructure**:
  - Manual Redis setup instructions included
  - Docker container setup support
  - Full stack testing from Redis commands through parser
  - Test threads: 1 (serialized to prevent port conflicts)
- **Benchmarks Scope**:
  - Bulk insert performance (1M, 10M, 100M triples)
  - SPARQL query performance (SELECT, JOIN, property paths, aggregates)
  - Cypher equivalents comparison

### 3. Compliance Tests (`tests-compliance/`)
- **Location**: `/tests-compliance/src/`
- **Files**:
  - `lib.rs` - Main compliance infrastructure
  - `rdf.rs` - RDF 1.1 compliance tests
  - `sparql.rs` - SPARQL 1.1 compliance tests
  - `report.rs` - Markdown report generation
- **Coverage**:
  - Turtle parser compliance (W3C test suite)
  - N-Triples parser compliance (W3C test suite)
  - N-Quads parser compliance (W3C test suite)
  - SPARQL 1.1 query syntax compliance
- **Structure**:
  - Test framework for positive/negative tests
  - Compliance gap tracking with severity levels
  - Report generation in Markdown format
- **Test Sources**:
  - Turtle: https://www.w3.org/2013/TurtleTests/
  - N-Triples: https://www.w3.org/2013/N-TriplesTests/
  - N-Quads: https://www.w3.org/2013/N-QuadsTests/
  - SPARQL: https://www.w3.org/2009/sparql/docs/tests/

### Test Organization Summary
```
tests/                 → Integration tests (requires running services)
tests-e2e/            → End-to-end tests + benchmarks (requires module)
tests-compliance/     → W3C standards compliance testing
```

## CI/CD Workflow

**Workflow File**: `.github/workflows/ci.yml`
**Trigger**: Commits to main/develop or PRs to main/develop
**Concurrency**: Cancels in-progress runs on same branch/PR

### CI Jobs (5 parallel jobs)

1. **Test Job** (tests on stable/beta/nightly)
   - Runs on: ubuntu-latest
   - Matrix: Rust stable, beta, nightly
   - Steps: checkout → install rust → cache cargo → build → run tests
   - Excludes: falkorsemantic-module (requires Redis)

2. **Lint Job**
   - Runs on: ubuntu-latest
   - Tools: rustfmt, clippy
   - Steps: checkout → install rust (stable + tools) → fmt check → clippy

3. **Build Module Job**
   - Runs on: ubuntu-latest
   - Output: Release Redis module (.so)
   - Artifact upload: redis-module
   - Steps: checkout → install rust → build release module

4. **Code Coverage Job**
   - Runs on: ubuntu-latest
   - Tool: cargo-llvm-cov
   - Upload: codecov (conditional on failures for push events)
   - Coverage includes:
     - Workspace coverage (excluding module and e2e)
     - E2E smoke test (module binary exists check)
   - Redis: Installed via apt
   - Output: lcov.info

5. **Security Audit Job**
   - Runs on: ubuntu-latest
   - Tool: rustsec/audit-check (v2)
   - Permissions: read contents, write checks (for check runs)
   - Checks: Cargo.lock for security vulnerabilities

### Environment
- `CARGO_TERM_COLOR: always`

## Project Structure

```
FalkorSemantic/
├── Cargo.toml                 # Workspace configuration
├── Makefile                   # Development targets
├── docker-compose.yml         # Dev environment
├── Dockerfile.dev            # Development container
├── LICENSE                   # AGPL v3
├── README.md                 # Project overview
├── CHANGELOG.md              # Version history
├── COMPLIANCE.md             # Standards compliance
├── CONTRIBUTING.md           # Contribution guidelines
├── SECURITY.md              # Security policy
│
├── parser/                   # RDF/SPARQL parsing crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── formats/          # RDF format parsers
│       ├── rdf/              # RDF data types
│       ├── results/          # Query result formatters
│       └── sparql/           # SPARQL query types
│
├── mapper/                   # RDF→Graph mapping & SPARQL→Cypher crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── graph/            # FalkorDB graph mapping
│       └── query/
│           ├── executor.rs
│           └── translator.rs # SPARQL→Cypher (2062 lines)
│
├── storage/                  # IRI dictionary & namespace storage
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── ...
│
├── module/                   # Redis module entry point
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # Module init + command registration
│       └── commands/
│           ├── mod.rs
│           ├── rdf_insert.rs
│           ├── rdf_bulk_insert.rs
│           ├── rdf_query.rs
│           ├── rdf_delete.rs
│           ├── rdf_namespaces.rs
│           ├── rdf_graph.rs
│           └── utils.rs
│
├── tests/                    # Integration tests
│   └── integration.rs        # Connectivity & module tests
│
├── tests-e2e/               # End-to-end tests
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── e2e.rs          # Full stack integration tests
│       └── benchmarks.rs   # Performance benchmarks
│
├── tests-compliance/        # W3C compliance tests
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Test infrastructure
│       ├── rdf.rs          # RDF 1.1 compliance
│       ├── sparql.rs       # SPARQL 1.1 compliance
│       └── report.rs       # Report generation
│
├── .github/
│   └── workflows/
│       └── ci.yml          # CI/CD pipeline
│
└── docs/                    # Documentation (generated)
```

## Security & Performance Notes

### Security
- **Path Traversal Protection** (rdf_bulk_insert): Canonicalization + ".." rejection
- **Allowed Directory Restriction**: Via `RDF_BULK_INSERT_ALLOWED_DIR` env var
- **Module Verification**: Checks for FalkorDB module on initialization

### Performance Optimizations
- Release profile: opt-level=3, LTO enabled, single codegen unit, stripped
- Bulk insert: Configurable batch processing (default 1000)
- Connection pooling: Async with tokio (full features)
- Module loading: Precompiled .so for runtime efficiency

## Key Implementation Details

### RDF Format Support
- **Turtle** (.ttl): rio_turtle parser + prefix support
- **N-Triples** (.nt): rio line-based format
- **N-Quads** (.nq): Quad format with named graphs
- **TriG**: Turtle with graphs (W3C standard)
- **JSON-LD**: Framework integration (not fully implemented)

### Command Behavior
- All commands are atomic operations within FalkorDB transactions
- Namespace prefixes scoped per graph-key in Redis
- RDF graphs tracked in Redis set: `rdf:graphs`
- Namespace mappings stored with prefix: `rdf:ns:`

### Error Handling
- thiserror-based error types with MapperError, QueryError, ParseError
- Proper Redis error propagation (WrongArity, type errors)
- Validation on input paths and SPARQL queries

---

*Last Updated: 2024-03-16*
*For detailed API documentation, see inline code comments or run `make docs`*
