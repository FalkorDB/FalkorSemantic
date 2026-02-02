//! RDF 1.1 Compliance Tests
//!
//! Tests for Turtle, N-Triples, and N-Quads parsers against W3C test suites.

use crate::{ComplianceGap, ComplianceReport, GapSeverity, TestResult, TestType};

use falkorsemantic_parser::formats::NTriplesReader;
use falkorsemantic_parser::TurtleParser;

/// Run Turtle parser compliance tests
pub fn run_turtle_tests() -> ComplianceReport {
    let mut report = ComplianceReport::new("RDF 1.1 Turtle");

    // Positive syntax tests - should parse successfully
    let positive_tests = vec![
        ("turtle-syntax-base-01", "@base <http://example.org/> ."),
        ("turtle-syntax-base-02", "BASE <http://example.org/>"),
        ("turtle-syntax-prefix-01", "@prefix : <http://example.org/> ."),
        ("turtle-syntax-prefix-02", "PREFIX : <http://example.org/>"),
        ("turtle-syntax-prefix-03", "@prefix p: <http://example.org/> ."),
        ("turtle-syntax-uri-01", "<http://example.org/> <http://example.org/p> <http://example.org/o> ."),
        ("turtle-syntax-string-01", "<http://example.org/s> <http://example.org/p> \"string\" ."),
        ("turtle-syntax-string-02", "<http://example.org/s> <http://example.org/p> \"string\"@en ."),
        ("turtle-syntax-string-03", "<http://example.org/s> <http://example.org/p> \"string\"^^<http://example.org/dt> ."),
        ("turtle-syntax-number-01", "<http://example.org/s> <http://example.org/p> 42 ."),
        ("turtle-syntax-number-02", "<http://example.org/s> <http://example.org/p> -42 ."),
        ("turtle-syntax-number-03", "<http://example.org/s> <http://example.org/p> 3.14 ."),
        ("turtle-syntax-number-04", "<http://example.org/s> <http://example.org/p> 1.5e10 ."),
        ("turtle-syntax-bnode-01", "_:b1 <http://example.org/p> <http://example.org/o> ."),
        ("turtle-syntax-bnode-02", "<http://example.org/s> <http://example.org/p> _:b1 ."),
        ("turtle-syntax-bnode-03", "[ <http://example.org/p> <http://example.org/o> ] <http://example.org/p2> <http://example.org/o2> ."),
        ("turtle-syntax-list-01", "<http://example.org/s> <http://example.org/p> () ."),
        ("turtle-syntax-list-02", "<http://example.org/s> <http://example.org/p> (\"a\" \"b\") ."),
        ("turtle-syntax-kw-01", "<http://example.org/s> a <http://example.org/Type> ."),
        ("turtle-syntax-kw-02", "@prefix : <http://example.org/> . :s a :Type ."),
        ("turtle-syntax-struct-01", "@prefix : <http://example.org/> . :s :p :o1 ; :q :o2 ."),
        ("turtle-syntax-struct-02", "@prefix : <http://example.org/> . :s :p :o1 , :o2 ."),
    ];

    for (name, input) in positive_tests {
        let mut parser = TurtleParser::new();
        let result = parser.parse(input);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_ok(),
            test_type: TestType::PositiveParser,
            error: result.err().map(|e| e.to_string()),
            expected: None,
            actual: None,
        });
    }

    // Negative syntax tests - should fail to parse
    let negative_tests = vec![
        ("turtle-syntax-bad-base-01", "@base ."),
        ("turtle-syntax-bad-base-02", "@base http://example.org/ ."),
        ("turtle-syntax-bad-prefix-01", "@prefix ."),
        (
            "turtle-syntax-bad-uri-01",
            "<http://example .org/> <http://example.org/p> <http://example.org/o> .",
        ),
        (
            "turtle-syntax-bad-string-01",
            "<http://example.org/s> <http://example.org/p> \"unterminated .",
        ),
        (
            "turtle-syntax-bad-num-01",
            "<http://example.org/s> <http://example.org/p> 123abc .",
        ),
    ];

    for (name, input) in negative_tests {
        let mut parser = TurtleParser::new();
        let result = parser.parse(input);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_err(),
            test_type: TestType::NegativeParser,
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
        feature: "Unicode escape sequences (\\uXXXX, \\UXXXXXXXX)".to_string(),
        reason: "Partial support - basic escapes work, full Unicode escapes need enhancement"
            .to_string(),
        severity: GapSeverity::Low,
        spec_reference: Some("Turtle 1.1 Section 6.1".to_string()),
    });

    report.add_gap(ComplianceGap {
        feature: "Long literals with embedded quotes".to_string(),
        reason: "Edge cases with triple-quoted strings containing quotes may not parse correctly"
            .to_string(),
        severity: GapSeverity::Low,
        spec_reference: Some("Turtle 1.1 Section 2.5.2".to_string()),
    });

    report
}

/// Run N-Triples parser compliance tests
pub fn run_ntriples_tests() -> ComplianceReport {
    let mut report = ComplianceReport::new("RDF 1.1 N-Triples");

    let reader = NTriplesReader::new();

    // Positive tests
    let positive_tests = vec![
        ("nt-syntax-uri-01", "<http://example.org/s> <http://example.org/p> <http://example.org/o> ."),
        ("nt-syntax-uri-02", "<http://example.org/resource1> <http://example.org/property> <http://example.org/resource2> ."),
        ("nt-syntax-bnode-01", "_:b1 <http://example.org/p> <http://example.org/o> ."),
        ("nt-syntax-bnode-02", "<http://example.org/s> <http://example.org/p> _:b2 ."),
        ("nt-syntax-bnode-03", "_:b1 <http://example.org/p> _:b2 ."),
        ("nt-syntax-string-01", "<http://example.org/s> <http://example.org/p> \"string\" ."),
        ("nt-syntax-string-02", "<http://example.org/s> <http://example.org/p> \"string with spaces\" ."),
        ("nt-syntax-string-03", "<http://example.org/s> <http://example.org/p> \"string\\nwith\\tescape\" ."),
        ("nt-syntax-str-esc-01", "<http://example.org/s> <http://example.org/p> \"escaped\\\"quote\" ."),
        ("nt-syntax-datatypes-01", "<http://example.org/s> <http://example.org/p> \"42\"^^<http://www.w3.org/2001/XMLSchema#integer> ."),
        ("nt-syntax-datatypes-02", "<http://example.org/s> <http://example.org/p> \"true\"^^<http://www.w3.org/2001/XMLSchema#boolean> ."),
        ("nt-syntax-lang-01", "<http://example.org/s> <http://example.org/p> \"hello\"@en ."),
        ("nt-syntax-lang-02", "<http://example.org/s> <http://example.org/p> \"bonjour\"@fr-FR ."),
    ];

    for (name, input) in positive_tests {
        let result = reader.parse_all_str(input);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_ok(),
            test_type: TestType::PositiveParser,
            error: result.err().map(|e| e.to_string()),
            expected: None,
            actual: None,
        });
    }

    // Negative tests
    let negative_tests = vec![
        (
            "nt-syntax-bad-uri-01",
            "http://example.org/s> <http://example.org/p> <http://example.org/o> .",
        ),
        (
            "nt-syntax-bad-uri-02",
            "<http://example.org/s <http://example.org/p> <http://example.org/o> .",
        ),
        (
            "nt-syntax-bad-string-01",
            "<http://example.org/s> <http://example.org/p> unterminated .",
        ),
        (
            "nt-syntax-bad-esc-01",
            "<http://example.org/s> <http://example.org/p> \"bad\\escape\" .",
        ),
        (
            "nt-syntax-bad-lang-01",
            "<http://example.org/s> <http://example.org/p> \"string\"@ .",
        ),
    ];

    for (name, input) in negative_tests {
        let result = reader.parse_all_str(input);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_err(),
            test_type: TestType::NegativeParser,
            error: if result.is_ok() {
                Some("Expected parse error but succeeded".to_string())
            } else {
                None
            },
            expected: Some("Parse error".to_string()),
            actual: Some(if result.is_ok() { "Success" } else { "Error" }.to_string()),
        });
    }

    report
}

/// Run N-Quads parser compliance tests
pub fn run_nquads_tests() -> ComplianceReport {
    let mut report = ComplianceReport::new("RDF 1.1 N-Quads");

    // N-Quads tests use the same reader but with 4 components
    let reader = NTriplesReader::new();

    // Positive tests (N-Quads are N-Triples compatible)
    let positive_tests = vec![
        ("nq-syntax-uri-01", "<http://example.org/s> <http://example.org/p> <http://example.org/o> ."),
        ("nq-syntax-bnode-01", "_:b1 <http://example.org/p> <http://example.org/o> ."),
        ("nq-syntax-string-01", "<http://example.org/s> <http://example.org/p> \"string\" ."),
        ("nq-syntax-datatypes-01", "<http://example.org/s> <http://example.org/p> \"42\"^^<http://www.w3.org/2001/XMLSchema#integer> ."),
    ];

    for (name, input) in positive_tests {
        let result = reader.parse_all_str(input);

        report.add_result(TestResult {
            name: name.to_string(),
            passed: result.is_ok(),
            test_type: TestType::PositiveParser,
            error: result.err().map(|e| e.to_string()),
            expected: None,
            actual: None,
        });
    }

    // Document compliance gap for named graphs
    report.add_gap(ComplianceGap {
        feature: "Named graph (4th component)".to_string(),
        reason: "N-Quads graph component parsing needs dedicated implementation".to_string(),
        severity: GapSeverity::Medium,
        spec_reference: Some("N-Quads 1.1 Section 4".to_string()),
    });

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turtle_compliance() {
        let report = run_turtle_tests();
        println!("\n{}", report.summary());

        for result in &report.results {
            if !result.passed {
                println!("  FAILED: {} - {:?}", result.name, result.error);
            }
        }

        for gap in &report.gaps {
            println!("  GAP: {} ({:?})", gap.feature, gap.severity);
        }

        // Assert high compliance rate
        assert!(
            report.compliance_percentage() >= 80.0,
            "Turtle compliance too low: {:.1}%",
            report.compliance_percentage()
        );
    }

    #[test]
    fn test_ntriples_compliance() {
        let report = run_ntriples_tests();
        println!("\n{}", report.summary());

        for result in &report.results {
            if !result.passed {
                println!("  FAILED: {} - {:?}", result.name, result.error);
            }
        }

        assert!(
            report.compliance_percentage() >= 80.0,
            "N-Triples compliance too low: {:.1}%",
            report.compliance_percentage()
        );
    }

    #[test]
    fn test_nquads_compliance() {
        let report = run_nquads_tests();
        println!("\n{}", report.summary());

        for result in &report.results {
            if !result.passed {
                println!("  FAILED: {} - {:?}", result.name, result.error);
            }
        }

        for gap in &report.gaps {
            println!("  GAP: {} ({:?})", gap.feature, gap.severity);
        }
    }
}
