# SPARQL Feature Support Matrix

FalkorSemantic implements a subset of SPARQL 1.1 Query Language. This document details the current feature support status.

## Query Forms

| Feature | Status | Notes |
|---------|--------|-------|
| SELECT | ✅ Full | Variable projection, DISTINCT, REDUCED |
| CONSTRUCT | ❌ | Not yet supported |
| ASK | ✅ Full | Boolean existence queries |
| DESCRIBE | ❌ | Not yet supported |

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
| UNDEF in VALUES | 🚧 | `VALUES ?x { 1 UNDEF 3 }` — not verified |

### Other Patterns

| Feature | Status | Example |
|---------|--------|---------|
| BIND | ✅ | `BIND (?a + ?b AS ?sum)` |
| SERVICE | ❌ | Federated queries not supported |
| GRAPH (named) | 🚧 | Pattern traversed but no graph scoping |

## Property Paths

> **Note:** All property paths are parsed but simplified to a generic traversal pattern. Full path semantics planned.

| Path Type | Syntax | Status | Notes |
|-----------|--------|--------|-------|
| Sequence | `a/b` | 🚧 | Parsed but simplified |
| Alternative | `a\|b` | 🚧 | Parsed but simplified |
| Inverse | `^a` | 🚧 | Parsed but simplified |
| Zero-or-one | `a?` | 🚧 | Parsed but simplified |
| Zero-or-more | `a*` | 🚧 | Parsed but simplified |
| One-or-more | `a+` | 🚧 | Parsed but simplified |
| Negated set | `!a` | 🚧 | Parsed but simplified |
| Fixed length | `{n}` | 🚧 | Parsed but simplified |
| Range | `{n,m}` | 🚧 | Parsed but simplified |
| Unbounded range | `{n,}` | 🚧 | Parsed but simplified |

## Solution Modifiers

| Feature | Status | Notes |
|---------|--------|-------|
| DISTINCT | ✅ | Remove duplicates |
| REDUCED | ✅ | Allow but don't require duplicates |
| ORDER BY | ✅ | ASC, DESC, multiple keys |
| LIMIT | ✅ | Limit result count |
| OFFSET | ✅ | Skip results |
| GROUP BY | 🚧 | Pattern traversed but no aggregate projection |
| HAVING | 🚧 | Not translated |

### Examples

```sparql
# Pagination
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
ORDER BY ?s
LIMIT 100
OFFSET 200
```

## Aggregates

> **Note:** Aggregate functions are parsed but not yet translated to Cypher.

| Function | Status | Notes |
|----------|--------|-------|
| COUNT | 🚧 | Not translated to Cypher |
| SUM | 🚧 | Not translated to Cypher |
| AVG | 🚧 | Not translated to Cypher |
| MIN | 🚧 | Not translated to Cypher |
| MAX | 🚧 | Not translated to Cypher |
| GROUP_CONCAT | 🚧 | Not translated to Cypher |
| SAMPLE | 🚧 | Not translated to Cypher |

## Built-in Functions

### Term Functions

| Function | Status | Description |
|----------|--------|-------------|
| STR | ✅ | Convert to string (mapped to `toString()`) |
| LANG | ✅ | Get language tag (mapped to `.language` property) |
| DATATYPE | ✅ | Get datatype IRI (mapped to `.datatype` property) |
| IRI / URI | ❌ | Not implemented |
| BNODE | ❌ | Not implemented |
| STRDT | ❌ | Not implemented |
| STRLANG | ❌ | Not implemented |
| UUID | ❌ | Not implemented |
| STRUUID | ❌ | Not implemented |

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
| STRBEFORE | ❌ | Not implemented |
| STRAFTER | ❌ | Not implemented |
| ENCODE_FOR_URI | ❌ | Not implemented |
| CONCAT | ✅ | String concatenation |
| REPLACE | ✅ | Regex replace |
| REGEX | ✅ | Regex match |
| langMatches | ❌ | Not implemented |

### Numeric Functions

| Function | Status | Description |
|----------|--------|-------------|
| ABS | ✅ | Absolute value |
| ROUND | ✅ | Round to nearest |
| CEIL | ✅ | Ceiling |
| FLOOR | ✅ | Floor |
| RAND | ❌ | Not implemented |

### Date/Time Functions

| Function | Status | Description |
|----------|--------|-------------|
| NOW | ❌ | Not implemented |
| YEAR | ❌ | Not implemented |
| MONTH | ❌ | Not implemented |
| DAY | ❌ | Not implemented |
| HOURS | ❌ | Not implemented |
| MINUTES | ❌ | Not implemented |
| SECONDS | ❌ | Not implemented |
| TIMEZONE | ❌ | Not implemented |
| TZ | ❌ | Not implemented |

### Hash Functions

| Function | Status | Description |
|----------|--------|-------------|
| MD5 | ❌ | Not implemented |
| SHA1 | ❌ | Not implemented |
| SHA256 | ❌ | Not implemented |
| SHA384 | ❌ | Not implemented |
| SHA512 | ❌ | Not implemented |

### Conditional Functions

| Function | Status | Description |
|----------|--------|-------------|
| IF | ✅ | `IF(condition, then, else)` |
| COALESCE | ✅ | First non-error value |
| EXISTS | ✅ | Pattern existence |
| NOT EXISTS | ✅ | Pattern non-existence |
| IN | ✅ | `?x IN (1, 2, 3)` |
| NOT IN | 🚧 | Not implemented |

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

> **Note:** Subqueries are flattened instead of nested. Full subquery support planned.

| Feature | Status | Notes |
|---------|--------|-------|
| SELECT subqueries | 🚧 | Flattened; full subquery support planned |
| Correlated subqueries | 🚧 | Flattened; full subquery support planned |
| Nested subqueries | 🚧 | Flattened; full subquery support planned |

## Named Graphs

> **Note:** GRAPH pattern is traversed but no graph scoping is applied. FROM/FROM NAMED are not translated.

| Feature | Status | Notes |
|---------|--------|-------|
| GRAPH clause | 🚧 | Pattern traversed but no graph scoping |
| FROM | 🚧 | Not translated |
| FROM NAMED | 🚧 | Not translated |
| Graph variable | 🚧 | Pattern traversed but no graph scoping |

## Features Not Supported

| Feature | Status | Notes |
|---------|--------|-------|
| CONSTRUCT | ❌ | Not yet supported |
| DESCRIBE | ❌ | Not yet supported |
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

- [Command Reference](COMMANDS.md) - RDF.QUERY command details
- [RDF Mapping](RDF_MAPPING.md) - How queries map to Cypher
- [Performance Guide](../guides/PERFORMANCE.md) - Query optimization
