.PHONY: help build test lint fmt clean module install-tools audit \
	docker-build docker-run docker-stop docker-test docker-push

# Docker settings
DOCKER_IMAGE ?= falkordb/falkorsemantic
DOCKER_TAG ?= latest

# Default target
help:
	@echo "FalkorSemantic - Development Commands"
	@echo ""
	@echo "Building:"
	@echo "  make build          - Build all crates"
	@echo "  make module         - Build Redis module in release mode"
	@echo "  make release        - Build optimized release version"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run unit tests"
	@echo "  make test-integration - Run integration tests"
	@echo "  make test-all       - Run all tests"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt            - Format code with rustfmt"
	@echo "  make fmt-check      - Check code formatting"
	@echo "  make lint           - Run clippy linter"
	@echo "  make audit          - Run security audit"
	@echo ""
	@echo "Docker:"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make docker-build   - Build production Docker image"
	@echo "  make docker-run     - Run FalkorSemantic container"
	@echo "  make docker-stop    - Stop FalkorSemantic container"
	@echo "  make docker-test    - Build and smoke-test Docker image"
	@echo "  make docker-push    - Push image to registry"
	@echo ""
	@echo "Utilities:"
	@echo "  make install-tools  - Install development tools"

# Building
build:
	cargo build --workspace

module:
	cargo build --release --package falkorsemantic-module

release:
	cargo build --release --workspace

# Testing
test:
	cargo test --workspace --exclude falkorsemantic-module

test-integration:
	cargo test --test integration -- --ignored

test-all: test test-integration

# Code Quality
fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace -- -D warnings

audit:
	cargo audit

check: fmt-check lint test

# Cleaning
clean:
	cargo clean
	rm -rf target/

# Installation
install-tools:
	cargo install cargo-watch cargo-audit cargo-expand
	rustup component add rustfmt clippy

# Redis commands
redis-cli:
	redis-cli

# Documentation
docs:
	cargo doc --no-deps --workspace

docs-open:
	cargo doc --no-deps --workspace --open

# Docker
docker-build:
	docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) .

docker-run:
	docker run -d --rm --name falkorsemantic -p 6379:6379 $(DOCKER_IMAGE):$(DOCKER_TAG)

docker-stop:
	docker stop falkorsemantic

docker-test: docker-build
	@echo "Starting container..."
	@docker run -d --rm --name falkorsemantic-test -p 16379:6379 $(DOCKER_IMAGE):$(DOCKER_TAG)
	@trap 'docker stop falkorsemantic-test 2>/dev/null || true' EXIT; \
	echo "Waiting for server..."; \
	for i in $$(seq 1 30); do \
		if docker exec falkorsemantic-test redis-cli ping 2>/dev/null | grep -q PONG; then \
			break; \
		fi; \
		sleep 1; \
	done; \
	echo "Checking modules..."; \
	docker exec falkorsemantic-test redis-cli MODULE LIST | grep -qi graph || { echo "FAIL: FalkorDB module not loaded"; exit 1; }; \
	docker exec falkorsemantic-test redis-cli MODULE LIST | grep -qi semantic || { echo "FAIL: FalkorSemantic module not loaded"; exit 1; }; \
	docker exec falkorsemantic-test redis-cli COMMAND INFO rdf.insert | grep -q rdf.insert || { echo "FAIL: rdf.insert command not found"; exit 1; }; \
	echo "Smoke test passed!"

docker-push:
	docker push $(DOCKER_IMAGE):$(DOCKER_TAG)
