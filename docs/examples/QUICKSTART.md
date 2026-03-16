# Example Queries and Use Cases

Practical examples for common FalkorSemantic scenarios.

## Table of Contents

- [Quick Start Examples](#quick-start-examples)
- [Knowledge Graph Use Cases](#knowledge-graph-use-cases)
- [Data Integration](#data-integration)
- [Analytics Queries](#analytics-queries)
- [SPARQL Patterns](#sparql-patterns)

## Quick Start Examples

### Hello World

```bash
# Connect to Redis
redis-cli

# Create a simple knowledge graph
RDF.GRAPH CREATE hello

# Add data
RDF.INSERT hello '
@prefix ex: <http://example.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

ex:alice a foaf:Person ;
         foaf:name "Alice" ;
         foaf:knows ex:bob .

ex:bob a foaf:Person ;
       foaf:name "Bob" .
'

# Query it
RDF.QUERY hello '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name
WHERE { ?person foaf:name ?name }
'
```

### Social Network

```bash
RDF.GRAPH CREATE social

RDF.INSERT social '
@prefix sn: <http://example.org/social/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

sn:alice a foaf:Person ;
    foaf:name "Alice Smith" ;
    foaf:age 30 ;
    foaf:interest "photography", "hiking" ;
    foaf:knows sn:bob, sn:charlie .

sn:bob a foaf:Person ;
    foaf:name "Bob Johnson" ;
    foaf:age 28 ;
    foaf:interest "music", "hiking" ;
    foaf:knows sn:charlie .

sn:charlie a foaf:Person ;
    foaf:name "Charlie Brown" ;
    foaf:age 35 ;
    foaf:interest "photography", "music" .
'

# Find friends of friends
RDF.QUERY social '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT DISTINCT ?fof ?name
WHERE {
    <http://example.org/social/alice> foaf:knows/foaf:knows ?fof .
    ?fof foaf:name ?name .
    FILTER (?fof != <http://example.org/social/alice>)
}
'

# Find people with shared interests
RDF.QUERY social '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?person1 ?person2 ?sharedInterest
WHERE {
    ?person1 foaf:interest ?sharedInterest .
    ?person2 foaf:interest ?sharedInterest .
    ?person1 foaf:name ?name1 .
    ?person2 foaf:name ?name2 .
    FILTER (?person1 < ?person2)
}
'
```

## Knowledge Graph Use Cases

### Product Catalog

```bash
RDF.GRAPH CREATE products

RDF.INSERT products '
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.org/products/> .

ex:laptop1 a schema:Product ;
    schema:name "ProBook 15" ;
    schema:brand "TechCorp" ;
    schema:price 999.99 ;
    schema:category ex:laptops ;
    schema:offers [
        a schema:Offer ;
        schema:price 899.99 ;
        schema:availability schema:InStock ;
        schema:validThrough "2024-12-31"
    ] .

ex:laptop2 a schema:Product ;
    schema:name "UltraBook Air" ;
    schema:brand "TechCorp" ;
    schema:price 1299.99 ;
    schema:category ex:laptops .

ex:mouse1 a schema:Product ;
    schema:name "Wireless Mouse Pro" ;
    schema:brand "Peripherals Inc" ;
    schema:price 49.99 ;
    schema:category ex:accessories ;
    schema:isAccessoryFor ex:laptop1, ex:laptop2 .
'

# Find products on sale
RDF.QUERY products '
PREFIX schema: <http://schema.org/>
SELECT ?name ?originalPrice ?salePrice ?savings
WHERE {
    ?product schema:name ?name ;
             schema:price ?originalPrice ;
             schema:offers ?offer .
    ?offer schema:price ?salePrice ;
           schema:availability schema:InStock .
    BIND (?originalPrice - ?salePrice AS ?savings)
    FILTER (?savings > 0)
}
ORDER BY DESC(?savings)
'

# Find accessories for a product
RDF.QUERY products '
PREFIX schema: <http://schema.org/>
SELECT ?productName ?accessoryName ?accessoryPrice
WHERE {
    ?product schema:name ?productName .
    ?accessory schema:isAccessoryFor ?product ;
               schema:name ?accessoryName ;
               schema:price ?accessoryPrice .
}
'
```

### Organization Structure

```bash
RDF.GRAPH CREATE org

RDF.INSERT org '
@prefix org: <http://www.w3.org/ns/org#> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex: <http://example.org/company/> .

ex:acme a org:Organization ;
    foaf:name "ACME Corporation" .

ex:engineering a org:OrganizationalUnit ;
    org:unitOf ex:acme ;
    foaf:name "Engineering" .

ex:frontend a org:OrganizationalUnit ;
    org:unitOf ex:engineering ;
    foaf:name "Frontend Team" .

ex:backend a org:OrganizationalUnit ;
    org:unitOf ex:engineering ;
    foaf:name "Backend Team" .

ex:alice a foaf:Person ;
    foaf:name "Alice Smith" ;
    org:memberOf ex:frontend ;
    org:headOf ex:frontend .

ex:bob a foaf:Person ;
    foaf:name "Bob Johnson" ;
    org:memberOf ex:backend .

ex:charlie a foaf:Person ;
    foaf:name "Charlie Brown" ;
    org:memberOf ex:engineering ;
    org:headOf ex:engineering .
'

# Find all people in engineering (including sub-units)
RDF.QUERY org '
PREFIX org: <http://www.w3.org/ns/org#>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name ?unit
WHERE {
    ?unit org:unitOf* <http://example.org/company/engineering> .
    ?person org:memberOf ?unit ;
            foaf:name ?name .
}
'

# Find reporting structure
RDF.QUERY org '
PREFIX org: <http://www.w3.org/ns/org#>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?employee ?manager
WHERE {
    ?emp org:memberOf ?unit ;
         foaf:name ?employee .
    ?mgr org:headOf ?unit ;
         foaf:name ?manager .
    FILTER (?emp != ?mgr)
}
'
```

### Citation Network

```bash
RDF.GRAPH CREATE citations

RDF.INSERT citations '
@prefix bibo: <http://purl.org/ontology/bibo/> .
@prefix dc: <http://purl.org/dc/terms/> .
@prefix ex: <http://example.org/papers/> .

ex:paper1 a bibo:AcademicArticle ;
    dc:title "Introduction to Graph Databases" ;
    dc:creator "Alice Researcher" ;
    dc:date "2020" ;
    bibo:citedBy ex:paper2, ex:paper3 .

ex:paper2 a bibo:AcademicArticle ;
    dc:title "Property Graphs vs RDF" ;
    dc:creator "Bob Scholar" ;
    dc:date "2021" ;
    dc:references ex:paper1 ;
    bibo:citedBy ex:paper4 .

ex:paper3 a bibo:AcademicArticle ;
    dc:title "SPARQL Optimization Techniques" ;
    dc:creator "Charlie Academic" ;
    dc:date "2021" ;
    dc:references ex:paper1 .

ex:paper4 a bibo:AcademicArticle ;
    dc:title "Unified Graph Query Language" ;
    dc:creator "Diana Professor" ;
    dc:date "2023" ;
    dc:references ex:paper1, ex:paper2, ex:paper3 .
'

# Find most cited papers (requires aggregate support - planned)
RDF.QUERY citations '
PREFIX bibo: <http://purl.org/ontology/bibo/>
PREFIX dc: <http://purl.org/dc/terms/>
SELECT ?title (COUNT(?citing) AS ?citations)
WHERE {
    ?paper dc:title ?title .
    ?citing dc:references ?paper .
}
GROUP BY ?paper ?title
ORDER BY DESC(?citations)
LIMIT 10
'

# Find citation chains
RDF.QUERY citations '
PREFIX dc: <http://purl.org/dc/terms/>
SELECT ?original ?citing
WHERE {
    ?original dc:title "Introduction to Graph Databases" .
    ?cited dc:references+ ?original .
    ?cited dc:title ?citing .
}
'
```

## Data Integration

### Linking External Data

```bash
RDF.GRAPH CREATE integrated

# Link to DBpedia
RDF.INSERT integrated '
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix dbp: <http://dbpedia.org/resource/> .
@prefix ex: <http://example.org/> .

ex:paris a ex:City ;
    owl:sameAs dbp:Paris ;
    ex:population 2161000 ;
    ex:country ex:france .

ex:france a ex:Country ;
    owl:sameAs dbp:France ;
    ex:capital ex:paris .
'

# Query with linked data context
RDF.QUERY integrated '
PREFIX owl: <http://www.w3.org/2002/07/owl#>
PREFIX dbp: <http://dbpedia.org/resource/>
PREFIX ex: <http://example.org/>
SELECT ?city ?dbpediaLink ?population
WHERE {
    ?city a ex:City ;
          owl:sameAs ?dbpediaLink ;
          ex:population ?population .
    FILTER (STRSTARTS(STR(?dbpediaLink), "http://dbpedia.org"))
}
'
```

### Schema Mapping

```bash
# Map different schemas to common model
RDF.INSERT integrated '
@prefix schema: <http://schema.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex: <http://example.org/> .

# Source 1 uses foaf
ex:alice a foaf:Person ;
    foaf:name "Alice" ;
    foaf:mbox <mailto:alice@example.org> .

# Source 2 uses schema.org
ex:bob a schema:Person ;
    schema:name "Bob" ;
    schema:email "bob@example.org" .

# Define equivalences
foaf:Person owl:equivalentClass schema:Person .
foaf:name owl:equivalentProperty schema:name .
'

# Query across schemas
RDF.QUERY integrated '
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX schema: <http://schema.org/>
SELECT ?person ?name
WHERE {
    { ?person foaf:name ?name }
    UNION
    { ?person schema:name ?name }
}
'
```

## Analytics Queries

### Aggregation Examples

> **Note:** Aggregates (COUNT, SUM, AVG, etc.) and GROUP BY/HAVING are not yet fully translated to Cypher. The examples below show planned SPARQL support.

```bash
RDF.GRAPH CREATE analytics

RDF.INSERT analytics '
@prefix ex: <http://example.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

ex:order1 ex:customer ex:alice ; ex:amount "150.00"^^xsd:decimal ; ex:date "2024-01-15" .
ex:order2 ex:customer ex:alice ; ex:amount "89.50"^^xsd:decimal ; ex:date "2024-01-20" .
ex:order3 ex:customer ex:bob ; ex:amount "245.00"^^xsd:decimal ; ex:date "2024-01-18" .
ex:order4 ex:customer ex:bob ; ex:amount "67.00"^^xsd:decimal ; ex:date "2024-01-22" .
ex:order5 ex:customer ex:charlie ; ex:amount "520.00"^^xsd:decimal ; ex:date "2024-01-19" .

ex:alice ex:name "Alice" ; ex:tier "Gold" .
ex:bob ex:name "Bob" ; ex:tier "Silver" .
ex:charlie ex:name "Charlie" ; ex:tier "Gold" .
'

# Customer spending summary
RDF.QUERY analytics '
PREFIX ex: <http://example.org/>
SELECT ?name (SUM(?amount) AS ?total) (AVG(?amount) AS ?avg) (COUNT(?order) AS ?orders)
WHERE {
    ?order ex:customer ?customer ;
           ex:amount ?amount .
    ?customer ex:name ?name .
}
GROUP BY ?customer ?name
ORDER BY DESC(?total)
'

# Spending by tier
RDF.QUERY analytics '
PREFIX ex: <http://example.org/>
SELECT ?tier (SUM(?amount) AS ?totalSpend) (COUNT(DISTINCT ?customer) AS ?customers)
WHERE {
    ?order ex:customer ?customer ;
           ex:amount ?amount .
    ?customer ex:tier ?tier .
}
GROUP BY ?tier
'

# Top spenders (HAVING example)
RDF.QUERY analytics '
PREFIX ex: <http://example.org/>
SELECT ?name (SUM(?amount) AS ?total)
WHERE {
    ?order ex:customer ?customer ;
           ex:amount ?amount .
    ?customer ex:name ?name .
}
GROUP BY ?customer ?name
HAVING (SUM(?amount) > 200)
ORDER BY DESC(?total)
'
```

### Time-Based Analysis

```sparql
PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

# Orders by month
SELECT (SUBSTR(?date, 1, 7) AS ?month) (COUNT(?order) AS ?orderCount) (SUM(?amount) AS ?revenue)
WHERE {
    ?order ex:date ?date ;
           ex:amount ?amount .
}
GROUP BY (SUBSTR(?date, 1, 7))
ORDER BY ?month
```

## SPARQL Patterns

### OPTIONAL with Defaults

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name (COALESCE(?email, "No email") AS ?contact)
WHERE {
    ?person foaf:name ?name .
    OPTIONAL { ?person foaf:mbox ?email }
}
```

### Negation Patterns

```sparql
# Find people without email
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name
WHERE {
    ?person foaf:name ?name .
    FILTER NOT EXISTS { ?person foaf:mbox ?email }
}

# Find people not in any group
SELECT ?name
WHERE {
    ?person foaf:name ?name .
    MINUS { ?person foaf:member ?group }
}
```

### CONSTRUCT for Data Transformation

> **Note:** CONSTRUCT queries are not yet supported. This is a planned feature.

```sparql
# Transform schema
PREFIX schema: <http://schema.org/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
CONSTRUCT {
    ?person a foaf:Person ;
            foaf:name ?name ;
            foaf:mbox ?email .
}
WHERE {
    ?person a schema:Person ;
            schema:name ?name ;
            schema:email ?email .
}
```

### Subqueries

> **Note:** Subqueries are currently flattened instead of nested. Complex subqueries may not produce expected results.

```sparql
# Find people with above-average connections
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?person ?name ?connectionCount
WHERE {
    ?person foaf:name ?name .
    {
        SELECT ?person (COUNT(?friend) AS ?connectionCount)
        WHERE { ?person foaf:knows ?friend }
        GROUP BY ?person
    }
    {
        SELECT (AVG(?cnt) AS ?avgConnections)
        WHERE {
            SELECT ?p (COUNT(?f) AS ?cnt)
            WHERE { ?p foaf:knows ?f }
            GROUP BY ?p
        }
    }
    FILTER (?connectionCount > ?avgConnections)
}
```

### VALUES for Batch Lookup

```sparql
PREFIX ex: <http://example.org/>
SELECT ?product ?name ?price
WHERE {
    VALUES ?product { ex:prod1 ex:prod2 ex:prod3 }
    ?product ex:name ?name ;
             ex:price ?price .
}
```

### Property Paths

> **Note:** Property paths are parsed but currently simplified to a generic traversal pattern. Full path semantics are planned.

```sparql
# All ancestors (transitive)
PREFIX rel: <http://example.org/relations/>
SELECT ?ancestor
WHERE {
    <http://example.org/person/alice> rel:parent+ ?ancestor .
}

# Siblings (inverse + forward)
SELECT DISTINCT ?sibling
WHERE {
    <http://example.org/person/alice> rel:parent/^rel:parent ?sibling .
    FILTER (?sibling != <http://example.org/person/alice>)
}

# Any relationship within 3 hops
SELECT ?connected ?path
WHERE {
    <http://example.org/person/alice> (rel:knows|rel:worksWWith|rel:relatedTo){1,3} ?connected .
}
```

## Client Examples

### Python

```python
import redis
import json

r = redis.Redis(host='localhost', port=6379, decode_responses=True)

# Insert data
r.execute_command('RDF.INSERT', 'mykg', '''
    @prefix ex: <http://example.org/> .
    ex:alice ex:name "Alice" ; ex:age 30 .
''')

# Query
result = r.execute_command('RDF.QUERY', 'mykg', '''
    PREFIX ex: <http://example.org/>
    SELECT ?name ?age
    WHERE { ?person ex:name ?name ; ex:age ?age }
''')

data = json.loads(result)
for binding in data['results']['bindings']:
    print(f"Name: {binding['name']['value']}, Age: {binding['age']['value']}")
```

### Node.js

```javascript
import { createClient } from 'redis';

const client = createClient();
await client.connect();

// Insert
await client.sendCommand(['RDF.INSERT', 'mykg', `
  @prefix ex: <http://example.org/> .
  ex:alice ex:name "Alice" ; ex:age 30 .
`]);

// Query
const result = await client.sendCommand(['RDF.QUERY', 'mykg', `
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age WHERE { ?person ex:name ?name ; ex:age ?age }
`]);

const data = JSON.parse(result);
data.results.bindings.forEach(b => {
  console.log(`Name: ${b.name.value}, Age: ${b.age.value}`);
});
```

### Java

```java
import redis.clients.jedis.Jedis;
import org.json.JSONObject;

Jedis jedis = new Jedis("localhost", 6379);

// Insert
jedis.sendCommand(() -> "RDF.INSERT".getBytes(), "mykg", 
    "@prefix ex: <http://example.org/> . ex:alice ex:name \"Alice\" .");

// Query
Object result = jedis.sendCommand(() -> "RDF.QUERY".getBytes(), "mykg",
    "PREFIX ex: <http://example.org/> SELECT ?name WHERE { ?s ex:name ?name }");

JSONObject json = new JSONObject(result.toString());
// Process results...
```

## See Also

- [Command Reference](../reference/COMMANDS.md) - Full command syntax
- [SPARQL Reference](../reference/SPARQL.md) - SPARQL feature details
- [RDF Mapping](../reference/RDF_MAPPING.md) - Data model explanation
