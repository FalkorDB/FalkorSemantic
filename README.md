# FalkorSemantic

A high-performance Redis module that extends [FalkorDB](https://www.falkordb.com/) with RDF and SPARQL capabilities, bridging the property graph and semantic web worlds.

[![CI](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml/badge.svg)](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

FalkorSemantic is a Redis module that enables semantic web data processing with FalkorDB. It provides:

- **RDF Data Support**: Store and manage RDF triples in FalkorDB's graph engine
- **SPARQL Queries**: Query your data using the standard semantic web query language
- **Multiple Formats**: Import/export RDF in Turtle, N-Triples, JSON-LD, and N-Quads
- **High Performance**: Leverage FalkorDB's speed for semantic workloads
- **Standards Compliant**: SPARQL 1.1 query support

## Architecture

The project is organized as a Cargo workspace with four main crates:

- **`parser`**: Parses RDF formats (Turtle, N-Triples, JSON-LD) and SPARQL queries
- **`mapper`**: Maps RDF triples to FalkorDB graph structures and translates SPARQL to Cypher
- **`storage`**: Dictionary and namespace storage for efficient IRI handling
- **`module`**: Redis module that exposes RDF/SPARQL commands

```
┌─────────────────────────────────────────────────────┐
│                   Redis Module API                   │
│         (RDF.INSERT, RDF.SPARQL, etc.)              │
├─────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────┐ │
│  │     Parser      │    │        Mapper           │ │
│  │  ├─ RDF         │    │  ├─ RDF → Graph         │ │
│  │  │  (Turtle,    │    │  └─ SPARQL → Cypher     │ │
│  │  │   N-Triples) │    │                         │ │
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

- Rust 1.75 or later
- Docker and Docker Compose (for development environment)
- Redis CLI tools (optional, for testing)

### Installation

#### Using Docker Compose (Recommended)

1. Clone the repository:
```bash
git clone https://github.com/FalkorDB/FalkorSemantic.git
cd FalkorSemantic
```

2. Build the module:
```bash
cargo build --release --package falkorsemantic-module
```

3. Start the development environment:
```bash
docker-compose up -d
```

The services will be available at:
- Redis with FalkorSemantic: `localhost:6379`
- FalkorDB: `localhost:6380`

#### Manual Installation

1. Build the module:
```bash
cargo build --release --package falkorsemantic-module
```

2. Load the module in Redis:
```bash
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so
```

### Basic Usage

```bash
# Connect to Redis
redis-cli

# Insert RDF data in Turtle format
RDF.INSERT mykg turtle '
  @prefix ex: <http://example.org/> .
  @prefix foaf: <http://xmlns.com/foaf/0.1/> .
  
  ex:alice foaf:name "Alice" ;
           foaf:knows ex:bob .
  ex:bob foaf:name "Bob" .
'

# Query with SPARQL
RDF.SPARQL mykg '
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
RDF.INSERT <graph> <format> <data>
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the target graph |
| `format` | Input format: `turtle`, `ntriples`, `jsonld`, `nquads` |
| `data` | RDF data as a string |

**Example:**
```bash
RDF.INSERT mykg ntriples '
<http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
<http://example.org/alice> <http://xmlns.com/foaf/0.1/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
'
```

### RDF.BULK_INSERT

Bulk insert RDF data from a file.

```
RDF.BULK_INSERT <graph> <format> <file_path> [BATCH <size>]
```

**Example:**
```bash
RDF.BULK_INSERT dbpedia ntriples /data/dbpedia.nt BATCH 50000
```

### RDF.SPARQL

Execute a SPARQL query.

```
RDF.SPARQL <graph> <query> [FORMAT <format>] [TIMEOUT <ms>]
```

| Argument | Description |
|----------|-------------|
| `graph` | Name of the graph to query |
| `query` | SPARQL query string |
| `FORMAT` | Output format: `json` (default), `xml`, `csv`, `tsv` |
| `TIMEOUT` | Query timeout in milliseconds |

**Example:**
```bash
RDF.SPARQL mykg '
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
RDF.DELETE <graph> <subject> <predicate> <object>
```

**Example:**
```bash
# Delete all triples about alice
RDF.DELETE mykg "<http://example.org/alice>" "*" "*"
```

### RDF.EXPORT

Export graph data in RDF format.

```
RDF.EXPORT <graph> <format>
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
RDF.GRAPH STATS <graph>
```

## SPARQL Support

### Query Forms

| Form | Status | Notes |
|------|--------|-------|
| SELECT | ✅ | Full support |
| CONSTRUCT | ✅ | Full support |
| ASK | ✅ | Full support |
| DESCRIBE | ✅ | CBD-based |

### Graph Patterns

| Feature | Status |
|---------|--------|
| Basic Graph Patterns | ✅ |
| OPTIONAL | ✅ |
| UNION | ✅ |
| MINUS | ✅ |
| FILTER | ✅ |
| BIND | ✅ |
| VALUES | ✅ |
| Subqueries | ✅ |
| Named Graphs (GRAPH) | ✅ |

### Property Paths

| Path | Syntax | Status |
|------|--------|--------|
| Sequence | a/b | ✅ |
| Alternative | a\|b | ✅ |
| Inverse | ^a | ✅ |
| Zero-or-more | a* | ✅ |
| One-or-more | a+ | ✅ |
| Zero-or-one | a? | ✅ |

### Functions

Supported functions include: `STR`, `LANG`, `DATATYPE`, `IRI`, `BNODE`, `BOUND`, `IF`, `COALESCE`, `EXISTS`, `STRLEN`, `SUBSTR`, `UCASE`, `LCASE`, `CONTAINS`, `REGEX`, `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `GROUP_CONCAT`, and more.

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

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `FALKORSEMANTIC.BATCH_SIZE` | 10000 | Default batch size for bulk operations |
| `FALKORSEMANTIC.QUERY_TIMEOUT` | 30000 | Default query timeout (ms) |
| `FALKORSEMANTIC.CACHE_SIZE` | 10000 | IRI dictionary cache size |

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

# Run integration tests (requires Docker environment)
docker-compose up -d
cargo test --test integration -- --ignored
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
│   │   │   └── ...     # IRIs, literals, triples, namespaces
│   │   └── sparql/     # SPARQL query parser
│   └── Cargo.toml
├── mapper/              # Graph mapping and query translation
│   ├── src/
│   │   ├── rdf/        # RDF to FalkorDB mapping
│   │   └── sparql/     # SPARQL to Cypher translation
│   └── Cargo.toml
├── storage/             # Storage utilities
│   ├── src/
│   │   ├── dictionary/ # IRI dictionary for efficient storage
│   │   └── namespace/  # Namespace prefix management
│   └── Cargo.toml
├── module/              # Redis module implementation
│   ├── src/
│   │   └── commands/   # RDF.* command handlers
│   └── Cargo.toml
├── tests-e2e/           # End-to-end tests
├── scripts/             # Utility scripts
├── .github/workflows/   # CI/CD pipelines
├── docker-compose.yml   # Development environment
└── Cargo.toml          # Workspace configuration
```

## Client Examples

### Python

```python
import redis
import json

r = redis.Redis(host='localhost', port=6379)

# Insert data
r.execute_command('RDF.INSERT', 'mykg', 'turtle', '''
    @prefix ex: <http://example.org/> .
    ex:product1 ex:name "Widget" ; ex:price 29.99 .
''')

# Query
result = r.execute_command('RDF.SPARQL', 'mykg', '''
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

await client.sendCommand(['RDF.INSERT', 'mykg', 'turtle', `
  @prefix schema: <http://schema.org/> .
  <http://example.org/event1> a schema:Event ;
    schema:name "Tech Conference" .
`]);

const result = await client.sendCommand(['RDF.SPARQL', 'mykg', `
  PREFIX schema: <http://schema.org/>
  SELECT ?name WHERE { ?e schema:name ?name }
`]);
console.log(JSON.parse(result));
```

## Roadmap

- [x] Project structure and CI/CD
- [ ] RDF parser implementation (Turtle, N-Triples)
- [ ] RDF to graph mapping
- [ ] SPARQL parser integration
- [ ] SPARQL to Cypher translation
- [ ] Core Redis commands (INSERT, SPARQL, DELETE)
- [x] JSON-LD support
- [ ] Property paths
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

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [redis-module-rs](https://github.com/RedisLabsModules/redismodule-rs)
- Designed to work with [FalkorDB](https://github.com/FalkorDB/FalkorDB)
- SPARQL parsing via [spargebra](https://crates.io/crates/spargebra)
- RDF parsing via [rio](https://crates.io/crates/rio_turtle)

## Support

- **Issues**: [GitHub Issues](https://github.com/FalkorDB/FalkorSemantic/issues)
- **Discussions**: [GitHub Discussions](https://github.com/FalkorDB/FalkorSemantic/discussions)
- **Documentation**: [Wiki](https://github.com/FalkorDB/FalkorSemantic/wiki)

## CI/CD

The project uses GitHub Actions for continuous integration:

- ✅ Build verification on stable, beta, and nightly Rust
- ✅ Automated testing
- ✅ Code formatting checks
- ✅ Linting with Clippy
- ✅ Security audits
- ✅ Artifact building

See the [CI workflow](.github/workflows/ci.yml) for details.
