# Command Reference

Complete reference for all FalkorSemantic Redis commands.

## Table of Contents

- [RDF.GRAPH](#rdfgraph) - Graph management
- [RDF.INSERT](#rdfinsert) - Insert RDF data
- [RDF.BULK_INSERT](#rdfbulk_insert) - Bulk import
- [RDF.NAMESPACES](#rdfnamespaces) - Namespace management
- [RDF.QUERY](#rdfquery) - SPARQL queries
- [RDF.DELETE](#rdfdelete) - Delete triples

---

## RDF.GRAPH

Manage RDF graphs.

### Syntax

```
RDF.GRAPH <subcommand> [arguments...]
```

### Subcommands

#### CREATE

Create a new RDF graph.

```
RDF.GRAPH CREATE <graph_name>
```

**Arguments:**
- `graph_name` - Name for the new graph (alphanumeric, underscores, hyphens)

**Returns:** `OK` on success

**Example:**
```bash
redis-cli RDF.GRAPH CREATE knowledge_base
# OK
```

#### DROP

Delete an RDF graph and all its data.

```
RDF.GRAPH DROP <graph_name>
```

**Aliases:** `DELETE`

**Arguments:**
- `graph_name` - Name of the graph to delete

**Returns:** `OK` on success

**Example:**
```bash
redis-cli RDF.GRAPH DROP temp_graph
# OK
```

#### LIST

List all RDF graphs with statistics.

```
RDF.GRAPH LIST
```

**Returns:** Array of `[graph_name, node_count]` pairs

**Example:**
```bash
redis-cli RDF.GRAPH LIST
# 1) 1) "knowledge_base"
#    2) (integer) 1500
# 2) 1) "products"
#    2) (integer) 250
```

#### CLEAR

Remove all data from a graph without deleting it.

```
RDF.GRAPH CLEAR <graph_name>
```

**Aliases:** `EMPTY`

**Arguments:**
- `graph_name` - Name of the graph to clear

**Returns:** `[nodes_deleted, relationships_deleted]`

**Example:**
```bash
redis-cli RDF.GRAPH CLEAR knowledge_base
# 1) (integer) 1500
# 2) (integer) 3200
```

---

## RDF.INSERT

Insert RDF data into a graph.

### Syntax

```
RDF.INSERT <graph_key> <data> [FORMAT <format>] [ATOMIC]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `graph_key` | Yes | Target graph name |
| `data` | Yes | RDF data as string |
| `FORMAT` | No | Input format (auto-detected if omitted) |
| `ATOMIC` | No | Execute as single transaction |

### Supported Formats

| Format | Description | Auto-detect Pattern |
|--------|-------------|---------------------|
| `turtle` | Turtle format | `@prefix`, `@base`, or `;` predicates |
| `ntriples` | N-Triples | Lines ending with ` .` |
| `nquads` | N-Quads | Lines with 4 elements ending with ` .` |
| `jsonld` | JSON-LD (not yet implemented) | Starts with `{` or `[` |

### Returns

Array: `[triples_parsed, statements_executed, errors]`

### Examples

#### Basic Turtle Insert

```bash
redis-cli RDF.INSERT mykg '
@prefix ex: <http://example.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

ex:alice foaf:name "Alice" ;
         foaf:age 30 ;
         foaf:knows ex:bob .
ex:bob foaf:name "Bob" .
'
# 1) (integer) 4
# 2) (integer) 4
# 3) (integer) 0
```

#### N-Triples Insert

```bash
redis-cli RDF.INSERT mykg FORMAT ntriples '
<http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
<http://example.org/alice> <http://xmlns.com/foaf/0.1/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
'
```

#### JSON-LD Insert (not yet supported)

> **Note:** JSON-LD format is detected but not yet supported. The following example will return a parse error.

```bash
redis-cli RDF.INSERT mykg FORMAT jsonld '{
  "@context": {
    "name": "http://xmlns.com/foaf/0.1/name",
    "knows": "http://xmlns.com/foaf/0.1/knows"
  },
  "@id": "http://example.org/alice",
  "name": "Alice",
  "knows": {
    "@id": "http://example.org/bob"
  }
}'
```

#### Atomic Insert

```bash
redis-cli RDF.INSERT mykg ATOMIC '
@prefix ex: <http://example.org/> .
ex:a ex:rel ex:b .
ex:b ex:rel ex:c .
ex:c ex:rel ex:d .
'
```

If any triple fails, the entire insert is rolled back.

---

## RDF.BULK_INSERT

Bulk import RDF data from a file or large dataset.

### Syntax

```
RDF.BULK_INSERT <graph_key> <file_path> [FORMAT <format>] [BATCH <size>] [SKIP <lines>] [MAXERRORS <count>] [STOPONERROR]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `graph_key` | Yes | Target graph name |
| `file_path` | Yes | Path to RDF file |
| `FORMAT` | No | Input format (auto-detected from extension if omitted) |
| `BATCH` | No | Batch size (default: 1000) |
| `SKIP` | No | Lines to skip for recovery |
| `MAXERRORS` | No | Max errors before stopping |
| `STOPONERROR` | No | Stop on first error |

### Returns

Array: `[triples_parsed, statements_executed, errors, batches_processed, last_successful_line]`

### Example

```bash
redis-cli RDF.BULK_INSERT dbpedia /data/dbpedia-persons.nt FORMAT ntriples BATCH 50000
# 1) (integer) 5000000
# 2) (integer) 5000000
# 3) (integer) 0
# 4) (integer) 100
# 5) (integer) 5000000
```

---

## RDF.NAMESPACES

Manage namespace prefix mappings for a graph.

### Syntax

```
RDF.NAMESPACES <graph_key> <subcommand> [arguments...]
```

### Subcommands

#### LIST

List all registered namespace prefixes.

```
RDF.NAMESPACES <graph_key> LIST
```

**Returns:** Array of `[prefix, uri]` pairs

**Example:**
```bash
redis-cli RDF.NAMESPACES mykg LIST
# 1) 1) "foaf"
#    2) "http://xmlns.com/foaf/0.1/"
# 2) 1) "ex"
#    2) "http://example.org/"
# 3) 1) "rdf"
#    2) "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
```

#### ADD

Add a namespace prefix mapping.

```
RDF.NAMESPACES <graph_key> ADD <prefix> <uri>
```

**Arguments:**
- `prefix` - Short prefix (e.g., `foaf`, `schema`)
- `uri` - Full namespace URI (must end with `/` or `#`)

**Returns:** `OK` on success

**Example:**
```bash
redis-cli RDF.NAMESPACES mykg ADD schema "http://schema.org/"
# OK
```

#### REMOVE

Remove a namespace prefix mapping.

```
RDF.NAMESPACES <graph_key> REMOVE <prefix>
```

**Aliases:** `DELETE`, `DEL`

**Returns:** `OK` on success

**Example:**
```bash
redis-cli RDF.NAMESPACES mykg REMOVE temp
# OK
```

### Common Prefixes

These prefixes are automatically available:

| Prefix | URI |
|--------|-----|
| `rdf` | `http://www.w3.org/1999/02/22-rdf-syntax-ns#` |
| `rdfs` | `http://www.w3.org/2000/01/rdf-schema#` |
| `xsd` | `http://www.w3.org/2001/XMLSchema#` |
| `owl` | `http://www.w3.org/2002/07/owl#` |

---

## RDF.QUERY

Execute SPARQL queries.

### Syntax

```bash
RDF.QUERY <graph_key> <query> [FORMAT <format>] [TIMEOUT <ms>]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `graph_key` | Yes | Graph to query |
| `query` | Yes | SPARQL query string |
| `FORMAT` | No | Output format (default: `json`) |
| `TIMEOUT` | No | Query timeout in milliseconds |

### Output Formats

| Format | Content-Type | Query Types |
|--------|--------------|-------------|
| `json` | application/sparql-results+json | SELECT, ASK |
| `xml` | application/sparql-results+xml | SELECT, ASK |
| `csv` | text/csv | SELECT |
| `tsv` | text/tab-separated-values | SELECT |

### Returns

Query results in the specified format.

### Examples

#### SELECT Query

```bash
redis-cli RDF.QUERY mykg '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name ?age
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:age ?age }
}
ORDER BY ?name
LIMIT 10
'
```

**JSON Result:**
```json
{
  "head": {"vars": ["name", "age"]},
  "results": {
    "bindings": [
      {"name": {"type": "literal", "value": "Alice"}, "age": {"type": "literal", "value": "30", "datatype": "http://www.w3.org/2001/XMLSchema#integer"}},
      {"name": {"type": "literal", "value": "Bob"}}
    ]
  }
}
```

#### ASK Query

```bash
redis-cli RDF.QUERY mykg '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
ASK { ?person foaf:name "Alice" }
'
```

**Result:**
```json
{"boolean": true}
```

#### Query with Timeout

```bash
redis-cli RDF.QUERY mykg '
SELECT ?s ?p ?o WHERE { ?s ?p ?o }
' TIMEOUT 5000
```

---

## RDF.DELETE

Delete triples matching a pattern.

### Syntax

```
RDF.DELETE <graph_key> <subject> <predicate> <object> [GRAPH <named_graph>] [ORPHANS]
```

### Arguments

Use `*` as a wildcard for any component.

| Argument | Required | Description |
|----------|----------|-------------|
| `graph_key` | Yes | Graph to delete from |
| `subject` | Yes | Subject URI or `*` |
| `predicate` | Yes | Predicate URI or `*` |
| `object` | Yes | Object (URI/literal) or `*` |
| `GRAPH` | No | Delete from a specific named graph |
| `ORPHANS` | No | Also delete orphaned nodes |

### Returns

Number of triples deleted.

### Examples

#### Delete Specific Triple

```bash
redis-cli RDF.DELETE mykg '<http://example.org/alice>' '<http://xmlns.com/foaf/0.1/age>' '"30"'
# (integer) 1
```

#### Delete All Triples About a Subject

```bash
redis-cli RDF.DELETE mykg '<http://example.org/alice>' '*' '*'
# (integer) 5
```

#### Delete All Triples with a Predicate

```bash
redis-cli RDF.DELETE mykg '*' '<http://example.org/deprecated>' '*'
# (integer) 12
```

---

## Error Handling

All commands return Redis errors for invalid operations:

| Error | Description |
|-------|-------------|
| `ERR Graph not found` | The specified graph doesn't exist |
| `ERR Invalid RDF format` | Cannot parse the input data |
| `ERR Parse error at line N` | Syntax error in RDF data |
| `ERR Invalid SPARQL query` | SPARQL syntax error |
| `ERR Query timeout` | Query exceeded timeout limit |
| `ERR Invalid namespace prefix` | Prefix contains invalid characters |
| `ERR Invalid URI` | URI is malformed |

---

## See Also

- [SPARQL Feature Matrix](SPARQL.md) - Detailed SPARQL support
- [RDF Mapping](RDF_MAPPING.md) - How RDF maps to property graphs
- [Examples](../examples/QUICKSTART.md) - Practical examples
