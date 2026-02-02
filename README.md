# FalkorSemantic

A high-performance Redis module for semantic graph operations, built with Rust and integrated with FalkorDB.

[![CI](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml/badge.svg)](https://github.com/FalkorDB/FalkorSemantic/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

FalkorSemantic is a Redis module that enables semantic data processing and graph operations. It provides:

- **Fast Semantic Parsing**: Parse semantic data formats efficiently
- **Graph Mapping**: Transform semantic data into graph structures
- **Redis Integration**: Native Redis module for seamless integration
- **FalkorDB Compatible**: Works seamlessly with FalkorDB graph database

## Architecture

The project is organized as a Cargo workspace with three main crates:

- **`parser`**: Parses semantic data from various formats
- **`mapper`**: Maps parsed data to graph structures
- **`module`**: Redis module that integrates parser and mapper

## Quick Start

### Prerequisites

- Rust 1.70 or later
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

### Usage

Connect to Redis and use the semantic commands:

```bash
# Connect to Redis
redis-cli

# Parse semantic data
SEMANTIC.PARSE "your semantic data here"
```

## Development

### Building

Build all crates:
```bash
cargo build
```

Build specific crate:
```bash
cargo build --package falkorsemantic-parser
```

Build release version:
```bash
cargo build --release
```

### Testing

Run unit tests:
```bash
cargo test --workspace --exclude falkorsemantic-module
```

Run integration tests (requires Docker environment):
```bash
docker-compose up -d
cargo test --test integration -- --ignored
```

### Code Quality

Format code:
```bash
cargo fmt --all
```

Run linter:
```bash
cargo clippy --workspace -- -D warnings
```

Security audit:
```bash
cargo audit
```

## Project Structure

```
FalkorSemantic/
├── parser/              # Semantic data parsing
├── mapper/              # Graph structure mapping
├── module/              # Redis module implementation
├── tests/               # Integration tests
├── .github/workflows/   # CI/CD pipelines
├── docker-compose.yml   # Development environment
└── Cargo.toml          # Workspace configuration
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Development setup
- Coding standards
- Testing requirements
- Pull request process

## Roadmap

- [ ] Complete parser implementation for common semantic formats
- [ ] Implement graph mapping algorithms
- [ ] Add support for multiple semantic data formats
- [ ] Performance optimization
- [ ] Comprehensive documentation
- [ ] Example applications

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [redis-module](https://github.com/RedisLabsModules/redismodule-rs)
- Designed to work with [FalkorDB](https://github.com/FalkorDB/FalkorDB)
- Inspired by the Redis modules ecosystem

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
