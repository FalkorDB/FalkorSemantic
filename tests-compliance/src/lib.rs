//! W3C Compliance Test Suite for FalkorSemantic
//!
//! This crate provides compliance testing against W3C test suites for:
//! - RDF 1.1 parsers (Turtle, N-Triples, N-Quads)
//! - SPARQL 1.1 query parsing
//!
//! # Test Suites
//!
//! ## RDF 1.1 Test Suites
//! - Turtle: https://www.w3.org/2013/TurtleTests/
//! - N-Triples: https://www.w3.org/2013/N-TriplesTests/
//! - N-Quads: https://www.w3.org/2013/N-QuadsTests/
//!
//! ## SPARQL 1.1 Test Suites
//! - Query: https://www.w3.org/2009/sparql/docs/tests/
//!
//! # Running Tests
//!
//! ```bash
//! # Run all compliance tests
//! cargo test -p falkorsemantic-compliance
//!
//! # Run specific test suite
//! cargo test -p falkorsemantic-compliance turtle
//! cargo test -p falkorsemantic-compliance ntriples
//! cargo test -p falkorsemantic-compliance sparql
//! ```

pub mod rdf;
pub mod sparql;
pub mod report;

use std::path::PathBuf;

/// Result of a single test case
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test case name/identifier
    pub name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Test type (positive/negative)
    pub test_type: TestType,
    /// Error message if failed
    pub error: Option<String>,
    /// Expected result (for comparison tests)
    pub expected: Option<String>,
    /// Actual result
    pub actual: Option<String>,
}

/// Type of test case
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestType {
    /// Should parse successfully
    PositiveParser,
    /// Should fail to parse
    NegativeParser,
    /// Should produce specific output
    PositiveEval,
    /// Query syntax test
    QuerySyntax,
    /// Query evaluation test
    QueryEval,
}

/// Compliance test report
#[derive(Debug, Default)]
pub struct ComplianceReport {
    /// Test suite name
    pub suite_name: String,
    /// Total number of tests
    pub total: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of skipped tests
    pub skipped: usize,
    /// Individual test results
    pub results: Vec<TestResult>,
    /// Compliance gaps (documented issues)
    pub gaps: Vec<ComplianceGap>,
}

/// A documented compliance gap
#[derive(Debug, Clone)]
pub struct ComplianceGap {
    /// Feature or test that is not compliant
    pub feature: String,
    /// Reason for non-compliance
    pub reason: String,
    /// Severity (low, medium, high)
    pub severity: GapSeverity,
    /// Related W3C specification section
    pub spec_reference: Option<String>,
}

/// Severity of a compliance gap
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GapSeverity {
    Low,
    Medium,
    High,
}

impl ComplianceReport {
    /// Create a new report for a test suite
    pub fn new(suite_name: impl Into<String>) -> Self {
        Self {
            suite_name: suite_name.into(),
            ..Default::default()
        }
    }

    /// Add a test result
    pub fn add_result(&mut self, result: TestResult) {
        if result.passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.total += 1;
        self.results.push(result);
    }

    /// Add a skipped test
    pub fn add_skipped(&mut self, name: String, reason: String) {
        self.skipped += 1;
        self.total += 1;
        self.results.push(TestResult {
            name,
            passed: false,
            test_type: TestType::PositiveParser,
            error: Some(format!("Skipped: {}", reason)),
            expected: None,
            actual: None,
        });
    }

    /// Add a compliance gap
    pub fn add_gap(&mut self, gap: ComplianceGap) {
        self.gaps.push(gap);
    }

    /// Calculate compliance percentage
    pub fn compliance_percentage(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        (self.passed as f64 / (self.total - self.skipped) as f64) * 100.0
    }

    /// Generate a summary string
    pub fn summary(&self) -> String {
        format!(
            "{}: {}/{} passed ({:.1}% compliant), {} failed, {} skipped",
            self.suite_name,
            self.passed,
            self.total - self.skipped,
            self.compliance_percentage(),
            self.failed,
            self.skipped
        )
    }
}

/// Get the fixtures directory path
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}
