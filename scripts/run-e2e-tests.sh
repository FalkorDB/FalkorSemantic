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
    STARTED_REDIS=false
else
    echo "=== Starting Redis on port $TEST_PORT ==="
    redis-server \
        --port "$TEST_PORT" \
        --loadmodule "$MODULE_PATH" \
        --daemonize yes \
        --loglevel warning \
        --save "" \
        --appendonly no \
        --pidfile "/tmp/redis-e2e-test-$TEST_PORT.pid"
    
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

# Verify module is loaded
if ! redis-cli -p "$TEST_PORT" MODULE LIST 2>/dev/null | grep -q falkorsemantic; then
    echo "Error: FalkorSemantic module is not loaded"
    if [ "$STARTED_REDIS" = true ]; then
        redis-cli -p "$TEST_PORT" SHUTDOWN NOSAVE 2>/dev/null || true
    fi
    exit 1
fi

echo "=== Running e2e tests ==="
TEST_REDIS_PORT="$TEST_PORT" cargo test -p falkorsemantic-e2e-tests --test e2e -- --ignored --test-threads=1

TEST_EXIT_CODE=$?

# Cleanup
if [ "$STARTED_REDIS" = true ] && [ "$KEEP_RUNNING" = false ]; then
    echo "=== Shutting down Redis ==="
    redis-cli -p "$TEST_PORT" SHUTDOWN NOSAVE 2>/dev/null || true
fi

if [ "$KEEP_RUNNING" = true ]; then
    echo ""
    echo "Redis is still running on port $TEST_PORT"
    echo "To stop: redis-cli -p $TEST_PORT SHUTDOWN NOSAVE"
fi

exit $TEST_EXIT_CODE
