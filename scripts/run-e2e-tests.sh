#!/bin/bash
# Run end-to-end tests for FalkorSemantic Redis module
#
# Usage:
#   ./scripts/run-e2e-tests.sh           # Run all e2e tests
#   ./scripts/run-e2e-tests.sh --keep    # Keep Redis running after tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TEST_PORT="${TEST_REDIS_PORT:-6399}"
KEEP_RUNNING=false
STARTED_REDIS=false
CONTAINER_NAME=""

# Ensure container cleanup on any exit (handles set -e failures)
cleanup() {
    if [ "$STARTED_REDIS" = true ] && [ "$KEEP_RUNNING" = false ] && [ -n "$CONTAINER_NAME" ]; then
        echo "=== Shutting down FalkorDB container ==="
        docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

# Parse arguments
for arg in "$@"; do
    case $arg in
        --keep)
            KEEP_RUNNING=true
            shift
            ;;
    esac
done

cd "$PROJECT_DIR"

echo "=== Building FalkorSemantic module ==="
cargo build -p falkorsemantic-module

MODULE_PATH="$PROJECT_DIR/target/debug/libfalkorsemantic_module.so"

if [ ! -f "$MODULE_PATH" ]; then
    echo "Error: Module not found at $MODULE_PATH"
    exit 1
fi

# Check if Redis is already running on test port
if redis-cli -p "$TEST_PORT" PING 2>/dev/null | grep -q PONG; then
    echo "Redis already running on port $TEST_PORT"
else
    echo "=== Starting FalkorDB container on port $TEST_PORT ==="
    CONTAINER_NAME="falkorsemantic-e2e-$TEST_PORT"

    docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
    docker run -d --rm \
        --name "$CONTAINER_NAME" \
        -p "$TEST_PORT:6379" \
        -v "$PROJECT_DIR/target/debug:/target" \
        falkordb/falkordb:v4.16.7 \
        --loadmodule /target/libfalkorsemantic_module.so \
        --loglevel warning \
        --save "" \
        --appendonly no >/dev/null
    
    # Wait for Redis to start
    for i in {1..30}; do
        if redis-cli -p "$TEST_PORT" PING 2>/dev/null | grep -q PONG; then
            break
        fi
        sleep 0.1
    done
    
    if ! redis-cli -p "$TEST_PORT" PING 2>/dev/null | grep -q PONG; then
        echo "Error: Redis failed to start"
        exit 1
    fi
    
    STARTED_REDIS=true
fi

# Verify modules are loaded
if ! redis-cli -p "$TEST_PORT" MODULE LIST 2>/dev/null | grep -q falkorsemantic; then
    echo "Error: FalkorSemantic module is not loaded"
    exit 1
fi

if ! redis-cli -p "$TEST_PORT" MODULE LIST 2>/dev/null | grep -q falkordb; then
    echo "Error: FalkorDB module is not loaded"
    exit 1
fi

echo "=== Running e2e tests ==="
TEST_REDIS_PORT="$TEST_PORT" cargo test -p falkorsemantic-e2e-tests --test e2e -- --ignored --test-threads=1

TEST_EXIT_CODE=$?

# Cleanup is handled by the EXIT trap.
if [ "$KEEP_RUNNING" = true ] && [ "$STARTED_REDIS" = true ]; then
    echo ""
    echo "FalkorDB container is still running on port $TEST_PORT"
    echo "To stop: docker stop $CONTAINER_NAME"
fi

exit $TEST_EXIT_CODE
