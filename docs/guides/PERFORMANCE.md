# Performance Tuning Guide

Optimize FalkorSemantic for your workload.

## Table of Contents

- [Understanding Performance](#understanding-performance)
- [Hardware Recommendations](#hardware-recommendations)
- [Redis Configuration](#redis-configuration)
- [Import Optimization](#import-optimization)
- [Query Optimization](#query-optimization)
- [Memory Optimization](#memory-optimization)
- [Monitoring](#monitoring)
- [Benchmarking](#benchmarking)

## Understanding Performance

### Key Performance Factors

| Factor | Impact | Tunable |
|--------|--------|---------|
| Memory | High | RAM, maxmemory |
| CPU | Medium | Threads, query complexity |
| Disk I/O | Low (in-memory) | Persistence settings |
| Network | Low | Connection pooling |
| Data model | High | Schema design, indexes |

### Performance Characteristics

| Operation | Typical Latency | Throughput |
|-----------|-----------------|------------|
| Simple triple lookup | < 1ms | 100K+ ops/sec |
| Property lookup | < 1ms | 100K+ ops/sec |
| Simple SPARQL SELECT | 1-10ms | 10K+ queries/sec |
| Complex JOIN query | 10-100ms | 1K+ queries/sec |
| Bulk insert (per triple) | 0.01ms | 100K+ triples/sec |

## Hardware Recommendations

### Development

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4 cores |
| RAM | 4 GB | 8 GB |
| Disk | HDD | SSD |

### Production (< 10M triples)

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 4 cores | 8 cores |
| RAM | 16 GB | 32 GB |
| Disk | SSD | NVMe SSD |
| Network | 1 Gbps | 10 Gbps |

### Production (10M-100M triples)

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 8 cores | 16+ cores |
| RAM | 64 GB | 128+ GB |
| Disk | NVMe SSD | NVMe RAID |
| Network | 10 Gbps | 25 Gbps |

### Memory Sizing

Estimate memory requirements:

```
Memory = (triples × 300 bytes) + (unique_IRIs × 100 bytes) + overhead
```

| Triples | Estimated Memory |
|---------|------------------|
| 1 million | 1-2 GB |
| 10 million | 8-12 GB |
| 100 million | 80-120 GB |
| 1 billion | 800 GB+ |

## Redis Configuration

### Essential Settings

```conf
# redis.conf for FalkorSemantic

# Memory
maxmemory 32gb
maxmemory-policy noeviction

# Connections
maxclients 10000
tcp-keepalive 300
timeout 0

# Threading (Redis 7+)
io-threads 4
io-threads-do-reads yes

# Persistence (adjust based on needs)
save 900 1
save 300 10
save 60 10000

appendonly yes
appendfsync everysec

# Memory efficiency
activedefrag yes
active-defrag-threshold-lower 10
active-defrag-threshold-upper 100
```

### Persistence Trade-offs

| Setting | Durability | Performance |
|---------|------------|-------------|
| No persistence | None | Best |
| RDB only | Periodic | Good |
| AOF everysec | ~1 second | Good |
| AOF always | Complete | Slower |
| RDB + AOF | Best | Moderate |

For maximum import speed, temporarily disable persistence:

```bash
redis-cli CONFIG SET save ""
redis-cli CONFIG SET appendonly no

# After import
redis-cli CONFIG SET save "900 1 300 10 60 10000"
redis-cli CONFIG SET appendonly yes
redis-cli BGREWRITEAOF
```

## Import Optimization

### Batch Size Selection

| Data Size | Optimal Batch | Notes |
|-----------|---------------|-------|
| < 100K | 10,000 | Small overhead |
| 100K - 1M | 50,000 | Balance of speed/memory |
| 1M - 10M | 100,000 | Higher throughput |
| > 10M | 100,000 - 500,000 | Test for optimal |

```bash
# Test different batch sizes
for batch in 10000 50000 100000 200000; do
  echo "Testing batch size: $batch"
  time redis-cli RDF.BULK_INSERT testgraph sample.nt FORMAT ntriples BATCH $batch
  redis-cli RDF.GRAPH DROP testgraph
done
```

### Format Selection

| Format | Parse Speed | Compression |
|--------|-------------|-------------|
| N-Triples | Fastest | None |
| N-Quads | Fast | None |
| Turtle | Slower | ~30% smaller |
| JSON-LD | Slowest | Variable |

**Recommendation:** Convert to N-Triples for bulk imports:

```bash
rapper -i turtle -o ntriples data.ttl > data.nt
```

### Parallel Import

For very large datasets, split and import in parallel:

```bash
# Split file
split -l 10000000 huge_dataset.nt chunk_

# Import chunks (sequentially - Redis is single-threaded for writes)
for chunk in chunk_*; do
  redis-cli RDF.BULK_INSERT graph $chunk FORMAT ntriples BATCH 100000
done
```

### Pre-Import Preparation

1. **Disable unnecessary logging:**
```bash
redis-cli CONFIG SET loglevel warning
```

2. **Reserve memory:**
```bash
redis-cli CONFIG SET maxmemory-reserved 2gb
```

3. **Disable client output buffer limits:**
```bash
redis-cli CONFIG SET client-output-buffer-limit "normal 0 0 0"
```

## Query Optimization

### Index Creation

Create indexes for frequently queried properties:

```bash
# Essential indexes
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(uri)"
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :BNode(id)"

# Property indexes for common queries
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(\`foaf:name\`)"
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(\`rdfs:label\`)"
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(\`rdf:type\`)"
```

### Query Patterns

#### Bind Constants Early

```sparql
# Slow - scans all resources first
SELECT ?name
WHERE {
  ?person foaf:name ?name .
  FILTER (?person = <http://example.org/alice>)
}

# Fast - uses index immediately
SELECT ?name
WHERE {
  <http://example.org/alice> foaf:name ?name .
}

# Or use BIND
SELECT ?name
WHERE {
  BIND(<http://example.org/alice> AS ?person)
  ?person foaf:name ?name .
}
```

#### Use Specific Types

```sparql
# Slow - scans all resources
SELECT ?name
WHERE {
  ?person foaf:name ?name .
}

# Faster - filters by type first
SELECT ?name
WHERE {
  ?person a foaf:Person ;
          foaf:name ?name .
}
```

#### Limit Property Paths

```sparql
# Potentially slow - unbounded
SELECT ?connected
WHERE {
  ?start foaf:knows* ?connected .
}

# Better - bounded depth
SELECT ?connected
WHERE {
  ?start foaf:knows{1,5} ?connected .
}
```

#### Avoid SELECT *

```sparql
# Slow - returns all variables
SELECT *
WHERE { ?s ?p ?o }

# Better - specific projection
SELECT ?s ?name
WHERE { ?s foaf:name ?name }
```

#### Use LIMIT with ORDER BY

```sparql
# Always paginate large results
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
ORDER BY ?s
LIMIT 1000
OFFSET 0
```

### Query Analysis

Use EXPLAIN to understand query plans:

```bash
redis-cli GRAPH.EXPLAIN graph "MATCH (n:Resource) WHERE n.uri = 'http://...' RETURN n"
```

Look for:
- Index usage
- Scan operations (try to minimize)
- Filter placement

## Memory Optimization

### Data Model Efficiency

1. **Use short URIs when possible:**
```turtle
# Instead of very long URIs
<http://example.org/ontology/v2/entities/persons/employees/fulltime/Alice>

# Use namespace prefixes
ex:Alice
```

2. **Consistent predicate usage:**
```turtle
# Good - single predicate
ex:alice foaf:name "Alice" .
ex:bob foaf:name "Bob" .

# Avoid - multiple predicates for same concept
ex:alice foaf:name "Alice" .
ex:bob rdfs:label "Bob" .
```

### Memory Monitoring

```bash
# Overall memory
redis-cli INFO memory

# Memory per key type
redis-cli MEMORY STATS

# Big keys
redis-cli --bigkeys

# Specific key memory
redis-cli MEMORY USAGE graph:key
```

### Memory Reduction Strategies

1. **Remove unused graphs:**
```bash
redis-cli RDF.GRAPH LIST
redis-cli RDF.GRAPH DROP unused_graph
```

2. **Clear old data:**
```sparql
# Delete old triples
RDF.DELETE graph '*' '<http://example.org/deprecated>' '*'
```

3. **Optimize namespace storage:**
```bash
# Use common prefixes
redis-cli RDF.NAMESPACES graph ADD rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
redis-cli RDF.NAMESPACES graph ADD rdfs "http://www.w3.org/2000/01/rdf-schema#"
```

## Monitoring

### Key Metrics

```bash
# Commands processed
redis-cli INFO stats | grep total_commands

# Memory usage
redis-cli INFO memory | grep used_memory_human

# Client connections
redis-cli INFO clients | grep connected_clients

# Query latency (use INFO commandstats)
redis-cli INFO commandstats | grep rdf
```

### Alerting Thresholds

| Metric | Warning | Critical |
|--------|---------|----------|
| Memory usage | > 80% maxmemory | > 95% maxmemory |
| Fragmentation ratio | > 1.5 | > 2.0 |
| Connected clients | > 80% maxclients | > 95% maxclients |
| Query latency (p99) | > 100ms | > 1s |

### Prometheus Metrics

Use Redis Exporter for Prometheus:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
```

## Benchmarking

### Built-in Benchmarks

```bash
# Run performance benchmarks
cargo test --test benchmarks -- --ignored --nocapture

# Specific benchmark
cargo test --test benchmarks bench_insert_1m_triples -- --ignored --nocapture
```

### Custom Benchmarking

```bash
# Simple insert benchmark
time redis-cli RDF.INSERT testgraph "$(cat sample_1000.ttl)"

# Query benchmark
redis-benchmark -n 10000 -c 50 \
  RDF.QUERY testgraph "SELECT ?s WHERE { ?s ?p ?o } LIMIT 10"
```

### Baseline Performance

Expected performance on modern hardware:

| Operation | Expected Rate |
|-----------|---------------|
| Single triple insert | 10K-50K/sec |
| Bulk insert | 100K-500K triples/sec |
| Simple lookup | 100K+ queries/sec |
| Simple SPARQL | 10K-50K queries/sec |
| Complex SPARQL | 100-1K queries/sec |

### Performance Testing Checklist

- [ ] Test with production-like data volume
- [ ] Test with realistic query patterns
- [ ] Test concurrent load
- [ ] Measure p50, p95, p99 latencies
- [ ] Monitor memory during tests
- [ ] Test after extended runtime (memory leaks)
- [ ] Compare with and without indexes

## See Also

- [Installation Guide](../INSTALLATION.md) - Hardware and configuration
- [Troubleshooting](TROUBLESHOOTING.md) - Performance problems
- [SPARQL Reference](../reference/SPARQL.md) - Query optimization
