[![CI](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml/badge.svg)](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml)
[![License: AGPL](https://img.shields.io/badge/license-AGPL--v3-blue)](https://opensource.org/licenses/agpl-v3)
[![Codecov](https://codecov.io/gh/falkordb/FalkorSemantic/branch/main/graph/badge.svg)](https://codecov.io/gh/falkordb/FalkorSemantic)

# FalkorSemantic

A high-performance Redis module that extends [FalkorDB](https://www.falkordb.com/) with RDF and SPARQL capabilities, bridging the property graph and semantic web worlds.
 
## Overview

FalkorSemantic is a Redis module that enables semantic web data processing with FalkorDB. It provides:

- **RDF Data Support**: Store and manage RDF triples in FalkorDB's graph engine
- **SPARQL Queries**: Query your data using the standard semantic web query language
- **Multiple Formats**: Import RDF in Turtle, N-Triples, N-Quads, and TriG (named graph support is limited — see command docs for details)
- **High Performance**: Leverage FalkorDB's speed for semantic workloads
- **Standards Compliant**: SPARQL 1.1 query support

## Architecture

The project is organized as a Cargo workspace with four main crates:

- **`parser`**: Parses RDF formats (Turtle, N-Triples, N-Quads, TriG) and SPARQL queries
- **`mapper`**: Maps RDF triples to FalkorDB graph structures and translates SPARQL to Cypher
- **`storage`**: Dictionary and namespace storage for efficient IRI handling
- **`module`**: Redis module that exposes RDF/SPARQL commands

```text
┌─────────────────────────────────────────────────────┐
│                   Redis Module API                   │
│         (RDF.INSERT, RDF.QUERY, etc.)                │
├─────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────┐ │
│  │     Parser      │    │        Mapper           │ │
│  │  ├─ RDF         │    │  ├─ RDF → Graph         │ │
│  │  │  (Turtle,    │    │  └─ SPARQL → Cypher     │ │
│  │  │   N-Triples, │    │                         │ │
│  │  │   N-Quads,   │    │                         │ │
│  │  │   TriG)      │    │                         │ │
│  │  └─ SPARQL      │    │                         │ │
│  └────────┬────────┘    └───────────┬─────────────┘ │
│           └──────────────┬──────────┘               │
├──────────────────────────┼──────────────────────────┤
│              FalkorDB Core (Cypher Engine)          │
├─────────────────────────────────────────────────────┤
│                        Redis                         │
└─────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Docker (recommended), or
- Rust 1.88 or later (for building from source)

### Installation

#### Using Docker (Recommended)

```bash
docker run -d --name falkorsemantic -p 6379:6379 falkordb/falkorsemantic:edge
```

This gives you FalkorDB with full RDF/SPARQL support on `localhost:6379`.

#### Building from Source

```bash
git clone https://github.com/FalkorDB/FalkorSemantic.git
cd FalkorSemantic
make docker-build
make docker-run
```

#### Building the Module Only

```bash
cargo build --release --package falkorsemantic-module
# Requires a running FalkorDB instance to load into
```

### Basic Usage

```bash
# Connect to Redis
redis-cli

# Insert RDF data (format is auto-detected, or specify with FORMAT)
RDF.INSERT mykg '
  @prefix ex: <http://example.org/> .
  @prefix foaf: <http://xmlns.com/foaf/0.1/> .
  
  ex:alice foaf:name "Alice" ;
           foaf:knows ex:bob .
  ex:bob foaf:name "Bob" .
'

# Query with SPARQL
RDF.QUERY mykg '
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name ?friendName
  WHERE {
    ?person foaf:name ?name ;
            foaf:knows ?friend .
    ?friend foaf:name ?friendName .
  }
'
```

## Commands

### RDF.INSERT

Insert RDF triples into a graph.

```
RDF.INSERT <graph> <data> [FORMAT <format>] [ATOMIC]
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the target graph |
| `data` | RDF data as a string |
| `FORMAT` | Optional format: `turtle`, `ntriples`, `nquads`, `trig` (auto-detected if omitted). Note: `nquads` is parsed via the N-Triples parser — lines with a 4th graph term will error; only graph-less quads are accepted |
| `ATOMIC` | Optional flag to execute all inserts as a single transaction |

**Example:**
```bash
RDF.INSERT mykg '
<http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
<http://example.org/alice> <http://xmlns.com/foaf/0.1/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
'
```

### RDF.BULK_INSERT

Bulk insert RDF data from a file.

```
RDF.BULK_INSERT <graph> <file_path> [FORMAT <format>] [BATCH <size>] [SKIP <lines>] [MAXERRORS <count>] [STOPONERROR]
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the target graph |
| `file_path` | Path to the RDF file |
| `FORMAT` | Optional format (auto-detected from extension if omitted) |
| `BATCH` | Batch size for processing (default: 1000) |
| `SKIP` | Number of lines to skip (for recovery from partial failures) |
| `MAXERRORS` | Maximum errors before stopping (default: unlimited) |
| `STOPONERROR` | Stop on first error instead of continuing |

**Example:**
```bash
RDF.BULK_INSERT dbpedia /data/dbpedia.nt FORMAT ntriples BATCH 50000
```

### RDF.QUERY

Execute a SPARQL query.

```text
RDF.QUERY <graph> <query> [FORMAT <format>] [TIMEOUT <ms>]
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the graph to query |
| `query` | SPARQL query string |
| `FORMAT` | Output format: `json` (default), `xml`, `csv`, `tsv` |
| `TIMEOUT` | Query timeout in milliseconds |

**Example:**
```bash
RDF.QUERY mykg '
  PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
  SELECT ?class (COUNT(?s) AS ?count)
  WHERE { ?s rdf:type ?class }
  GROUP BY ?class
  ORDER BY DESC(?count)
  LIMIT 10
' FORMAT json
```

### RDF.DELETE

Delete triples matching a pattern. Use `*` as a wildcard.

```
RDF.DELETE <graph> <subject> <predicate> <object> [GRAPH <named_graph>] [ORPHANS]
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the target graph |
| `subject` | Subject IRI, blank node, or `*` wildcard |
| `predicate` | Predicate IRI or `*` wildcard |
| `object` | Object IRI, literal, or `*` wildcard |
| `GRAPH` | Optional: scope deletion to a specific named graph |
| `ORPHANS` | Optional: also delete nodes that become orphaned |

**Example:**
```bash
# Delete all triples about alice
RDF.DELETE mykg "<http://example.org/alice>" "*" "*"

# Delete and clean up orphaned nodes
RDF.DELETE mykg "<http://example.org/alice>" "*" "*" ORPHANS
```

### RDF.NAMESPACES

Manage namespace prefixes.

```
RDF.NAMESPACES <graph> LIST
RDF.NAMESPACES <graph> ADD <prefix> <uri>
RDF.NAMESPACES <graph> REMOVE <prefix>
```

### RDF.GRAPH

Manage RDF graphs.

```
RDF.GRAPH LIST
RDF.GRAPH CREATE <graph>
RDF.GRAPH DROP <graph>
RDF.GRAPH CLEAR <graph>
```

## SPARQL Support

### Query Forms

| Form | Status | Notes |
|------|--------|-------|
| SELECT | ✅ | Including DISTINCT, ORDER BY, LIMIT/OFFSET |
| ASK | ✅ | Full support |
| CONSTRUCT | 🚧 | Planned ([#53](https://github.com/FalkorDB/FalkorSemantic/issues/53)) |
| DESCRIBE | 🚧 | Planned ([#54](https://github.com/FalkorDB/FalkorSemantic/issues/54)) |

### Graph Patterns

| Feature | Status | Notes |
|---------|--------|-------|
| Basic Graph Patterns | ✅ | |
| OPTIONAL | ✅ | |
| UNION | ✅ | Top-level UNION with DISTINCT support |
| MINUS | ✅ | |
| FILTER | ✅ | See supported functions below |
| BIND | ✅ | Variable tracked via Extend pattern |
| VALUES | ✅ | Single-variable and multi-variable support |
| Subqueries | 🚧 | Flattened instead of nested ([#78](https://github.com/FalkorDB/FalkorSemantic/issues/78)) |
| Named Graphs (GRAPH) | 🚧 | Parsed but inner pattern translated without graph scoping |

### Property Paths

Property paths are parsed but currently simplified to a generic traversal pattern. Full path semantics are planned ([#77](https://github.com/FalkorDB/FalkorSemantic/issues/77)).

| Path | Syntax | Status |
|------|--------|--------|
| Sequence | a/b | 🚧 |
| Alternative | a\|b | 🚧 |
| Inverse | ^a | 🚧 |
| Zero-or-more | a* | 🚧 |
| One-or-more | a+ | 🚧 |
| Zero-or-one | a? | 🚧 |

### Functions & Expressions

| Feature | Status | Notes |
|---------|--------|-------|
| Comparison (=, !=, <, >, <=, >=) | ✅ | |
| Logical (&&, \|\|, !) | ✅ | |
| Arithmetic (+, -, *, /) | ✅ | Including unary plus/minus |
| `STR`, `IRI`, `BNODE` | ✅ | |
| `BOUND`, `IF`, `COALESCE` | ✅ | |
| `STRLEN`, `UCASE`, `LCASE` | ✅ | |
| `STRSTARTS`, `STRENDS`, `CONTAINS` | ✅ | |
| `REGEX` | ✅ | |
| `SUBSTR`, `CONCAT`, `REPLACE` | ✅ | |
| `LANG`, `DATATYPE` | ✅ | |
| `isIRI`, `isBlank`, `isLiteral`, `isNumeric` | ✅ | |
| `ABS`, `CEIL`, `FLOOR`, `ROUND` | ✅ | |
| `EXISTS` / `NOT EXISTS` | ✅ | |
| `IN` / `NOT IN` | ✅ / 🚧 | NOT IN planned ([#66](https://github.com/FalkorDB/FalkorSemantic/issues/66)) |
| Aggregates (`COUNT`, `SUM`, `AVG`, `MIN`, `MAX`) | 🚧 | Planned ([#71](https://github.com/FalkorDB/FalkorSemantic/issues/71)) |
| `GROUP BY` / `HAVING` | 🚧 | Pattern traversed, no aggregate translation ([#72](https://github.com/FalkorDB/FalkorSemantic/issues/72)) |

## RDF to Graph Mapping

FalkorSemantic maps RDF triples to FalkorDB's property graph model:

| RDF | FalkorDB |
|-----|----------|
| Subject (IRI) | Node with `:Resource` label, `uri` property |
| Subject (Blank Node) | Node with `:BNode` label |
| Predicate | Edge with label (local name) |
| Object (IRI) | Node with `:Resource` label |
| Object (Literal) | Property on subject node |
| rdf:type | Additional node label |

**Example:**

```turtle
# RDF (Turtle)
ex:alice a foaf:Person ;
         foaf:name "Alice" ;
         foaf:knows ex:bob .
```

```cypher
// FalkorDB equivalent
CREATE (alice:Resource:Person {uri: 'http://example.org/alice', 'foaf:name': 'Alice'})
CREATE (bob:Resource {uri: 'http://example.org/bob'})
CREATE (alice)-[:knows]->(bob)
```

## Development

### Building

```bash
# Build all crates
cargo build

# Build specific crate
cargo build --package falkorsemantic-parser

# Build release version
cargo build --release
```

### Testing

```bash
# Run unit tests
cargo test --workspace --exclude falkorsemantic-module

# Run integration tests (requires running FalkorSemantic container)
make docker-run
cargo test --test integration -- --ignored
make docker-stop
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit
```

## Project Structure

```
FalkorSemantic/
├── parser/              # RDF and SPARQL parsing
│   ├── src/
│   │   ├── rdf/        # RDF data types and parsers
│   │   │   ├── jsonld/ # JSON-LD parser (expansion, compaction, framing)
│   │   │   ├── serializer/ # RDF serialization
│   │   │   └── ...     # IRIs, literals, triples, namespaces
│   │   ├── sparql/     # SPARQL query parser
│   │   ├── formats/    # Format readers (N-Triples, TriG, Turtle)
│   │   ├── results/    # SPARQL result serializers (JSON, XML, CSV, TSV)
│   │   └── export/     # RDF export utilities
│   └── Cargo.toml
├── mapper/              # Graph mapping and query translation
│   ├── src/
│   │   ├── graph/      # RDF to FalkorDB graph mapping
│   │   └── query/      # SPARQL to Cypher translation
│   └── Cargo.toml
├── storage/             # Storage utilities (dictionary, namespace, cache)
│   └── Cargo.toml
├── module/              # Redis module implementation
│   ├── src/
│   │   └── commands/   # RDF.* command handlers
│   └── Cargo.toml
├── tests-e2e/           # End-to-end tests
├── tests-compliance/    # SPARQL compliance tests
├── scripts/             # Utility scripts
├── .github/workflows/   # CI/CD pipelines
├── Dockerfile           # Production Docker image
├── docker/              # Docker entrypoint scripts
└── Cargo.toml          # Workspace configuration
```

## Client Examples

### Python

```python
import redis
import json

r = redis.Redis(host='localhost', port=6379)

# Insert data
r.execute_command('RDF.INSERT', 'mykg', '''
    @prefix ex: <http://example.org/> .
    ex:product1 ex:name "Widget" ; ex:price 29.99 .
''')

# Query
result = r.execute_command('RDF.QUERY', 'mykg', '''
    PREFIX ex: <http://example.org/>
    SELECT ?name ?price WHERE {
        ?product ex:name ?name ; ex:price ?price .
    }
''')
print(json.loads(result))
```

### Node.js

```javascript
import { createClient } from 'redis';

const client = createClient();
await client.connect();

await client.sendCommand(['RDF.INSERT', 'mykg', `
  @prefix schema: <http://schema.org/> .
  <http://example.org/event1> a schema:Event ;
    schema:name "Tech Conference" .
`]);

const result = await client.sendCommand(['RDF.QUERY', 'mykg', `
  PREFIX schema: <http://schema.org/>
  SELECT ?name WHERE { ?e schema:name ?name }
`]);
console.log(JSON.parse(result));
```

## Roadmap

- [x] Project structure and CI/CD
- [x] RDF parser implementation (Turtle, N-Triples, N-Quads, TriG)
- [x] RDF to graph mapping
- [x] SPARQL parser integration
- [x] SPARQL to Cypher translation
- [x] Core Redis commands (INSERT, QUERY, DELETE)
- [ ] JSON-LD support (parser exists, module integration pending)
- [ ] Property paths (parsed, full semantics planned)
- [ ] Aggregates and GROUP BY
- [ ] Performance optimization
- [ ] SPARQL UPDATE support
- [ ] RDFS inference

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Development setup
- Coding standards
- Testing requirements
- Pull request process

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [redis-module](https://crates.io/crates/redis-module)
- Designed to work with [FalkorDB](https://github.com/FalkorDB/FalkorDB)
- SPARQL parsing via [spargebra](https://crates.io/crates/spargebra)
- RDF parsing via [rio](https://crates.io/crates/rio_turtle)

## Support

- **Issues**: [GitHub Issues](https://github.com/FalkorDB/FalkorSemantic/issues)
- **Discussions**: [GitHub Discussions](https://github.com/FalkorDB/FalkorSemantic/discussions)
- **Documentation**: [Wiki](https://github.com/FalkorDB/FalkorSemantic/wiki)

## Docker Image

The production Docker image is published to Docker Hub as [`falkordb/falkorsemantic`](https://hub.docker.com/r/falkordb/falkorsemantic).

| Tag | Description |
|-----|-------------|
| `edge` | Latest build from `main` branch |
| `x.y.z` | Specific release version |
| `latest` | Most recent tagged release |

```bash
docker run -d --name falkorsemantic -p 6379:6379 falkordb/falkorsemantic:edge
```

## CI/CD

The project uses GitHub Actions for continuous integration:

- ✅ Build verification on stable, beta, and nightly Rust
- ✅ Automated testing
- ✅ Code formatting checks
- ✅ Linting with Clippy
- ✅ Security audits
- ✅ Docker image build and publish

See the [CI workflow](.github/workflows/ci.yml) and [Docker workflow](.github/workflows/docker.yml) for details.
