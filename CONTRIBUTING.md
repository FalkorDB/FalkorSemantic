# Contributing to FalkorSemantic

Thank you for your interest in contributing to FalkorSemantic! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Project Structure](#project-structure)

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code. Please be respectful and constructive in all interactions.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/FalkorSemantic.git`
3. Add upstream remote: `git remote add upstream https://github.com/FalkorDB/FalkorSemantic.git`
4. Create a new branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Docker and Docker Compose
- Redis CLI tools (for testing)

### Quick Start

1. **Install Rust:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone the repository:**
   ```bash
   git clone https://github.com/FalkorDB/FalkorSemantic.git
   cd FalkorSemantic
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test --workspace --exclude falkorsemantic-module
   ```

5. **Start development environment:**
   ```bash
   docker-compose up -d
   ```

6. **Build and load the Redis module:**
   ```bash
   cargo build --release --package falkorsemantic-module
   docker-compose restart redis
   ```

7. **Run integration tests:**
   ```bash
   cargo test --test integration -- --ignored
   ```

## Coding Standards

### Rust Style Guide

This project follows the official Rust style guide with some project-specific conventions.

#### Code Formatting

- Use `rustfmt` to format your code before committing:
  ```bash
  cargo fmt --all
  ```

- Configuration is in `rustfmt.toml`:
  - Max line width: 100 characters
  - Use spaces, not tabs
  - Follow Rust 2021 edition conventions

#### Linting

- Run `clippy` to catch common mistakes:
  ```bash
  cargo clippy --workspace -- -D warnings
  ```

- Fix all clippy warnings before submitting a PR
- Configuration is in `clippy.toml`

#### Naming Conventions

- **Crates:** Use kebab-case (e.g., `falkorsemantic-parser`)
- **Modules:** Use snake_case (e.g., `semantic_parser`)
- **Types:** Use PascalCase (e.g., `Parser`, `MapperError`)
- **Functions/Variables:** Use snake_case (e.g., `parse_input`, `error_message`)
- **Constants:** Use SCREAMING_SNAKE_CASE (e.g., `MAX_BUFFER_SIZE`)

#### Documentation

- All public items must have documentation comments (`///`)
- Module-level documentation should explain the purpose and usage
- Include examples in documentation where helpful:
  ```rust
  /// Parse semantic data from a string.
  ///
  /// # Examples
  ///
  /// ```
  /// use falkorsemantic_parser::Parser;
  ///
  /// let parser = Parser::new();
  /// parser.parse("input data")?;
  /// ```
  pub fn parse(&self, input: &str) -> Result<()> {
      // implementation
  }
  ```

#### Error Handling

- Use `thiserror` for defining error types
- Use `Result<T>` type aliases for cleaner signatures
- Provide meaningful error messages
- Example:
  ```rust
  use thiserror::Error;

  #[derive(Debug, Error)]
  pub enum ParserError {
      #[error("Parse error: {0}")]
      ParseError(String),
      #[error("Invalid input: {0}")]
      InvalidInput(String),
  }

  pub type Result<T> = std::result::Result<T, ParserError>;
  ```

#### Logging

- Use the `log` crate for logging
- Log levels:
  - `error!`: Errors that prevent normal operation
  - `warn!`: Potential issues that don't prevent operation
  - `info!`: Important operational information
  - `debug!`: Detailed debugging information
  - `trace!`: Very detailed tracing information

### Project Structure

```
FalkorSemantic/
├── parser/              # Semantic data parsing
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── mapper/              # Data to graph structure mapping
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── module/              # Redis module integration
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── tests/               # Integration tests
│   └── integration.rs
├── .github/
│   └── workflows/       # CI/CD pipelines
├── docker-compose.yml   # Development environment
├── Cargo.toml          # Workspace configuration
└── README.md
```

#### Crate Responsibilities

- **parser**: Handles parsing of semantic data formats
- **mapper**: Transforms parsed data into graph structures
- **module**: Redis module implementation that integrates parser and mapper

## Testing

### Unit Tests

- Write unit tests in the same file as the code being tested
- Use the `#[cfg(test)]` module convention
- Test both success and failure cases
- Run with: `cargo test`

### Integration Tests

- Place integration tests in the `tests/` directory
- These require a running Redis and FalkorDB instance
- Run with: `cargo test --test integration -- --ignored`

### Test Coverage

- Aim for high test coverage, especially for critical paths
- Use `cargo tarpaulin` for coverage reports (optional)

## Pull Request Process

1. **Update your fork:**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Make your changes:**
   - Follow the coding standards
   - Add tests for new functionality
   - Update documentation as needed

3. **Verify your changes:**
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace --exclude falkorsemantic-module
   cargo build --release
   ```

4. **Commit your changes:**
   - Use clear, descriptive commit messages
   - Follow conventional commits format:
     - `feat:` New feature
     - `fix:` Bug fix
     - `docs:` Documentation changes
     - `test:` Test changes
     - `refactor:` Code refactoring
     - `chore:` Build/tooling changes

5. **Push and create a PR:**
   ```bash
   git push origin feature/your-feature-name
   ```
   - Open a pull request on GitHub
   - Fill out the PR template
   - Link any related issues

6. **Review process:**
   - Address review comments
   - Keep the PR up to date with main branch
   - CI must pass before merging

## Questions or Issues?

- Open an issue on GitHub for bugs or feature requests
- Start a discussion for questions or ideas
- Check existing issues before creating new ones

## License

By contributing to FalkorSemantic, you agree that your contributions will be licensed under the MIT License.
