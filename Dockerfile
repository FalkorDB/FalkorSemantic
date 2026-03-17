# syntax=docker/dockerfile:1

# --- Stage 1: Build the FalkorSemantic module ---
ARG RUST_VERSION=1.75
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy workspace manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY parser/Cargo.toml parser/Cargo.toml
COPY mapper/Cargo.toml mapper/Cargo.toml
COPY storage/Cargo.toml storage/Cargo.toml
COPY module/Cargo.toml module/Cargo.toml

# Create dummy source files to pre-fetch and compile dependencies
RUN mkdir -p parser/src mapper/src storage/src module/src && \
    echo "pub fn _dummy() {}" > parser/src/lib.rs && \
    echo "pub fn _dummy() {}" > mapper/src/lib.rs && \
    echo "pub fn _dummy() {}" > storage/src/lib.rs && \
    echo "pub fn _dummy() {}" > module/src/lib.rs && \
    cargo build --release --package falkorsemantic-module 2>/dev/null || true && \
    rm -rf parser/src mapper/src storage/src module/src

# Copy real source code
COPY parser/ parser/
COPY mapper/ mapper/
COPY storage/ storage/
COPY module/ module/

# Build the module
RUN cargo build --release --package falkorsemantic-module && \
    test -f target/release/libfalkorsemantic_module.so

# --- Stage 2: Production runtime ---
ARG FALKORDB_VERSION=v4.16.7
FROM falkordb/falkordb:${FALKORDB_VERSION}

LABEL org.opencontainers.image.title="FalkorSemantic" \
      org.opencontainers.image.description="FalkorDB with RDF and SPARQL support" \
      org.opencontainers.image.source="https://github.com/FalkorDB/FalkorSemantic" \
      org.opencontainers.image.licenses="AGPL-3.0"

# Copy the compiled module
COPY --from=builder /src/target/release/libfalkorsemantic_module.so \
     /var/lib/falkordb/bin/falkorsemantic.so

# Copy custom entrypoint that loads both modules
COPY docker/run-semantic.sh /var/lib/falkordb/bin/run-semantic.sh
RUN chmod +x /var/lib/falkordb/bin/run-semantic.sh

EXPOSE 6379

ENTRYPOINT ["/var/lib/falkordb/bin/run-semantic.sh"]
