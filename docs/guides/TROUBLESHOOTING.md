# Troubleshooting Guide

Common issues and solutions for FalkorSemantic.

## Table of Contents

- [Module Loading Issues](#module-loading-issues)
- [Connection Problems](#connection-problems)
- [Import Errors](#import-errors)
- [Query Issues](#query-issues)
- [Memory Problems](#memory-problems)
- [Performance Issues](#performance-issues)
- [Data Issues](#data-issues)
- [Getting Help](#getting-help)

## Module Loading Issues

### Module fails to load

**Symptoms:**
```
Module /path/to/libfalkorsemantic_module.so failed to load
```

**Solutions:**

1. **Check file exists and permissions:**
```bash
ls -la /path/to/libfalkorsemantic_module.so
# Should show: -rwxr-xr-x
chmod 755 /path/to/libfalkorsemantic_module.so
```

2. **Check library dependencies:**
```bash
ldd /path/to/libfalkorsemantic_module.so
# Look for "not found" entries
```

3. **Check Redis version:**
```bash
redis-server --version
# Requires Redis 7.0+
```

4. **Check architecture match:**
```bash
file /path/to/libfalkorsemantic_module.so
# Should match your system (x86_64, aarch64, etc.)
```

### "Module not found" when running commands

**Symptoms:**
```
ERR unknown command 'RDF.INSERT'
```

**Solutions:**

1. **Verify module is loaded:**
```bash
redis-cli MODULE LIST | grep falkorsemantic
```

2. **Reload the module:**
```bash
redis-cli MODULE LOAD /path/to/libfalkorsemantic_module.so
```

3. **Check redis.conf:**
```conf
loadmodule /path/to/libfalkorsemantic_module.so
```

## Connection Problems

### Cannot connect to Redis

**Symptoms:**
```
Could not connect to Redis at localhost:6379: Connection refused
```

**Solutions:**

1. **Check Redis is running:**
```bash
ps aux | grep redis-server
systemctl status redis
```

2. **Check bind address:**
```bash
redis-cli -h 127.0.0.1 ping
# If using Docker, try the container name or IP
```

3. **Check firewall:**
```bash
sudo iptables -L -n | grep 6379
sudo ufw status
```

### Connection timeout

**Symptoms:**
```
Error: Connection timed out
```

**Solutions:**

1. **Check network connectivity:**
```bash
telnet redis-host 6379
nc -zv redis-host 6379
```

2. **Increase timeout:**
```bash
redis-cli -h host -p 6379 --timeout 30
```

3. **Check `tcp-keepalive` in redis.conf:**
```conf
tcp-keepalive 300
timeout 0
```

## Import Errors

### Parse error in RDF data

**Symptoms:**
```
ERR Parse error at line 42: Unexpected character
```

**Solutions:**

1. **Validate RDF syntax:**
```bash
# Using rapper (from Raptor)
rapper -i turtle -c data.ttl
rapper -i ntriples -c data.nt
```

2. **Check encoding:**
```bash
file -i data.ttl
# Should be: text/plain; charset=utf-8
```

3. **Find the problematic line:**
```bash
sed -n '40,45p' data.ttl
```

4. **Common issues:**
   - Missing period at end of statement
   - Unescaped special characters
   - Invalid URI characters
   - Wrong string quotes

### Invalid URI error

**Symptoms:**
```
ERR Invalid URI: <http://example.org/path with spaces>
```

**Solutions:**

1. **URL-encode problematic characters:**
```
# Wrong
<http://example.org/path with spaces>

# Correct
<http://example.org/path%20with%20spaces>
```

2. **Batch fix URIs:**
```bash
sed -i 's/ /%20/g' data.nt
```

### Out of memory during import

**Symptoms:**
```
ERR OOM command not allowed when used memory > 'maxmemory'
```

**Solutions:**

1. **Increase Redis memory:**
```bash
redis-cli CONFIG SET maxmemory 16gb
```

2. **Use smaller batch sizes:**
```bash
redis-cli RDF.BULK_INSERT graph ntriples data.nt BATCH 10000
```

3. **Split the import:**
```bash
split -l 1000000 large.nt chunk_
for f in chunk_*; do
  redis-cli RDF.BULK_INSERT graph ntriples $f BATCH 50000
done
```

## Query Issues

### Query syntax error

**Symptoms:**
```
ERR Invalid SPARQL query: Expected keyword at position 15
```

**Solutions:**

1. **Validate SPARQL syntax online:**
   - [SPARQL Validator](https://www.sparql.org/query-validator.html)

2. **Common SPARQL syntax issues:**

```sparql
# Missing prefix declaration
SELECT ?name WHERE { ?s foaf:name ?name }
# Fix: Add PREFIX foaf: <http://xmlns.com/foaf/0.1/>

# Wrong variable syntax
SELECT $name WHERE { $s $p $o }
# Fix: Use ? not $ for variables

# Missing WHERE keyword
SELECT ?s ?p ?o { ?s ?p ?o }
# Fix: Add WHERE

# Missing braces
SELECT ?s WHERE ?s ?p ?o
# Fix: Add { }
```

### Query returns no results

**Solutions:**

1. **Check data exists:**
```bash
redis-cli RDF.SPARQL graph 'SELECT * WHERE { ?s ?p ?o } LIMIT 5'
```

2. **Check prefixes match:**
```sparql
# Verify the actual URIs in your data
SELECT DISTINCT ?p WHERE { ?s ?p ?o } LIMIT 20
```

3. **Try without filters:**
```sparql
# Remove FILTER clauses to see if basic pattern matches
SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 100
```

4. **Check case sensitivity:**
```sparql
# URIs are case-sensitive
<http://Example.org/Alice>  # Different from
<http://example.org/alice>
```

### Query timeout

**Symptoms:**
```
ERR Query timeout after 30000ms
```

**Solutions:**

1. **Increase timeout:**
```bash
redis-cli RDF.SPARQL graph 'SELECT ...' TIMEOUT 120000
```

2. **Add LIMIT:**
```sparql
SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 1000
```

3. **Simplify query:**
   - Reduce JOINs
   - Add more specific patterns
   - Avoid `SELECT *`

4. **Check for Cartesian products:**
```sparql
# Bad - unconnected patterns create cross product
SELECT ?a ?b WHERE { ?a ?p1 ?o1 . ?b ?p2 ?o2 }

# Better - connect the patterns
SELECT ?a ?b WHERE { ?a ?p1 ?b . ?b ?p2 ?o2 }
```

### Unexpected query results

**Solutions:**

1. **Check for duplicate data:**
```sparql
SELECT ?s ?p ?o (COUNT(*) AS ?count)
WHERE { ?s ?p ?o }
GROUP BY ?s ?p ?o
HAVING (COUNT(*) > 1)
```

2. **Use DISTINCT:**
```sparql
SELECT DISTINCT ?name WHERE { ?s foaf:name ?name }
```

3. **Verify data types:**
```sparql
# String "30" is different from integer 30
SELECT ?age (DATATYPE(?age) AS ?type)
WHERE { ?s foaf:age ?age }
```

## Memory Problems

### Redis using too much memory

**Diagnosis:**
```bash
redis-cli INFO memory
redis-cli MEMORY STATS
```

**Solutions:**

1. **Check maxmemory setting:**
```bash
redis-cli CONFIG GET maxmemory
redis-cli CONFIG SET maxmemory 16gb
```

2. **Analyze memory usage:**
```bash
redis-cli MEMORY DOCTOR
redis-cli --bigkeys
```

3. **Clear unused graphs:**
```bash
redis-cli RDF.GRAPH LIST
redis-cli RDF.GRAPH DROP unused_graph
```

4. **Enable memory-efficient encoding:**
```conf
# In redis.conf
hash-max-ziplist-entries 512
hash-max-ziplist-value 64
```

### Memory fragmentation

**Symptoms:**
```
# mem_fragmentation_ratio > 1.5
INFO memory shows high fragmentation
```

**Solutions:**

1. **Use jemalloc:**
```bash
# Check allocator
redis-cli INFO | grep mem_allocator
# Should show: jemalloc
```

2. **Restart Redis:** (if fragmentation is very high)

3. **Enable active defragmentation:**
```conf
activedefrag yes
active-defrag-threshold-lower 10
active-defrag-threshold-upper 100
```

## Performance Issues

### Slow insert performance

**Solutions:**

1. **Use bulk insert:**
```bash
# Instead of many RDF.INSERT calls
redis-cli RDF.BULK_INSERT graph ntriples data.nt BATCH 50000
```

2. **Disable persistence during import:**
```bash
redis-cli CONFIG SET save ""
redis-cli CONFIG SET appendonly no
# Re-enable after import
```

3. **Use pipelining for multiple inserts:**
```bash
cat commands.txt | redis-cli --pipe
```

### Slow query performance

**Solutions:**

1. **Create indexes:**
```bash
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(uri)"
redis-cli GRAPH.QUERY graph "CREATE INDEX ON :Resource(\`foaf:name\`)"
```

2. **Profile the query:**
```bash
redis-cli GRAPH.EXPLAIN graph "MATCH (n:Resource) WHERE n.uri = 'http://...' RETURN n"
```

3. **Optimize query patterns:**
```sparql
# Bind specific values early
SELECT ?name
WHERE {
  BIND(<http://example.org/alice> AS ?person)
  ?person foaf:name ?name .
}

# Use specific types
SELECT ?name
WHERE {
  ?person a foaf:Person ;
          foaf:name ?name .
}
```

4. **Limit results:**
```sparql
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
ORDER BY ?s
LIMIT 100
```

## Data Issues

### Missing triples after import

**Solutions:**

1. **Verify count matches:**
```bash
# Count lines in source
wc -l data.nt

# Count triples in graph
redis-cli RDF.SPARQL graph 'SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }'
```

2. **Check for parse errors:**
```bash
# Import returns error count
redis-cli RDF.BULK_INSERT graph ntriples data.nt
# Returns: [total, inserted, errors]
```

3. **Validate source file:**
```bash
rapper -i ntriples -c data.nt 2>&1 | grep -i error
```

### Duplicate triples

**Diagnosis:**
```sparql
SELECT ?s ?p ?o (COUNT(*) AS ?count)
WHERE { ?s ?p ?o }
GROUP BY ?s ?p ?o
HAVING (COUNT(*) > 1)
```

**Solutions:**

1. **Remove duplicates from source:**
```bash
sort -u data.nt > data_unique.nt
```

2. **Use MERGE semantics:** (already default for FalkorSemantic)

### Blank node handling

**Issue:** Blank nodes from different files may collide.

**Solution:** Use unique blank node prefixes per file:
```bash
sed 's/_:b/_:file1_b/g' file1.nt > file1_prefixed.nt
sed 's/_:b/_:file2_b/g' file2.nt > file2_prefixed.nt
```

### Character encoding issues

**Symptoms:**
- Garbled characters in results
- Parse errors with non-ASCII text

**Solutions:**

1. **Ensure UTF-8 encoding:**
```bash
file -i data.nt
iconv -f ISO-8859-1 -t UTF-8 data.nt > data_utf8.nt
```

2. **Check for BOM:**
```bash
hexdump -C data.nt | head -1
# UTF-8 BOM: ef bb bf
sed -i '1s/^\xEF\xBB\xBF//' data.nt
```

## Getting Help

### Diagnostic Information

When reporting issues, include:

```bash
# System info
uname -a
redis-server --version
redis-cli MODULE LIST

# Memory info
redis-cli INFO memory

# Graph info
redis-cli RDF.GRAPH LIST
redis-cli RDF.GRAPH STATS yourGraph
```

### Log Analysis

```bash
# Redis logs (location varies)
tail -f /var/log/redis/redis-server.log
journalctl -u redis -f
```

### Debug Mode

```bash
# Start Redis with verbose logging
redis-server --loglevel debug
```

### Community Support

- **GitHub Issues:** [FalkorSemantic Issues](https://github.com/FalkorDB/FalkorSemantic/issues)
- **Discussions:** [GitHub Discussions](https://github.com/FalkorDB/FalkorSemantic/discussions)
- **FalkorDB Community:** [Discord/Slack]

### Reporting Bugs

Include:
1. FalkorSemantic version
2. Redis version
3. Operating system
4. Minimal reproducible example
5. Expected vs actual behavior
6. Error messages (full text)

## See Also

- [Installation Guide](../INSTALLATION.md) - Setup and configuration
- [Performance Guide](PERFORMANCE.md) - Optimization tips
- [Command Reference](../reference/COMMANDS.md) - Command usage
