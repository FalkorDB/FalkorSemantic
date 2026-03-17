# syntax=docker/dockerfile:1

# Build arguments (declared globally so they're available to all stages)
ARG RUST_VERSION=1.88
ARG FALKORDB_VERSION=v4.16.7

# --- Stage 1: Build the FalkorSemantic module ---
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy full source
COPY Cargo.toml Cargo.lock ./
COPY parser/ parser/
COPY mapper/ mapper/
COPY storage/ storage/
COPY module/ module/
COPY tests-e2e/Cargo.toml tests-e2e/Cargo.toml
COPY tests-compliance/Cargo.toml tests-compliance/Cargo.toml

# Create dummy lib.rs for test crates (not built, but needed for workspace resolution)
RUN mkdir -p tests-e2e/src tests-compliance/src && \
    echo "pub fn _dummy() {}" > tests-e2e/src/lib.rs && \
    echo "pub fn _dummy() {}" > tests-compliance/src/lib.rs

# Build the module
RUN cargo build --release --package falkorsemantic-module && \
    test -f target/release/libfalkorsemantic_module.so

# --- Stage 2: Production runtime ---
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
