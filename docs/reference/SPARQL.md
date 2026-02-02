# SPARQL Feature Support Matrix

FalkorSemantic implements SPARQL 1.1 Query Language. This document details the supported features.

## Query Forms

| Feature | Status | Notes |
|---------|--------|-------|
| SELECT | ✅ Full | Variable projection, DISTINCT, REDUCED |
| CONSTRUCT | ✅ Full | Template-based graph construction |
| ASK | ✅ Full | Boolean existence queries |
| DESCRIBE | ✅ Full | Concise Bounded Description (CBD) |

## Graph Patterns

### Basic Patterns

| Feature | Status | Example |
|---------|--------|---------|
| Triple patterns | ✅ | `?s ?p ?o` |
| Basic Graph Pattern (BGP) | ✅ | Multiple triples in `{}` |
| Empty pattern | ✅ | `{}` |
| Filter placement | ✅ | FILTER within BGP |

### Optional & Union

| Feature | Status | Example |
|---------|--------|---------|
| OPTIONAL | ✅ | `OPTIONAL { ?s ?p ?o }` |
| Nested OPTIONAL | ✅ | Multiple levels |
| UNION | ✅ | `{ ... } UNION { ... }` |
| Multiple UNION | ✅ | Three or more alternatives |

### Negation

| Feature | Status | Example |
|---------|--------|---------|
| MINUS | ✅ | `MINUS { ?s ?p ?o }` |
| NOT EXISTS | ✅ | `FILTER NOT EXISTS { ... }` |
| EXISTS | ✅ | `FILTER EXISTS { ... }` |

### Inline Data

| Feature | Status | Example |
|---------|--------|---------|
| VALUES (inline) | ✅ | `VALUES ?x { 1 2 3 }` |
| VALUES (multi-var) | ✅ | `VALUES (?x ?y) { (1 2) (3 4) }` |
| UNDEF in VALUES | ✅ | `VALUES ?x { 1 UNDEF 3 }` |

### Other Patterns

| Feature | Status | Example |
|---------|--------|---------|
| BIND | ✅ | `BIND (?a + ?b AS ?sum)` |
| SERVICE | ❌ | Federated queries not supported |
| GRAPH (named) | ✅ | `GRAPH ?g { ?s ?p ?o }` |

## Property Paths

| Path Type | Syntax | Status | Example |
|-----------|--------|--------|---------|
| Sequence | `a/b` | ✅ | `foaf:knows/foaf:name` |
| Alternative | `a\|b` | ✅ | `foaf:name\|rdfs:label` |
| Inverse | `^a` | ✅ | `^foaf:knows` |
| Zero-or-one | `a?` | ✅ | `foaf:knows?` |
| Zero-or-more | `a*` | ✅ | `foaf:knows*` |
| One-or-more | `a+` | ✅ | `foaf:knows+` |
| Negated set | `!a` | ✅ | `!(rdf:type\|rdfs:label)` |
| Fixed length | `{n}` | ✅ | `foaf:knows{3}` |
| Range | `{n,m}` | ✅ | `foaf:knows{1,5}` |
| Unbounded range | `{n,}` | ✅ | `foaf:knows{2,}` |

### Property Path Examples

```sparql
# Find all people within 3 hops
SELECT ?person ?connected
WHERE {
  ?person foaf:knows{1,3} ?connected .
}

# Find any label (name, title, or rdfs:label)
SELECT ?thing ?label
WHERE {
  ?thing (foaf:name|dc:title|rdfs:label) ?label .
}

# Inverse relationship - who knows Alice?
SELECT ?knower
WHERE {
  <http://example.org/alice> ^foaf:knows ?knower .
}
```

## Solution Modifiers

| Feature | Status | Notes |
|---------|--------|-------|
| DISTINCT | ✅ | Remove duplicates |
| REDUCED | ✅ | Allow but don't require duplicates |
| ORDER BY | ✅ | ASC, DESC, multiple keys |
| LIMIT | ✅ | Limit result count |
| OFFSET | ✅ | Skip results |
| GROUP BY | ✅ | Aggregate grouping |
| HAVING | ✅ | Group filter |

### Examples

```sparql
# Pagination
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
ORDER BY ?s
LIMIT 100
OFFSET 200

# Aggregation with filtering
SELECT ?class (COUNT(?instance) AS ?count)
WHERE { ?instance a ?class }
GROUP BY ?class
HAVING (COUNT(?instance) > 10)
ORDER BY DESC(?count)
```

## Aggregates

| Function | Status | Notes |
|----------|--------|-------|
| COUNT | ✅ | `COUNT(*)`, `COUNT(?var)`, `COUNT(DISTINCT ?var)` |
| SUM | ✅ | Numeric sum |
| AVG | ✅ | Numeric average |
| MIN | ✅ | Minimum value |
| MAX | ✅ | Maximum value |
| GROUP_CONCAT | ✅ | String concatenation with separator |
| SAMPLE | ✅ | Arbitrary value from group |

### Aggregate Examples

```sparql
# Count by type
SELECT ?type (COUNT(*) AS ?count)
WHERE { ?s a ?type }
GROUP BY ?type

# Concatenate names
SELECT ?group (GROUP_CONCAT(?name; separator=", ") AS ?members)
WHERE { ?person foaf:member ?group ; foaf:name ?name }
GROUP BY ?group
```

## Built-in Functions

### Term Functions

| Function | Status | Description |
|----------|--------|-------------|
| STR | ✅ | Convert to string |
| LANG | ✅ | Get language tag |
| DATATYPE | ✅ | Get datatype IRI |
| IRI / URI | ✅ | Construct IRI |
| BNODE | ✅ | Construct blank node |
| STRDT | ✅ | String with datatype |
| STRLANG | ✅ | String with language |
| UUID | ✅ | Generate UUID IRI |
| STRUUID | ✅ | Generate UUID string |

### Term Tests

| Function | Status | Description |
|----------|--------|-------------|
| isIRI / isURI | ✅ | Test if IRI |
| isBLANK | ✅ | Test if blank node |
| isLITERAL | ✅ | Test if literal |
| isNUMERIC | ✅ | Test if numeric |
| BOUND | ✅ | Test if bound |
| sameTerm | ✅ | RDF term equality |

### String Functions

| Function | Status | Description |
|----------|--------|-------------|
| STRLEN | ✅ | String length |
| SUBSTR | ✅ | Substring |
| UCASE | ✅ | Uppercase |
| LCASE | ✅ | Lowercase |
| STRSTARTS | ✅ | String starts with |
| STRENDS | ✅ | String ends with |
| CONTAINS | ✅ | String contains |
| STRBEFORE | ✅ | String before match |
| STRAFTER | ✅ | String after match |
| ENCODE_FOR_URI | ✅ | URI encode |
| CONCAT | ✅ | String concatenation |
| REPLACE | ✅ | Regex replace |
| REGEX | ✅ | Regex match |
| langMatches | ✅ | Language tag matching |

### Numeric Functions

| Function | Status | Description |
|----------|--------|-------------|
| ABS | ✅ | Absolute value |
| ROUND | ✅ | Round to nearest |
| CEIL | ✅ | Ceiling |
| FLOOR | ✅ | Floor |
| RAND | ✅ | Random number |

### Date/Time Functions

| Function | Status | Description |
|----------|--------|-------------|
| NOW | ✅ | Current datetime |
| YEAR | ✅ | Extract year |
| MONTH | ✅ | Extract month |
| DAY | ✅ | Extract day |
| HOURS | ✅ | Extract hours |
| MINUTES | ✅ | Extract minutes |
| SECONDS | ✅ | Extract seconds |
| TIMEZONE | ✅ | Get timezone |
| TZ | ✅ | Timezone as string |

### Hash Functions

| Function | Status | Description |
|----------|--------|-------------|
| MD5 | ✅ | MD5 hash |
| SHA1 | ✅ | SHA-1 hash |
| SHA256 | ✅ | SHA-256 hash |
| SHA384 | ✅ | SHA-384 hash |
| SHA512 | ✅ | SHA-512 hash |

### Conditional Functions

| Function | Status | Description |
|----------|--------|-------------|
| IF | ✅ | `IF(condition, then, else)` |
| COALESCE | ✅ | First non-error value |
| EXISTS | ✅ | Pattern existence |
| NOT EXISTS | ✅ | Pattern non-existence |
| IN | ✅ | `?x IN (1, 2, 3)` |
| NOT IN | ✅ | `?x NOT IN (1, 2, 3)` |

## Operators

### Comparison Operators

| Operator | Status |
|----------|--------|
| `=` | ✅ |
| `!=` | ✅ |
| `<` | ✅ |
| `>` | ✅ |
| `<=` | ✅ |
| `>=` | ✅ |

### Logical Operators

| Operator | Status |
|----------|--------|
| `&&` (AND) | ✅ |
| `\|\|` (OR) | ✅ |
| `!` (NOT) | ✅ |

### Arithmetic Operators

| Operator | Status |
|----------|--------|
| `+` | ✅ |
| `-` | ✅ |
| `*` | ✅ |
| `/` | ✅ |

## Subqueries

| Feature | Status | Notes |
|---------|--------|-------|
| SELECT subqueries | ✅ | Full support |
| Correlated subqueries | ✅ | Variables from outer scope |
| Nested subqueries | ✅ | Multiple levels |

### Subquery Example

```sparql
SELECT ?person ?name ?maxAge
WHERE {
  ?person foaf:name ?name .
  {
    SELECT (MAX(?age) AS ?maxAge)
    WHERE { ?p foaf:age ?age }
  }
  ?person foaf:age ?maxAge .
}
```

## Named Graphs

| Feature | Status | Notes |
|---------|--------|-------|
| GRAPH clause | ✅ | Query specific graph |
| FROM | ✅ | Default graph |
| FROM NAMED | ✅ | Named graph dataset |
| Graph variable | ✅ | `GRAPH ?g { ... }` |

### Named Graph Examples

```sparql
# Query specific graph
SELECT ?s ?p ?o
FROM <http://example.org/graph1>
WHERE { ?s ?p ?o }

# Query across named graphs
SELECT ?g ?s ?p ?o
WHERE {
  GRAPH ?g { ?s ?p ?o }
}

# Combine default and named graphs
SELECT ?s ?p ?o
FROM <http://example.org/default>
FROM NAMED <http://example.org/named1>
FROM NAMED <http://example.org/named2>
WHERE {
  ?s ?p ?o .
  GRAPH <http://example.org/named1> {
    ?s ?p2 ?o2
  }
}
```

## Features Not Supported

| Feature | Status | Notes |
|---------|--------|-------|
| SERVICE | ❌ | Federated queries |
| LOAD | ❌ | Use RDF.INSERT instead |
| CLEAR | ❌ | Use RDF.GRAPH CLEAR instead |
| DROP | ❌ | Use RDF.GRAPH DROP instead |
| CREATE | ❌ | Use RDF.GRAPH CREATE instead |
| ADD | ❌ | Not implemented |
| MOVE | ❌ | Not implemented |
| COPY | ❌ | Not implemented |
| INSERT DATA | ❌ | Use RDF.INSERT instead |
| DELETE DATA | ❌ | Use RDF.DELETE instead |
| INSERT/DELETE | ❌ | Not implemented |

## SPARQL 1.1 Update

SPARQL Update operations are handled through dedicated commands:

| SPARQL | FalkorSemantic Equivalent |
|--------|---------------------------|
| INSERT DATA | `RDF.INSERT` |
| DELETE DATA | `RDF.DELETE` |
| LOAD | `RDF.BULK_INSERT` |
| CLEAR GRAPH | `RDF.GRAPH CLEAR` |
| DROP GRAPH | `RDF.GRAPH DROP` |
| CREATE GRAPH | `RDF.GRAPH CREATE` |

## Performance Considerations

### Well-Optimized Patterns

- Simple triple patterns with bound subjects
- Filters that reduce early
- Indexed property paths
- Small result sets with aggregation

### Patterns to Avoid

- Cartesian products (unconnected patterns)
- `SELECT *` on large graphs
- Unbounded property paths (`*`) on dense graphs
- Complex regex on large datasets

### Query Hints

```sparql
# Use specific prefixes to help the optimizer
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

# Bind constants early
SELECT ?name
WHERE {
  BIND(<http://example.org/alice> AS ?person)
  ?person foaf:name ?name .
}

# Use LIMIT with ORDER BY
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
ORDER BY ?s
LIMIT 1000
```

## See Also

- [Command Reference](COMMANDS.md) - RDF.SPARQL command details
- [RDF Mapping](RDF_MAPPING.md) - How queries map to Cypher
- [Performance Guide](../guides/PERFORMANCE.md) - Query optimization
