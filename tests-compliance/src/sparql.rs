//! SPARQL 1.1 Compliance Tests
//!
//! Tests for SPARQL query parsing against W3C SPARQL 1.1 test suites.

use crate::{ComplianceGap, ComplianceReport, GapSeverity, TestResult, TestType};

use falkorsemantic_parser::SparqlParser;

/// Run SPARQL 1.1 Query syntax compliance tests
#[must_use] 
pub fn run_sparql_syntax_tests() -> ComplianceReport {
    let mut report = ComplianceReport::new("SPARQL 1.1 Query Syntax");

    // Positive syntax tests - should parse successfully
    let positive_tests = vec![
        // Basic patterns
        ("syntax-basic-01", "SELECT * WHERE { ?s ?p ?o }"),
        ("syntax-basic-02", "SELECT ?s ?p ?o WHERE { ?s ?p ?o }"),
        ("syntax-basic-03", "SELECT * { ?s ?p ?o }"),
        (
            "syntax-basic-04",
            "PREFIX : <http://example.org/> SELECT * { :s :p :o }",
        ),
        (
            "syntax-basic-05",
            "BASE <http://example.org/> SELECT * { <s> <p> <o> }",
        ),
        // Projections
        ("syntax-select-01", "SELECT ?x WHERE { ?x ?p ?o }"),
        ("syntax-select-02", "SELECT ?x ?y WHERE { ?x ?p ?y }"),
        (
            "syntax-select-distinct",
            "SELECT DISTINCT ?x WHERE { ?x ?p ?o }",
        ),
        (
            "syntax-select-reduced",
            "SELECT REDUCED ?x WHERE { ?x ?p ?o }",
        ),
        // Query forms
        (
            "syntax-construct-01",
            "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-construct-02",
            "CONSTRUCT { <http://example.org/s> <http://example.org/p> ?o } WHERE { ?s ?p ?o }",
        ),
        ("syntax-ask-01", "ASK { ?s ?p ?o }"),
        ("syntax-describe-01", "DESCRIBE ?x WHERE { ?x ?p ?o }"),
        ("syntax-describe-02", "DESCRIBE <http://example.org/x>"),
        // Filters
        (
            "syntax-filter-01",
            "SELECT * WHERE { ?s ?p ?o FILTER (?o = 42) }",
        ),
        (
            "syntax-filter-02",
            "SELECT * WHERE { ?s ?p ?o FILTER regex(?o, \"test\") }",
        ),
        (
            "syntax-filter-03",
            "SELECT * WHERE { ?s ?p ?o FILTER (bound(?o)) }",
        ),
        (
            "syntax-filter-04",
            "SELECT * WHERE { ?s ?p ?o FILTER (?o > 10 && ?o < 100) }",
        ),
        (
            "syntax-filter-05",
            "SELECT * WHERE { ?s ?p ?o FILTER (?o > 10 || ?o < 0) }",
        ),
        (
            "syntax-filter-06",
            "SELECT * WHERE { ?s ?p ?o FILTER (!bound(?o)) }",
        ),
        // Literals
        ("syntax-lit-01", "SELECT * { ?s ?p \"string\" }"),
        ("syntax-lit-02", "SELECT * { ?s ?p \"string\"@en }"),
        (
            "syntax-lit-03",
            "SELECT * { ?s ?p \"42\"^^<http://www.w3.org/2001/XMLSchema#integer> }",
        ),
        ("syntax-lit-04", "SELECT * { ?s ?p 42 }"),
        ("syntax-lit-05", "SELECT * { ?s ?p 3.14 }"),
        ("syntax-lit-06", "SELECT * { ?s ?p 1e10 }"),
        ("syntax-lit-07", "SELECT * { ?s ?p true }"),
        ("syntax-lit-08", "SELECT * { ?s ?p false }"),
        // OPTIONAL
        (
            "syntax-optional-01",
            "SELECT * WHERE { ?s ?p ?o OPTIONAL { ?s ?p2 ?o2 } }",
        ),
        (
            "syntax-optional-02",
            "SELECT * WHERE { ?s ?p ?o OPTIONAL { ?s ?p2 ?o2 OPTIONAL { ?s ?p3 ?o3 } } }",
        ),
        // UNION
        (
            "syntax-union-01",
            "SELECT * WHERE { { ?s ?p ?o } UNION { ?s ?p2 ?o2 } }",
        ),
        (
            "syntax-union-02",
            "SELECT * WHERE { { ?s ?p ?o } UNION { ?s ?p2 ?o2 } UNION { ?s ?p3 ?o3 } }",
        ),
        // Named graphs
        (
            "syntax-graph-01",
            "SELECT * WHERE { GRAPH <http://example.org/g> { ?s ?p ?o } }",
        ),
        (
            "syntax-graph-02",
            "SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }",
        ),
        // Blank nodes
        ("syntax-bnode-01", "SELECT * WHERE { _:b1 ?p ?o }"),
        ("syntax-bnode-02", "SELECT * WHERE { [ ?p ?o ] ?p2 ?o2 }"),
        ("syntax-bnode-03", "SELECT * WHERE { ?s ?p [ ?p2 ?o2 ] }"),
        // Collections
        ("syntax-collection-01", "SELECT * WHERE { ?s ?p () }"),
        ("syntax-collection-02", "SELECT * WHERE { ?s ?p (1 2 3) }"),
        // ORDER BY / LIMIT / OFFSET
        ("syntax-order-01", "SELECT * WHERE { ?s ?p ?o } ORDER BY ?o"),
        (
            "syntax-order-02",
            "SELECT * WHERE { ?s ?p ?o } ORDER BY ASC(?o)",
        ),
        (
            "syntax-order-03",
            "SELECT * WHERE { ?s ?p ?o } ORDER BY DESC(?o)",
        ),
        ("syntax-limit-01", "SELECT * WHERE { ?s ?p ?o } LIMIT 10"),
        ("syntax-offset-01", "SELECT * WHERE { ?s ?p ?o } OFFSET 5"),
        (
            "syntax-limit-offset",
            "SELECT * WHERE { ?s ?p ?o } LIMIT 10 OFFSET 5",
        ),
        // Aggregates
        (
            "syntax-aggregate-01",
            "SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-aggregate-02",
            "SELECT (SUM(?o) AS ?sum) WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-aggregate-03",
            "SELECT (AVG(?o) AS ?avg) WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-aggregate-04",
            "SELECT (MIN(?o) AS ?min) WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-aggregate-05",
            "SELECT (MAX(?o) AS ?max) WHERE { ?s ?p ?o }",
        ),
        (
            "syntax-aggregate-06",
            "SELECT ?s (COUNT(?o) AS ?c) WHERE { ?s ?p ?o } GROUP BY ?s",
        ),
        (
            "syntax-aggregate-07",
            "SELECT ?s (COUNT(?o) AS ?c) WHERE { ?s ?p ?o } GROUP BY ?s HAVING (COUNT(?o) > 5)",
        ),
        // BIND and VALUES
        (
            "syntax-bind-01",
            "SELECT * WHERE { ?s ?p ?o BIND (?o * 2 AS ?double) }",
        ),
        (
            "syntax-values-01",
            "SELECT * WHERE { VALUES ?x { 1 2 3 } ?s ?p ?x }",
        ),
        (
            "syntax-values-02",
            "SELECT * WHERE { ?s ?p ?o } VALUES ?x { 1 2 }",
        ),
        // Subqueries
        (
            "syntax-subquery-01",
            "SELECT * WHERE { { SELECT ?s WHERE { ?s ?p ?o } } }",
        ),
        // MINUS
        (
            "syntax-minus-01",
            "SELECT * WHERE { ?s ?p ?o MINUS { ?s <http://example.org/exclude> ?o } }",
        ),
        // NOT EXISTS / EXISTS
        (
            "syntax-exists-01",
            "SELECT * WHERE { ?s ?p ?o FILTER EXISTS { ?s <http://example.org/p2> ?o2 } }",
        ),
        (
            "syntax-not-exists-01",
            "SELECT * WHERE { ?s ?p ?o FILTER NOT EXISTS { ?s <http://example.org/p2> ?o2 } }",
        ),
        // Property paths
        (
            "syntax-path-01",
            "SELECT * WHERE { ?s <http://example.org/p>/<http://example.org/q> ?o }",
        ),
        (
            "syntax-path-02",
            "SELECT * WHERE { ?s <http://example.org/p>* ?o }",
        ),
        (
            "syntax-path-03",
            "SELECT * WHERE { ?s <http://example.org/p>+ ?o }",
        ),
        (
            "syntax-path-04",
            "SELECT * WHERE { ?s <http://example.org/p>? ?o }",
        ),
        (
            "syntax-path-05",
            "SELECT * WHERE { ?s ^<http://example.org/p> ?o }",
        ),
        (
            "syntax-path-06",
            "SELECT * WHERE { ?s (<http://example.org/p>|<http://example.org/q>) ?o }",
        ),
        // SERVICE (federated queries)
        (
            "syntax-service-01",
            "SELECT * WHERE { SERVICE <http://example.org/sparql> { ?s ?p ?o } }",
        ),
        (
            "syntax-service-02",
            "SELECT * WHERE { SERVICE SILENT <http://example.org/sparql> { ?s ?p ?o } }",
        ),
    ];

    let parser = SparqlParser::new();

    for (name, query) in positive_tests {
        let result = parser.parse(query);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_ok(),
            test_type: TestType::QuerySyntax,
            error: result.err().map(|e| e.to_string()),
            expected: None,
            actual: None,
        });
    }

    // Negative syntax tests - should fail to parse
    let negative_tests = vec![
        ("syntax-bad-01", "SELET * WHERE { ?s ?p ?o }"), // Typo in SELECT
        ("syntax-bad-02", "SELECT * WHERE { ?s ?p ?o"),  // Missing closing brace
        ("syntax-bad-03", "SELECT WHERE { ?s ?p ?o }"),  // Missing projection
        ("syntax-bad-04", "SELECT * { ?s ?p }"),         // Incomplete triple
        ("syntax-bad-05", "SELECT * WHERE ?s ?p ?o }"),  // Missing opening brace
    ];

    for (name, query) in negative_tests {
        let result = parser.parse(query);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_err(),
            test_type: TestType::QuerySyntax,
            error: if result.is_ok() {
                Some("Expected parse error but succeeded".to_string())
            } else {
                None
            },
            expected: Some("Parse error".to_string()),
            actual: Some(if result.is_ok() { "Success" } else { "Error" }.to_string()),
        });
    }

    // Document known compliance gaps
    report.add_gap(ComplianceGap {
        feature: "SPARQL 1.1 Update".to_string(),
        reason: "Update operations (INSERT, DELETE, LOAD, etc.) parsing not implemented"
            .to_string(),
        severity: GapSeverity::High,
        spec_reference: Some("SPARQL 1.1 Update Section 3".to_string()),
    });

    report.add_gap(ComplianceGap {
        feature: "Full-text search functions".to_string(),
        reason: "Functions like bif:contains are vendor extensions".to_string(),
        severity: GapSeverity::Low,
        spec_reference: None,
    });

    report
}

/// Run SPARQL 1.1 Update syntax compliance tests
#[must_use] 
pub fn run_sparql_update_tests() -> ComplianceReport {
    let mut report = ComplianceReport::new("SPARQL 1.1 Update Syntax");

    // SPARQL Update is not currently implemented - document as gap
    let update_operations = vec![
        ("INSERT DATA", "Insert static triples"),
        ("DELETE DATA", "Delete static triples"),
        ("DELETE/INSERT WHERE", "Modify data based on patterns"),
        ("CLEAR GRAPH", "Clear named graphs"),
        ("DROP GRAPH", "Drop named graphs"),
        ("CREATE GRAPH", "Create named graphs"),
        ("LOAD", "Load data from URI"),
        ("COPY", "Copy graph contents"),
        ("MOVE", "Move graph contents"),
        ("ADD", "Add graph contents"),
    ];

    for (op, _desc) in update_operations {
        report.add_skipped(
            format!("update-{}", op.to_lowercase().replace([' ', '/'], "-")),
            format!("SPARQL Update ({op}) not implemented"),
        );
    }

    // Document as major compliance gap
    report.add_gap(ComplianceGap {
        feature: "SPARQL 1.1 Update".to_string(),
        reason: "Full SPARQL Update specification not implemented. Update operations are handled via RDF.INSERT command instead.".to_string(),
        severity: GapSeverity::High,
        spec_reference: Some("https://www.w3.org/TR/sparql11-update/".to_string()),
    });

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparql_syntax_compliance() {
        let report = run_sparql_syntax_tests();
        println!("\n{}", report.summary());

        for result in &report.results {
            if !result.passed {
                println!("  FAILED: {} - {:?}", result.name, result.error);
            }
        }

        for gap in &report.gaps {
            println!("  GAP: {} ({:?})", gap.feature, gap.severity);
        }

        // SPARQL syntax is handled by spargebra, expect high compliance
        assert!(
            report.compliance_percentage() >= 90.0,
            "SPARQL syntax compliance too low: {:.1}%",
            report.compliance_percentage()
        );
    }

    #[test]
    fn test_sparql_update_compliance() {
        let report = run_sparql_update_tests();
        println!("\n{}", report.summary());

        for result in &report.results {
            if !result.passed {
                println!("  FAILED/SKIPPED: {} - {:?}", result.name, result.error);
            }
        }

        // Document update support status
        println!("\nUpdate support: {:.1}%", report.compliance_percentage());
    }
}
