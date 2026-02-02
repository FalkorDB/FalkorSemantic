# Migration Guide

Migrate your RDF data from other triple stores to FalkorSemantic.

## Table of Contents

- [From Apache Jena/Fuseki](#from-apache-jenafuseki)
- [From Virtuoso](#from-virtuoso)
- [From GraphDB](#from-graphdb)
- [From Amazon Neptune](#from-amazon-neptune)
- [From Blazegraph](#from-blazegraph)
- [From RDF Files](#from-rdf-files)
- [Data Validation](#data-validation)
- [Performance Considerations](#performance-considerations)

## General Migration Strategy

1. **Export** data from source system in N-Triples or N-Quads format
2. **Validate** the exported data
3. **Import** using `RDF.BULK_INSERT` with appropriate batch size
4. **Verify** data integrity with sample queries
5. **Migrate** SPARQL queries (adjust if needed)

## From Apache Jena/Fuseki

### Export Data

```bash
# Using Jena's command-line tools
# Export as N-Triples
tdb2.tdbquery --loc=/path/to/tdb2 \
  "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }" \
  --results=NT > export.nt

# Or use SPARQL endpoint
curl -X POST http://localhost:3030/dataset/query \
  -H "Accept: application/n-triples" \
  -d "query=CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }" \
  > export.nt

# For named graphs, export as N-Quads
curl -X POST http://localhost:3030/dataset/query \
  -H "Accept: application/n-quads" \
  -d "query=CONSTRUCT { GRAPH ?g { ?s ?p ?o } } WHERE { GRAPH ?g { ?s ?p ?o } }" \
  > export.nq
```

### Import to FalkorSemantic

```bash
# Create the graph
redis-cli RDF.GRAPH CREATE myknowledge

# Bulk import
redis-cli RDF.BULK_INSERT myknowledge ntriples /path/to/export.nt BATCH 50000
```

### Query Migration

Most SPARQL queries work unchanged. Key differences:

| Jena Feature | FalkorSemantic |
|--------------|----------------|
| `SERVICE` clause | Not supported (no federation) |
| `LOAD` | Use `RDF.BULK_INSERT` |
| Custom functions | Check function support |
| Full-text search | Use native FalkorDB text search |

## From Virtuoso

### Export Data

```sql
-- Export via SPARQL endpoint
-- Using isql-v command line
SPARQL DEFINE output:format "NT"
CONSTRUCT { ?s ?p ?o }
FROM <http://my-graph>
WHERE { ?s ?p ?o };

-- Or use the conductor interface to export
-- Navigate to: Linked Data -> Quad Store Upload
-- Select "Export" and choose N-Triples format
```

```bash
# Using curl
curl -X POST "http://localhost:8890/sparql" \
  -H "Accept: text/plain" \
  -d "query=CONSTRUCT { ?s ?p ?o } FROM <http://my-graph> WHERE { ?s ?p ?o }" \
  > export.nt
```

### Import to FalkorSemantic

```bash
redis-cli RDF.GRAPH CREATE mygraph
redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/export.nt BATCH 50000
```

### Query Migration

| Virtuoso Feature | FalkorSemantic Equivalent |
|------------------|---------------------------|
| `bif:contains` | Use FILTER with CONTAINS or REGEX |
| `sql:` functions | Not available |
| `DEFINE` pragmas | Not supported |
| Stored procedures | Not supported |

## From GraphDB

### Export Data

```bash
# Using the workbench
# 1. Go to Explore -> Graphs
# 2. Select your repository
# 3. Click Export -> N-Triples

# Or use the REST API
curl -X GET "http://localhost:7200/repositories/myrepo/statements" \
  -H "Accept: application/n-triples" \
  > export.nt

# Export specific graph
curl -X GET "http://localhost:7200/repositories/myrepo/statements?context=<http://my-graph>" \
  -H "Accept: application/n-triples" \
  > export.nt
```

### Import to FalkorSemantic

```bash
redis-cli RDF.GRAPH CREATE myrepo
redis-cli RDF.BULK_INSERT myrepo ntriples /path/to/export.nt BATCH 50000
```

### Query Migration

| GraphDB Feature | FalkorSemantic Equivalent |
|-----------------|---------------------------|
| Full-text search `luc:` | Use FILTER with REGEX |
| GeoSPARQL | Not supported |
| SHACL validation | Not supported |
| Inference (RDFS/OWL) | Not built-in |

## From Amazon Neptune

### Export Data

```bash
# Using Neptune Export
# Configure export job via AWS CLI
aws neptune start-export-task \
  --export-task-identifier my-export \
  --s3-bucket-name my-bucket \
  --s3-bucket-prefix exports/ \
  --output-format NTRIPLES

# Or via SPARQL endpoint
curl -X POST "https://your-neptune-endpoint:8182/sparql" \
  -H "Accept: application/n-triples" \
  -d "query=CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }" \
  > export.nt
```

### Import to FalkorSemantic

```bash
# Download from S3
aws s3 cp s3://my-bucket/exports/export.nt /path/to/export.nt

# Import
redis-cli RDF.GRAPH CREATE neptune_migrated
redis-cli RDF.BULK_INSERT neptune_migrated ntriples /path/to/export.nt BATCH 50000
```

### Query Migration

| Neptune Feature | FalkorSemantic Equivalent |
|-----------------|---------------------------|
| Gremlin queries | Rewrite as SPARQL or Cypher |
| Streams | Use Redis pub/sub |
| IAM authentication | Use Redis ACLs |
| ML predictions | Not supported |

## From Blazegraph

### Export Data

```bash
# Via SPARQL endpoint
curl -X POST "http://localhost:9999/blazegraph/sparql" \
  -H "Accept: application/n-triples" \
  -d "query=CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }" \
  > export.nt

# For named graphs
curl -X POST "http://localhost:9999/blazegraph/sparql" \
  -H "Accept: application/n-quads" \
  -d "query=CONSTRUCT { GRAPH ?g { ?s ?p ?o } } WHERE { GRAPH ?g { ?s ?p ?o } }" \
  > export.nq
```

### Import to FalkorSemantic

```bash
redis-cli RDF.GRAPH CREATE blazegraph_migrated
redis-cli RDF.BULK_INSERT blazegraph_migrated ntriples /path/to/export.nt BATCH 50000
```

### Query Migration

| Blazegraph Feature | FalkorSemantic Equivalent |
|--------------------|---------------------------|
| Full-text search | Use FILTER with REGEX |
| GeoSpatial | Not supported |
| Analytics mode | Use aggregation queries |
| Custom inferencing | Not supported |

## From RDF Files

### N-Triples (.nt)

```bash
redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/data.nt BATCH 50000
```

### N-Quads (.nq)

```bash
redis-cli RDF.BULK_INSERT mygraph nquads /path/to/data.nq BATCH 50000
```

### Turtle (.ttl)

```bash
# Small files - direct insert
redis-cli RDF.INSERT mygraph FORMAT turtle "$(cat /path/to/data.ttl)"

# Large files - convert to N-Triples first
rapper -i turtle -o ntriples data.ttl > data.nt
redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/data.nt BATCH 50000
```

### RDF/XML (.rdf, .owl)

```bash
# Convert to N-Triples using rapper (part of Raptor)
rapper -i rdfxml -o ntriples data.rdf > data.nt
redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/data.nt BATCH 50000
```

### JSON-LD (.jsonld)

```bash
# Small files
redis-cli RDF.INSERT mygraph FORMAT jsonld "$(cat /path/to/data.jsonld)"

# Large files - convert to N-Triples
jsonld normalize data.jsonld | jsonld flatten | jsonld toRdf > data.nt
redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/data.nt BATCH 50000
```

## Data Validation

### Pre-Import Validation

```bash
# Validate N-Triples syntax
rapper -i ntriples -c data.nt

# Count triples
wc -l data.nt

# Check for common issues
grep -c "^\s*$" data.nt  # Empty lines
grep -c "^#" data.nt      # Comments
```

### Post-Import Validation

```bash
# Count triples in graph
redis-cli RDF.SPARQL mygraph 'SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }'

# Check for expected entities
redis-cli RDF.SPARQL mygraph '
  SELECT ?type (COUNT(?s) AS ?count)
  WHERE { ?s a ?type }
  GROUP BY ?type
  ORDER BY DESC(?count)
  LIMIT 20
'

# Verify specific resources exist
redis-cli RDF.SPARQL mygraph '
  ASK { <http://example.org/important-entity> ?p ?o }
'
```

### Sample Query Comparison

Run the same queries on source and FalkorSemantic to verify results match:

```bash
# Source system
curl -X POST "http://source:8890/sparql" \
  -H "Accept: application/json" \
  -d "query=SELECT ?type (COUNT(*) AS ?c) WHERE { ?s a ?type } GROUP BY ?type" \
  > source_results.json

# FalkorSemantic
redis-cli RDF.SPARQL mygraph '
  SELECT ?type (COUNT(*) AS ?c)
  WHERE { ?s a ?type }
  GROUP BY ?type
' FORMAT json > falkorsemantic_results.json

# Compare
diff source_results.json falkorsemantic_results.json
```

## Performance Considerations

### Batch Size Selection

| Data Size | Recommended Batch |
|-----------|-------------------|
| < 100K triples | 10,000 |
| 100K - 1M triples | 50,000 |
| 1M - 10M triples | 100,000 |
| > 10M triples | 100,000 - 500,000 |

### Memory Requirements

Estimate memory before importing:

```
Memory (GB) ≈ triples × 0.0003 + unique_IRIs × 0.0001
```

### Import Performance Tips

1. **Use N-Triples** - Fastest to parse
2. **Disable persistence during import** - Re-enable after
3. **Use dedicated import instance** - Then replicate
4. **Split large files** - Process in parallel

```bash
# Split large file
split -l 10000000 large_export.nt chunk_

# Import chunks
for chunk in chunk_*; do
  redis-cli RDF.BULK_INSERT mygraph ntriples /path/to/$chunk BATCH 100000
done
```

### Post-Import Optimization

```bash
# Create indexes for common query patterns
redis-cli GRAPH.QUERY mygraph "CREATE INDEX ON :Resource(uri)"
redis-cli GRAPH.QUERY mygraph "CREATE INDEX ON :Resource(\`rdf:type\`)"
redis-cli GRAPH.QUERY mygraph "CREATE INDEX ON :Resource(\`rdfs:label\`)"

# Persist to disk
redis-cli BGSAVE
```

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| Out of memory | Reduce batch size, increase Redis memory |
| Slow import | Use N-Triples, increase batch size |
| Parse errors | Validate file with rapper, fix encoding |
| Missing triples | Check for blank lines, validate count |
| Query differences | Check SPARQL feature compatibility |

### Encoding Issues

```bash
# Check file encoding
file -i data.nt

# Convert to UTF-8 if needed
iconv -f ISO-8859-1 -t UTF-8 data.nt > data_utf8.nt
```

### Large Literal Handling

If you have very large literals (>1MB):

```bash
# Find large literals
awk -F'"' 'length($2) > 100000 {print NR": "length($2)" chars"}' data.nt
```

Consider:
- Storing as external files with URI references
- Truncating with warning
- Splitting across properties

## See Also

- [Installation Guide](../INSTALLATION.md) - Set up FalkorSemantic
- [Command Reference](COMMANDS.md) - Import commands
- [Performance Guide](PERFORMANCE.md) - Optimization tips
