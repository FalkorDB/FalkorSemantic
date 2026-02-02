# Installation Guide

This guide covers all methods for installing and deploying FalkorSemantic.

## Table of Contents

- [System Requirements](#system-requirements)
- [Quick Install (Docker)](#quick-install-docker)
- [Build from Source](#build-from-source)
- [Redis Configuration](#redis-configuration)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Verifying Installation](#verifying-installation)

## System Requirements

### Minimum Requirements

| Component | Requirement |
|-----------|-------------|
| CPU | 2 cores |
| RAM | 4 GB |
| Disk | 10 GB SSD |
| OS | Linux (Ubuntu 20.04+, Debian 11+, RHEL 8+) |

### Recommended for Production

| Component | Requirement |
|-----------|-------------|
| CPU | 8+ cores |
| RAM | 32 GB+ |
| Disk | NVMe SSD |
| OS | Linux with kernel 5.4+ |

### Software Dependencies

- **Rust**: 1.75 or later (for building from source)
- **Redis**: 7.0 or later
- **FalkorDB**: 4.0 or later (bundled or separate)
- **Docker**: 20.10+ (for containerized deployment)

## Quick Install (Docker)

The fastest way to get started is using Docker Compose:

```bash
# Clone the repository
git clone https://github.com/FalkorDB/FalkorSemantic.git
cd FalkorSemantic

# Start services
docker-compose up -d

# Verify
redis-cli -p 6379 PING
# Expected: PONG
```

### Docker Compose Services

The default `docker-compose.yml` starts:

| Service | Port | Description |
|---------|------|-------------|
| `falkorsemantic` | 6379 | Redis with FalkorSemantic module |
| `falkordb` | 6380 | FalkorDB for graph storage |

### Custom Docker Image

Build a custom image with your configuration:

```dockerfile
FROM redis:7.2

# Copy the module
COPY target/release/libfalkorsemantic_module.so /usr/lib/redis/modules/

# Load module on startup
CMD ["redis-server", "--loadmodule", "/usr/lib/redis/modules/libfalkorsemantic_module.so"]
```

Build and run:

```bash
docker build -t falkorsemantic:custom .
docker run -p 6379:6379 falkorsemantic:custom
```

## Build from Source

### Prerequisites

Install Rust and development tools:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.75+

# Install build dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# Install build dependencies (RHEL/CentOS)
sudo dnf install -y gcc make openssl-devel
```

### Building the Module

```bash
# Clone repository
git clone https://github.com/FalkorDB/FalkorSemantic.git
cd FalkorSemantic

# Build release version
cargo build --release --package falkorsemantic-module

# The module is located at:
ls -la target/release/libfalkorsemantic_module.so
```

### Build Options

```bash
# Debug build (faster compilation, larger binary)
cargo build --package falkorsemantic-module

# Release build with optimizations
cargo build --release --package falkorsemantic-module

# Build with specific features
cargo build --release --package falkorsemantic-module --features "json-ld"

# Cross-compile for different architecture
cargo build --release --target x86_64-unknown-linux-musl
```

## Redis Configuration

### Loading the Module

#### Method 1: Command Line

```bash
redis-server --loadmodule /path/to/libfalkorsemantic_module.so
```

#### Method 2: Redis Configuration File

Add to `redis.conf`:

```conf
# FalkorSemantic Module
loadmodule /path/to/libfalkorsemantic_module.so

# Optional: Module-specific settings
# falkorsemantic.batch_size 10000
# falkorsemantic.query_timeout 30000
```

Start Redis:

```bash
redis-server /path/to/redis.conf
```

#### Method 3: Runtime Loading

```bash
redis-cli MODULE LOAD /path/to/libfalkorsemantic_module.so
```

### Recommended Redis Settings

For optimal performance, add these to `redis.conf`:

```conf
# Memory management
maxmemory 16gb
maxmemory-policy noeviction

# Persistence (for production)
appendonly yes
appendfsync everysec

# Network
tcp-keepalive 300
timeout 0

# Limits
maxclients 10000

# Threading (Redis 7+)
io-threads 4
io-threads-do-reads yes
```

### Memory Considerations

FalkorSemantic stores RDF data in FalkorDB graphs. Memory usage depends on:

| Factor | Impact |
|--------|--------|
| Number of triples | ~200-500 bytes per triple |
| URI length | Longer URIs use more memory |
| Literal size | Large literals increase memory |
| Namespace compression | Reduces memory by 30-50% |

**Estimation formula:**
```
Memory (GB) ≈ (triples × 0.0003) + (unique_iris × 0.0001)
```

## Kubernetes Deployment

### Helm Chart (Coming Soon)

```bash
helm repo add falkorsemantic https://charts.falkorsemantic.io
helm install my-release falkorsemantic/falkorsemantic
```

### Manual Deployment

Create a ConfigMap for Redis configuration:

```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: falkorsemantic-config
data:
  redis.conf: |
    loadmodule /usr/lib/redis/modules/libfalkorsemantic_module.so
    maxmemory 8gb
    maxmemory-policy noeviction
```

Create the Deployment:

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: falkorsemantic
spec:
  replicas: 1
  selector:
    matchLabels:
      app: falkorsemantic
  template:
    metadata:
      labels:
        app: falkorsemantic
    spec:
      containers:
      - name: falkorsemantic
        image: falkorsemantic:latest
        ports:
        - containerPort: 6379
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "16Gi"
            cpu: "8"
        volumeMounts:
        - name: config
          mountPath: /etc/redis
        - name: data
          mountPath: /data
      volumes:
      - name: config
        configMap:
          name: falkorsemantic-config
      - name: data
        persistentVolumeClaim:
          claimName: falkorsemantic-data
```

Create a Service:

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: falkorsemantic
spec:
  selector:
    app: falkorsemantic
  ports:
  - port: 6379
    targetPort: 6379
  type: ClusterIP
```

Apply the manifests:

```bash
kubectl apply -f configmap.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
```

## Verifying Installation

### Check Module Loading

```bash
redis-cli MODULE LIST
```

Expected output:
```
1) 1) "name"
   2) "falkorsemantic"
   3) "ver"
   4) (integer) 1
```

### Test Basic Commands

```bash
# Create a graph
redis-cli RDF.GRAPH CREATE testgraph

# Insert a triple
redis-cli RDF.INSERT testgraph '<http://example.org/s> <http://example.org/p> "test" .'

# List graphs
redis-cli RDF.GRAPH LIST

# Clean up
redis-cli RDF.GRAPH DROP testgraph
```

### Health Check Script

Save as `healthcheck.sh`:

```bash
#!/bin/bash

HOST=${1:-localhost}
PORT=${2:-6379}

# Check Redis connection
if ! redis-cli -h $HOST -p $PORT PING > /dev/null 2>&1; then
    echo "ERROR: Cannot connect to Redis"
    exit 1
fi

# Check module loaded
if ! redis-cli -h $HOST -p $PORT MODULE LIST | grep -q "falkorsemantic"; then
    echo "ERROR: FalkorSemantic module not loaded"
    exit 1
fi

# Test basic operation
TEST_GRAPH="__healthcheck_$(date +%s)"
redis-cli -h $HOST -p $PORT RDF.GRAPH CREATE $TEST_GRAPH > /dev/null 2>&1
redis-cli -h $HOST -p $PORT RDF.GRAPH DROP $TEST_GRAPH > /dev/null 2>&1

echo "OK: FalkorSemantic is healthy"
exit 0
```

Run:
```bash
chmod +x healthcheck.sh
./healthcheck.sh localhost 6379
```

## Next Steps

- [Command Reference](reference/COMMANDS.md) - Learn all available commands
- [Quick Start Examples](examples/QUICKSTART.md) - Try example queries
- [Performance Tuning](guides/PERFORMANCE.md) - Optimize for your workload
