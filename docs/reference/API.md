# API Reference

Complete API documentation for FalkorSemantic Redis commands.

## Command Summary

| Command | Description |
|---------|-------------|
| [RDF.GRAPH](#rdfgraph) | Manage RDF graphs |
| [RDF.INSERT](#rdfinsert) | Insert RDF triples |
| [RDF.BULK_INSERT](#rdfbulk_insert) | Bulk import RDF data |
| [RDF.NAMESPACES](#rdfnamespaces) | Manage namespace prefixes |
| [RDF.SPARQL](#rdfsparql) | Execute SPARQL queries |
| [RDF.DELETE](#rdfdelete) | Delete triples |
| [RDF.EXPORT](#rdfexport) | Export graph as RDF |

---

## RDF.GRAPH

### RDF.GRAPH CREATE

Create a new RDF graph.

**Syntax:**
```
RDF.GRAPH CREATE <graph_name>
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| graph_name | String | Graph identifier (alphanumeric, `-`, `_`) |

**Return Value:**
- Simple string: `OK`

**Errors:**
- `ERR Graph already exists` - Graph with this name exists
- `ERR Invalid graph name` - Name contains invalid characters

**Example:**
```
RDF.GRAPH CREATE knowledge_base
→ OK
```

**Time Complexity:** O(1)

---

### RDF.GRAPH DROP

Delete an RDF graph.

**Syntax:**
```
RDF.GRAPH DROP <graph_name>
```

**Aliases:** `RDF.GRAPH DELETE`

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| graph_name | String | Graph to delete |

**Return Value:**
- Simple string: `OK`

**Errors:**
- `ERR Graph not found` - Graph doesn't exist

**Example:**
```
RDF.GRAPH DROP old_graph
→ OK
```

**Time Complexity:** O(N) where N is the number of nodes/edges

---

### RDF.GRAPH LIST

List all RDF graphs.

**Syntax:**
```
RDF.GRAPH LIST
```

**Return Value:**
- Array of arrays: `[[graph_name, node_count], ...]`

**Example:**
```
RDF.GRAPH LIST
→ 1) 1) "knowledge_base"
      2) (integer) 1500
   2) 1) "products"
      2) (integer) 250
```

**Time Complexity:** O(G) where G is number of graphs

---

### RDF.GRAPH CLEAR

Remove all data from a graph.

**Syntax:**
```
RDF.GRAPH CLEAR <graph_name>
```

**Aliases:** `RDF.GRAPH EMPTY`

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| graph_name | String | Graph to clear |

**Return Value:**
- Array: `[nodes_deleted, relationships_deleted]`

**Example:**
```
RDF.GRAPH CLEAR temp_graph
→ 1) (integer) 500
   2) (integer) 1200
```

**Time Complexity:** O(N + E) where N is nodes, E is edges

---

### RDF.GRAPH STATS

Get detailed graph statistics.

**Syntax:**
```
RDF.GRAPH STATS <graph_name>
```

**Return Value:**
- Array of key-value pairs with statistics

**Example:**
```
RDF.GRAPH STATS mykg
→ 1) "nodes"
   2) (integer) 1500
   3) "edges"
   4) (integer) 3200
   5) "labels"
   6) 1) "Resource"
      2) "Person"
      3) "Organization"
   7) "relationship_types"
   8) 1) "knows"
      2) "worksFor"
```

---

## RDF.INSERT

Insert RDF data into a graph.

**Syntax:**
```
RDF.INSERT <graph_key> <data> [FORMAT <format>] [ATOMIC]
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| graph_key | String | Yes | Target graph |
| data | String | Yes | RDF data |
| FORMAT | String | No | Input format |
| ATOMIC | Flag | No | Transactional insert |

**Formats:**
| Value | Description | MIME Type |
|-------|-------------|-----------|
| `turtle` | Turtle format | text/turtle |
| `ntriples` | N-Triples | application/n-triples |
| `nquads` | N-Quads | application/n-quads |
| `jsonld` | JSON-LD | application/ld+json |

If FORMAT is omitted, auto-detection is used based on content.

**Return Value:**
- Array: `[triples_parsed, statements_executed, errors]`

**Errors:**
- `ERR Parse error at line N: message` - Syntax error
- `ERR Invalid URI: <uri>` - Malformed URI
- `ERR Graph not found` - Target graph doesn't exist

**Examples:**

Turtle format:
```
RDF.INSERT mykg '@prefix ex: <http://example.org/> . ex:a ex:b ex:c .'
→ 1) (integer) 1
   2) (integer) 1
   3) (integer) 0
```

Explicit format:
```
RDF.INSERT mykg FORMAT ntriples '<http://example.org/a> <http://example.org/b> "value" .'
→ 1) (integer) 1
   2) (integer) 1
   3) (integer) 0
```

Atomic insert:
```
RDF.INSERT mykg ATOMIC '@prefix ex: <http://example.org/> . ex:a ex:b ex:c . ex:d ex:e ex:f .'
→ 1) (integer) 2
   2) (integer) 2
   3) (integer) 0
```

**Time Complexity:** O(T) where T is number of triples

---

## RDF.BULK_INSERT

Bulk import RDF data from a file.

**Syntax:**
```
RDF.BULK_INSERT <graph_key> <format> <file_path> [BATCH <size>]
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| graph_key | String | Yes | Target graph |
| format | String | Yes | File format |
| file_path | String | Yes | Path to file |
| BATCH | Integer | No | Batch size (default: 10000) |

**Return Value:**
- Array: `[total_triples, batches_processed, errors]`

**Errors:**
- `ERR File not found: path` - File doesn't exist
- `ERR Permission denied: path` - Cannot read file
- `ERR Parse error in batch N` - Syntax error

**Example:**
```
RDF.BULK_INSERT mykg ntriples /data/dbpedia.nt BATCH 50000
→ 1) (integer) 5000000
   2) (integer) 100
   3) (integer) 0
```

**Time Complexity:** O(T) where T is number of triples

---

## RDF.NAMESPACES

Manage namespace prefix mappings.

### RDF.NAMESPACES LIST

List all namespace prefixes.

**Syntax:**
```
RDF.NAMESPACES <graph_key> LIST
```

**Return Value:**
- Array of arrays: `[[prefix, uri], ...]`

**Example:**
```
RDF.NAMESPACES mykg LIST
→ 1) 1) "rdf"
      2) "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
   2) 1) "foaf"
      2) "http://xmlns.com/foaf/0.1/"
```

---

### RDF.NAMESPACES ADD

Add a namespace mapping.

**Syntax:**
```
RDF.NAMESPACES <graph_key> ADD <prefix> <uri>
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| prefix | String | Short prefix (letters, digits, `_`) |
| uri | String | Full namespace URI |

**Return Value:**
- Simple string: `OK`

**Errors:**
- `ERR Invalid prefix` - Prefix contains invalid characters
- `ERR Invalid URI` - URI is malformed

**Example:**
```
RDF.NAMESPACES mykg ADD schema "http://schema.org/"
→ OK
```

---

### RDF.NAMESPACES REMOVE

Remove a namespace mapping.

**Syntax:**
```
RDF.NAMESPACES <graph_key> REMOVE <prefix>
```

**Aliases:** `DELETE`, `DEL`

**Return Value:**
- Simple string: `OK`

**Example:**
```
RDF.NAMESPACES mykg REMOVE temp
→ OK
```

---

## RDF.SPARQL

Execute a SPARQL query.

**Syntax:**
```
RDF.SPARQL <graph_key> <query> [FORMAT <format>] [TIMEOUT <ms>]
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| graph_key | String | Yes | Graph to query |
| query | String | Yes | SPARQL query |
| FORMAT | String | No | Output format (default: json) |
| TIMEOUT | Integer | No | Timeout in milliseconds |

**Output Formats:**
| Format | Content-Type | Query Types |
|--------|--------------|-------------|
| `json` | application/sparql-results+json | SELECT, ASK |
| `xml` | application/sparql-results+xml | SELECT, ASK |
| `csv` | text/csv | SELECT |
| `tsv` | text/tab-separated-values | SELECT |
| `turtle` | text/turtle | CONSTRUCT, DESCRIBE |
| `ntriples` | application/n-triples | CONSTRUCT, DESCRIBE |

**Return Value:**
- String containing query results in specified format

**Errors:**
- `ERR Invalid SPARQL query: message` - Syntax error
- `ERR Query timeout` - Exceeded timeout
- `ERR Graph not found` - Graph doesn't exist

**Examples:**

SELECT query:
```
RDF.SPARQL mykg 'PREFIX foaf: <http://xmlns.com/foaf/0.1/> SELECT ?name WHERE { ?p foaf:name ?name }'
→ {"head":{"vars":["name"]},"results":{"bindings":[{"name":{"type":"literal","value":"Alice"}}]}}
```

ASK query:
```
RDF.SPARQL mykg 'ASK { ?s ?p ?o }'
→ {"boolean":true}
```

CONSTRUCT query:
```
RDF.SPARQL mykg 'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }' FORMAT turtle
→ @prefix ex: <http://example.org/> .
  ex:alice ex:knows ex:bob .
```

With timeout:
```
RDF.SPARQL mykg 'SELECT * WHERE { ?s ?p ?o }' TIMEOUT 5000
```

**Time Complexity:** O(varies) depending on query complexity

---

## RDF.DELETE

Delete triples matching a pattern.

**Syntax:**
```
RDF.DELETE <graph_key> <subject> <predicate> <object>
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| graph_key | String | Graph to modify |
| subject | String | Subject URI or `*` wildcard |
| predicate | String | Predicate URI or `*` wildcard |
| object | String | Object (URI/literal) or `*` wildcard |

**Return Value:**
- Integer: number of triples deleted

**Example:**
```
RDF.DELETE mykg '<http://example.org/alice>' '*' '*'
→ (integer) 5
```

**Time Complexity:** O(M) where M is matching triples

---

## RDF.EXPORT

Export graph data in RDF format.

**Syntax:**
```
RDF.EXPORT <graph_key> <format> [GRAPH <named_graph>]
```

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| graph_key | String | Yes | Graph to export |
| format | String | Yes | Output format |
| GRAPH | String | No | Named graph filter |

**Formats:**
| Format | Description |
|--------|-------------|
| `ntriples` | N-Triples |
| `nquads` | N-Quads (with named graphs) |
| `turtle` | Turtle |
| `trig` | TriG (Turtle with named graphs) |
| `jsonld` | JSON-LD |

**Return Value:**
- String containing RDF data

**Example:**
```
RDF.EXPORT mykg turtle
→ @prefix foaf: <http://xmlns.com/foaf/0.1/> .
  @prefix ex: <http://example.org/> .
  
  ex:alice foaf:name "Alice" ;
           foaf:knows ex:bob .
```

**Time Complexity:** O(T) where T is number of triples

---

## Error Codes

| Error | Description |
|-------|-------------|
| `ERR Graph not found` | Specified graph doesn't exist |
| `ERR Graph already exists` | Cannot create duplicate graph |
| `ERR Invalid graph name` | Graph name has invalid characters |
| `ERR Parse error` | RDF syntax error |
| `ERR Invalid URI` | Malformed URI |
| `ERR Invalid SPARQL query` | SPARQL syntax error |
| `ERR Query timeout` | Query exceeded time limit |
| `ERR Invalid prefix` | Namespace prefix invalid |
| `ERR File not found` | Bulk insert file missing |
| `ERR Permission denied` | Cannot read file |
| `ERR Out of memory` | Insufficient memory |

---

## Data Types

### URI Format
```
<http://example.org/resource>
```

### Literal Formats
```
"plain string"
"string with language"@en
"typed value"^^<http://www.w3.org/2001/XMLSchema#integer>
```

### Blank Node Format
```
_:identifier
```

---

## See Also

- [SPARQL Reference](SPARQL.md) - Detailed SPARQL support
- [RDF Mapping](RDF_MAPPING.md) - Data model details
- [Examples](../examples/QUICKSTART.md) - Usage examples
