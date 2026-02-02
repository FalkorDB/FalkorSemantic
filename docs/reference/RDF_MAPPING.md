# RDF to Property Graph Mapping

FalkorSemantic bridges the RDF/SPARQL world with FalkorDB's property graph model. This document explains how RDF concepts are mapped to property graph structures.

## Overview

| RDF Concept | Property Graph Mapping |
|-------------|------------------------|
| Subject (IRI) | Node with `:Resource` label |
| Subject (Blank Node) | Node with `:BlankNode` label |
| Predicate | Edge with predicate metadata |
| Object (IRI) | Node with `:Resource` label |
| Object (Literal) | `:Literal` node connected via edge |
| rdf:type | Additional node label |
| Named Graph | Graph isolation via FalkorDB graph key |

## Detailed Mappings

### IRIs (Resources)

IRIs become nodes with the `:Resource` label and a `uri` property:

**RDF (N-Triples):**
```
<http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/bob> .
```

**Cypher:**
```cypher
MERGE (s:Resource {uri: 'http://example.org/alice'})
MERGE (o:Resource {uri: 'http://example.org/bob'})
MERGE (s)-[:knows{predicate: 'http://xmlns.com/foaf/0.1/knows'}]->(o)
```

### Blank Nodes

Blank nodes become nodes with the `:BlankNode` label:

**RDF (Turtle):**
```turtle
_:person1 foaf:name "Unknown Person" .
```

**Cypher:**
```cypher
MERGE (s:BlankNode {uri: '_:person1', isBlank: true})
MERGE (l:Literal {value: 'Unknown Person', datatype: 'http://www.w3.org/2001/XMLSchema#string'})
MERGE (s)-[:name{predicate: 'http://xmlns.com/foaf/0.1/name', value: 'Unknown Person'}]->(l)
```

### Literals as Nodes

Literals are stored as `:Literal` nodes connected to the subject via edges. The edge contains the predicate URI and value metadata:

**RDF (Turtle):**
```turtle
ex:alice foaf:name "Alice" ;
         foaf:age 30 .
```

**Cypher:**
```cypher
MERGE (alice:Resource {uri: 'http://example.org/alice'})
MERGE (name:Literal {value: 'Alice', datatype: 'http://www.w3.org/2001/XMLSchema#string'})
MERGE (alice)-[:name{predicate: 'http://xmlns.com/foaf/0.1/name', value: 'Alice'}]->(name)
MERGE (age:Literal {value: 30, datatype: 'http://www.w3.org/2001/XMLSchema#integer'})
MERGE (alice)-[:age{predicate: 'http://xmlns.com/foaf/0.1/age', value: 30}]->(age)
```

This approach:
- Preserves full RDF semantics (multi-valued properties, datatype information)
- Enables efficient SPARQL query translation
- Supports language-tagged literals

### Typed Literals

Datatype information is preserved in Literal nodes:

| XSD Type | Cypher Value Type |
|----------|-------------------|
| `xsd:string` | String |
| `xsd:integer` | Integer |
| `xsd:decimal` | Float |
| `xsd:float` | Float |
| `xsd:double` | Float |
| `xsd:boolean` | Boolean |
| `xsd:dateTime` | String (ISO 8601) |
| `xsd:date` | String (ISO 8601) |

**RDF:**
```turtle
ex:product ex:price "29.99"^^xsd:decimal ;
           ex:inStock "true"^^xsd:boolean ;
           ex:quantity "100"^^xsd:integer .
```

**Cypher:**
```cypher
MERGE (p:Resource {uri: 'http://example.org/product'})
MERGE (price:Literal {value: 29.99, datatype: 'http://www.w3.org/2001/XMLSchema#decimal'})
MERGE (p)-[:price{predicate: 'http://example.org/price', value: 29.99}]->(price)
MERGE (inStock:Literal {value: true, datatype: 'http://www.w3.org/2001/XMLSchema#boolean'})
MERGE (p)-[:inStock{predicate: 'http://example.org/inStock', value: true}]->(inStock)
MERGE (qty:Literal {value: 100, datatype: 'http://www.w3.org/2001/XMLSchema#integer'})
MERGE (p)-[:quantity{predicate: 'http://example.org/quantity', value: 100}]->(qty)
```

### Language-Tagged Literals

Language tags are preserved in Literal nodes:

**RDF:**
```turtle
ex:paris rdfs:label "Paris"@en ;
         rdfs:label "Paris"@fr ;
         rdfs:label "パリ"@ja .
```

**Cypher:**
```cypher
MERGE (paris:Resource {uri: 'http://example.org/paris'})
MERGE (l1:Literal {value: 'Paris', datatype: 'http://www.w3.org/1999/02/22-rdf-syntax-ns#langString', language: 'en'})
MERGE (paris)-[:label{predicate: 'http://www.w3.org/2000/01/rdf-schema#label', value: 'Paris', language: 'en'}]->(l1)
MERGE (l2:Literal {value: 'Paris', language: 'fr'})
MERGE (paris)-[:label{predicate: 'http://www.w3.org/2000/01/rdf-schema#label', value: 'Paris', language: 'fr'}]->(l2)
MERGE (l3:Literal {value: 'パリ', language: 'ja'})
MERGE (paris)-[:label{predicate: 'http://www.w3.org/2000/01/rdf-schema#label', value: 'パリ', language: 'ja'}]->(l3)
```

### rdf:type Mapping

The `rdf:type` predicate adds labels to nodes:

**RDF:**
```turtle
ex:alice a foaf:Person ;
         a ex:Employee .
```

**Cypher:**
```cypher
MERGE (alice:Resource:Person:Employee {uri: 'http://example.org/alice'})
```

The type's local name becomes a label. Full type information is also preserved:

```cypher
MERGE (alice:Resource:Person:Employee {uri: 'http://example.org/alice'})
SET alice.`rdf:type` = ['http://xmlns.com/foaf/0.1/Person', 'http://example.org/Employee']
```

### Multi-Valued Properties

RDF allows multiple values for the same predicate. These become arrays:

**RDF:**
```turtle
ex:alice foaf:interest "Reading" ;
         foaf:interest "Hiking" ;
         foaf:interest "Cooking" .
```

**Cypher:**
```cypher
MERGE (alice:Resource {uri: 'http://example.org/alice'})
SET alice.`foaf:interest` = ['Reading', 'Hiking', 'Cooking']
```

### Relationship Properties

When a predicate connects two resources, it becomes an edge:

**RDF:**
```turtle
ex:alice foaf:knows ex:bob .
ex:bob foaf:knows ex:charlie .
```

**Cypher:**
```cypher
MERGE (alice:Resource {uri: 'http://example.org/alice'})
MERGE (bob:Resource {uri: 'http://example.org/bob'})
MERGE (charlie:Resource {uri: 'http://example.org/charlie'})
CREATE (alice)-[:knows]->(bob)
CREATE (bob)-[:knows]->(charlie)
```

### Reified Statements

RDF reification (statements about statements) maps to intermediate nodes:

**RDF:**
```turtle
ex:statement1 a rdf:Statement ;
    rdf:subject ex:alice ;
    rdf:predicate ex:says ;
    rdf:object "Hello" ;
    ex:source ex:twitter ;
    ex:timestamp "2024-01-15T10:30:00Z"^^xsd:dateTime .
```

**Cypher:**
```cypher
MERGE (stmt:Resource:Statement {uri: 'http://example.org/statement1'})
MERGE (alice:Resource {uri: 'http://example.org/alice'})
MERGE (twitter:Resource {uri: 'http://example.org/twitter'})
CREATE (stmt)-[:subject]->(alice)
SET stmt.`rdf:predicate` = 'http://example.org/says'
SET stmt.`rdf:object` = 'Hello'
CREATE (stmt)-[:source]->(twitter)
SET stmt.`ex:timestamp` = '2024-01-15T10:30:00Z'
```

## SPARQL to Cypher Translation

### Basic Triple Pattern

**SPARQL:**
```sparql
SELECT ?name
WHERE {
  ?person foaf:name ?name .
}
```

**Cypher:**
```cypher
MATCH (person:Resource)
WHERE person.`foaf:name` IS NOT NULL
RETURN person.`foaf:name` AS name
```

### Relationship Pattern

**SPARQL:**
```sparql
SELECT ?person ?friend
WHERE {
  ?person foaf:knows ?friend .
}
```

**Cypher:**
```cypher
MATCH (person:Resource)-[:knows]->(friend:Resource)
RETURN person.uri AS person, friend.uri AS friend
```

### OPTIONAL Pattern

**SPARQL:**
```sparql
SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
}
```

**Cypher:**
```cypher
MATCH (person:Resource)
WHERE person.`foaf:name` IS NOT NULL
OPTIONAL MATCH (person)-[:mbox]->(email:Resource)
RETURN person.`foaf:name` AS name, email.uri AS email
```

### FILTER Pattern

**SPARQL:**
```sparql
SELECT ?name ?age
WHERE {
  ?person foaf:name ?name ;
          foaf:age ?age .
  FILTER (?age >= 18)
}
```

**Cypher:**
```cypher
MATCH (person:Resource)
WHERE person.`foaf:name` IS NOT NULL
  AND person.`foaf:age` IS NOT NULL
  AND person.`foaf:age` >= 18
RETURN person.`foaf:name` AS name, person.`foaf:age` AS age
```

### UNION Pattern

**SPARQL:**
```sparql
SELECT ?name
WHERE {
  { ?person foaf:name ?name }
  UNION
  { ?person rdfs:label ?name }
}
```

**Cypher:**
```cypher
MATCH (person:Resource)
WHERE person.`foaf:name` IS NOT NULL
RETURN person.`foaf:name` AS name
UNION
MATCH (person:Resource)
WHERE person.`rdfs:label` IS NOT NULL
RETURN person.`rdfs:label` AS name
```

### Property Path Translation

| SPARQL Path | Cypher Pattern |
|-------------|----------------|
| `foaf:knows/foaf:name` | `(a)-[:knows]->(b) WHERE b.\`foaf:name\`` |
| `foaf:knows*` | `(a)-[:knows*0..]->(b)` |
| `foaf:knows+` | `(a)-[:knows*1..]->(b)` |
| `foaf:knows?` | `(a)-[:knows*0..1]->(b)` |
| `foaf:knows{2,5}` | `(a)-[:knows*2..5]->(b)` |
| `^foaf:knows` | `(a)<-[:knows]-(b)` |

**SPARQL (transitive):**
```sparql
SELECT ?ancestor
WHERE {
  ex:alice foaf:knows+ ?ancestor .
}
```

**Cypher:**
```cypher
MATCH (alice:Resource {uri: 'http://example.org/alice'})-[:knows*1..]->(ancestor:Resource)
RETURN DISTINCT ancestor.uri AS ancestor
```

### Aggregation

**SPARQL:**
```sparql
SELECT ?type (COUNT(?instance) AS ?count)
WHERE {
  ?instance a ?type .
}
GROUP BY ?type
ORDER BY DESC(?count)
LIMIT 10
```

**Cypher:**
```cypher
MATCH (instance:Resource)
UNWIND labels(instance) AS type
WHERE type <> 'Resource' AND type <> 'BNode'
RETURN type, COUNT(instance) AS count
ORDER BY count DESC
LIMIT 10
```

## Namespace Handling

Namespaces are stored separately and used for prefix compression in queries and exports.

**Storage:**
- Prefix mappings stored in Redis hash: `{graph}:namespaces`
- Common prefixes (rdf, rdfs, xsd, owl) pre-registered

**Query Resolution:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name WHERE { ?person foaf:name ?name }
```

The prefix `foaf:name` is expanded to `http://xmlns.com/foaf/0.1/name` for matching, then compressed back for display.

## Schema Considerations

### Labels

| Label | Description |
|-------|-------------|
| `:Resource` | All IRI-identified nodes |
| `:BNode` | Blank nodes |
| Custom labels | From `rdf:type` local names |

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `uri` | String | Full IRI for resources |
| `id` | String | Blank node identifier |
| `{prefix}:{localname}` | Various | Literal values |
| `{prefix}:{localname}@{lang}` | String | Language-tagged values |
| `rdf:type` | Array | Type IRIs |

### Indexes

For optimal query performance, create indexes:

```cypher
// Index on resource URIs (critical)
CREATE INDEX ON :Resource(uri)

// Index on blank node IDs
CREATE INDEX ON :BNode(id)

// Index on common properties
CREATE INDEX ON :Resource(`foaf:name`)
CREATE INDEX ON :Resource(`rdfs:label`)
```

## Limitations

1. **RDF-star (RDF 1.2)** - Not currently supported
2. **Blank node scoping** - Blank nodes are graph-local
3. **Quad storage** - Named graphs use separate FalkorDB graphs
4. **Inference** - RDFS/OWL inference not built-in

## Best Practices

### Use Consistent Predicates

```turtle
# Good - consistent property
ex:alice foaf:name "Alice" .
ex:bob foaf:name "Bob" .

# Avoid - mixed properties for same concept
ex:alice foaf:name "Alice" .
ex:bob rdfs:label "Bob" .
```

### Leverage rdf:type for Labels

```turtle
# Creates useful labels for querying
ex:alice a foaf:Person, ex:Employee .
```

Enables queries like:
```cypher
MATCH (p:Person:Employee) RETURN p
```

### Use Namespaces

```turtle
# Good - uses prefixes
@prefix ex: <http://example.org/> .
ex:alice ex:knows ex:bob .

# Avoid - long URIs everywhere
<http://example.org/alice> <http://example.org/knows> <http://example.org/bob> .
```

## See Also

- [SPARQL Feature Matrix](SPARQL.md) - Query capabilities
- [Command Reference](COMMANDS.md) - Insert and query commands
- [Performance Guide](../guides/PERFORMANCE.md) - Optimization tips
