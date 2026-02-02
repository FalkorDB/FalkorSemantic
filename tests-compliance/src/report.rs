//! Compliance Report Generation
//!
//! Utilities for generating compliance reports in various formats.

use crate::{ComplianceReport, GapSeverity};

/// Generate a markdown compliance report
pub fn generate_markdown_report(reports: &[ComplianceReport]) -> String {
    let mut md = String::new();
    
    md.push_str("# FalkorSemantic W3C Compliance Report\n\n");
    md.push_str("This document summarizes compliance with W3C RDF and SPARQL specifications.\n\n");
    
    // Summary table
    md.push_str("## Summary\n\n");
    md.push_str("| Test Suite | Passed | Failed | Skipped | Compliance |\n");
    md.push_str("|------------|--------|--------|---------|------------|\n");
    
    for report in reports {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}% |\n",
            report.suite_name,
            report.passed,
            report.failed,
            report.skipped,
            report.compliance_percentage()
        ));
    }
    
    md.push_str("\n");
    
    // Detailed results per suite
    for report in reports {
        md.push_str(&format!("## {}\n\n", report.suite_name));
        
        // Failed tests
        let failed: Vec<_> = report.results.iter()
            .filter(|r| !r.passed)
            .collect();
        
        if !failed.is_empty() {
            md.push_str("### Failed Tests\n\n");
            for result in failed {
                md.push_str(&format!("- **{}**: {}\n", 
                    result.name,
                    result.error.as_deref().unwrap_or("Unknown error")
                ));
            }
            md.push_str("\n");
        }
        
        // Compliance gaps
        if !report.gaps.is_empty() {
            md.push_str("### Known Compliance Gaps\n\n");
            for gap in &report.gaps {
                let severity = match gap.severity {
                    GapSeverity::Low => "🟢 Low",
                    GapSeverity::Medium => "🟡 Medium",
                    GapSeverity::High => "🔴 High",
                };
                md.push_str(&format!("- **{}** ({})\n", gap.feature, severity));
                md.push_str(&format!("  - {}\n", gap.reason));
                if let Some(ref spec) = gap.spec_reference {
                    md.push_str(&format!("  - Spec: {}\n", spec));
                }
            }
            md.push_str("\n");
        }
    }
    
    // Specification references
    md.push_str("## Specification References\n\n");
    md.push_str("- [RDF 1.1 Turtle](https://www.w3.org/TR/turtle/)\n");
    md.push_str("- [RDF 1.1 N-Triples](https://www.w3.org/TR/n-triples/)\n");
    md.push_str("- [RDF 1.1 N-Quads](https://www.w3.org/TR/n-quads/)\n");
    md.push_str("- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)\n");
    md.push_str("- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)\n");
    md.push_str("\n");
    
    // Test suite references
    md.push_str("## W3C Test Suites\n\n");
    md.push_str("- [RDF 1.1 Test Cases](https://www.w3.org/2013/RDFTests/)\n");
    md.push_str("- [SPARQL 1.1 Test Suite](https://www.w3.org/2009/sparql/docs/tests/)\n");
    
    md
}

/// Generate a JSON compliance report
pub fn generate_json_report(reports: &[ComplianceReport]) -> String {
    use serde_json::{json, Value};
    
    let suites: Vec<Value> = reports.iter().map(|r| {
        json!({
            "name": r.suite_name,
            "total": r.total,
            "passed": r.passed,
            "failed": r.failed,
            "skipped": r.skipped,
            "compliance_percentage": r.compliance_percentage(),
            "failed_tests": r.results.iter()
                .filter(|t| !t.passed)
                .map(|t| json!({
                    "name": t.name,
                    "error": t.error
                }))
                .collect::<Vec<_>>(),
            "gaps": r.gaps.iter().map(|g| json!({
                "feature": g.feature,
                "reason": g.reason,
                "severity": format!("{:?}", g.severity),
                "spec_reference": g.spec_reference
            })).collect::<Vec<_>>()
        })
    }).collect();
    
    let report = json!({
        "version": "1.0",
        "suites": suites,
        "overall_compliance": if reports.is_empty() { 
            100.0 
        } else {
            reports.iter().map(|r| r.compliance_percentage()).sum::<f64>() / reports.len() as f64
        }
    });
    
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf;
    use crate::sparql;
    
    #[test]
    fn test_generate_full_report() {
        let reports = vec![
            rdf::run_turtle_tests(),
            rdf::run_ntriples_tests(),
            rdf::run_nquads_tests(),
            sparql::run_sparql_syntax_tests(),
            sparql::run_sparql_update_tests(),
        ];
        
        let markdown = generate_markdown_report(&reports);
        println!("\n{}", markdown);
        
        assert!(markdown.contains("# FalkorSemantic W3C Compliance Report"));
        assert!(markdown.contains("RDF 1.1 Turtle"));
        assert!(markdown.contains("SPARQL 1.1 Query Syntax"));
    }
    
    #[test]
    fn test_generate_json_report() {
        let reports = vec![
            rdf::run_turtle_tests(),
            sparql::run_sparql_syntax_tests(),
        ];
        
        let json = generate_json_report(&reports);
        println!("\n{}", json);
        
        assert!(json.contains("\"version\": \"1.0\""));
        assert!(json.contains("\"compliance_percentage\""));
    }
}
