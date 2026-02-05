//! SPARQL to Cypher Translator
//!
//! Translates SPARQL queries to Cypher queries for execution against FalkorDB.

use std::collections::HashSet;
use std::fmt::Write;

use falkorsemantic_parser::sparql::{GraphPattern, Query, SelectQuery, Variable};

use crate::graph::escape_cypher_string;
use crate::MapperError;

/// Translates SPARQL queries to Cypher
#[derive(Debug, Default)]
pub struct SparqlToCypher {
    /// Variable counter for generating unique Cypher variables
    var_counter: std::cell::RefCell<usize>,
}

impl SparqlToCypher {
    /// Create a new translator
    pub fn new() -> Self {
        Self::default()
    }

    /// Translate a SPARQL query to Cypher
    pub fn translate(&self, query: &Query) -> Result<CypherQuery, MapperError> {
        match query {
            Query::Select(select) => self.translate_select(select),
            Query::Ask(ask) => self.translate_ask(&ask.pattern),
            Query::Construct(_) => Err(MapperError::MappingError(
                "CONSTRUCT queries not yet supported".into(),
            )),
            Query::Describe(_) => Err(MapperError::MappingError(
                "DESCRIBE queries not yet supported".into(),
            )),
        }
    }

    /// Translate a SELECT query
    fn translate_select(&self, select: &SelectQuery) -> Result<CypherQuery, MapperError> {
        let mut cypher = String::new();

        // Translate the graph pattern to MATCH clause
        let (match_clause, where_clause, bound_vars) =
            self.translate_graph_pattern(&select.pattern)?;

        cypher.push_str(&match_clause);

        if !where_clause.is_empty() {
            write!(cypher, "\nWHERE {}", where_clause).unwrap();
        }

        // Build RETURN clause
        let return_vars = if select.is_select_all() {
            bound_vars.iter().cloned().collect::<Vec<_>>()
        } else {
            select.projected_variables()
        };

        let return_clause = self.build_return_clause(&return_vars, select.distinct);
        cypher.push_str(&return_clause);

        // Handle ORDER BY
        if let Some(ref order_by) = select.order_by {
            let order_clause = self.build_order_clause(order_by);
            cypher.push_str(&order_clause);
        }

        // Handle LIMIT and OFFSET
        if let Some(limit) = select.limit {
            write!(cypher, "\nLIMIT {}", limit).unwrap();
        }
        if let Some(offset) = select.offset {
            write!(cypher, "\nSKIP {}", offset).unwrap();
        }

        Ok(CypherQuery {
            query: cypher,
            variables: return_vars.iter().map(|v| v.name.clone()).collect(),
            query_type: CypherQueryType::Select,
        })
    }

    /// Translate an ASK query
    fn translate_ask(&self, pattern: &GraphPattern) -> Result<CypherQuery, MapperError> {
        let (match_clause, where_clause, _) = self.translate_graph_pattern(pattern)?;

        let mut cypher = match_clause;
        if !where_clause.is_empty() {
            write!(cypher, "\nWHERE {}", where_clause).unwrap();
        }
        cypher.push_str("\nRETURN count(*) > 0 AS result");

        Ok(CypherQuery {
            query: cypher,
            variables: vec!["result".to_string()],
            query_type: CypherQueryType::Ask,
        })
    }

    /// Translate a graph pattern to MATCH and WHERE clauses
    fn translate_graph_pattern(
        &self,
        pattern: &GraphPattern,
    ) -> Result<(String, String, HashSet<Variable>), MapperError> {
        let mut match_parts = Vec::new();
        let mut where_parts = Vec::new();
        let mut bound_vars = HashSet::new();

        self.process_pattern(
            pattern.inner(),
            &mut match_parts,
            &mut where_parts,
            &mut bound_vars,
        )?;

        let match_clause = if match_parts.is_empty() {
            "MATCH (n)".to_string() // Fallback for empty patterns
        } else {
            format!("MATCH {}", match_parts.join(", "))
        };

        let where_clause = where_parts.join(" AND ");

        Ok((match_clause, where_clause, bound_vars))
    }

    /// Process a spargebra graph pattern recursively
    fn process_pattern(
        &self,
        pattern: &spargebra::algebra::GraphPattern,
        match_parts: &mut Vec<String>,
        where_parts: &mut Vec<String>,
        bound_vars: &mut HashSet<Variable>,
    ) -> Result<(), MapperError> {
        use spargebra::algebra::GraphPattern as GP;

        match pattern {
            GP::Bgp { patterns } => {
                for triple in patterns {
                    let (match_part, conditions, vars) = self.translate_triple_pattern(triple)?;
                    match_parts.push(match_part);
                    where_parts.extend(conditions);
                    bound_vars.extend(vars);
                }
            }
            GP::Join { left, right } => {
                self.process_pattern(left, match_parts, where_parts, bound_vars)?;
                self.process_pattern(right, match_parts, where_parts, bound_vars)?;
            }
            GP::LeftJoin {
                left,
                right,
                expression,
            } => {
                self.process_pattern(left, match_parts, where_parts, bound_vars)?;
                // OPTIONAL becomes OPTIONAL MATCH in Cypher
                let mut opt_match = Vec::new();
                let mut opt_where = Vec::new();
                self.process_pattern(right, &mut opt_match, &mut opt_where, bound_vars)?;
                if !opt_match.is_empty() {
                    match_parts.push(format!("OPTIONAL MATCH {}", opt_match.join(", ")));
                }
                // Handle the expression (filter on optional)
                if let Some(expr) = expression {
                    if let Some(filter) = self.translate_expression(expr)? {
                        where_parts.push(filter);
                    }
                }
            }
            GP::Filter { inner, expr } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
                if let Some(filter) = self.translate_expression(expr)? {
                    where_parts.push(filter);
                }
            }
            GP::Union { .. } => {
                // UNION queries require generating multiple separate Cypher queries
                // and combining results, which is not yet implemented.
                // Return an error rather than silently dropping the right branch.
                return Err(MapperError::MappingError(
                    "SPARQL UNION queries are not yet supported. \
                     Consider rewriting as separate queries."
                        .into(),
                ));
            }
            GP::Extend {
                inner,
                variable,
                expression: _,
            } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
                bound_vars.insert(Variable::from(variable.clone()));
                // BIND becomes WITH ... AS in Cypher
            }
            GP::OrderBy { inner, .. } | GP::Distinct { inner } | GP::Reduced { inner } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
            }
            GP::Project { inner, variables } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
                for v in variables {
                    bound_vars.insert(Variable::from(v.clone()));
                }
            }
            GP::Slice { inner, .. } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
            }
            GP::Graph { inner, .. } => {
                // Named graph - translate inner pattern
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
            }
            GP::Group { inner, .. } => {
                self.process_pattern(inner, match_parts, where_parts, bound_vars)?;
            }
            GP::Values {
                variables,
                bindings: _,
            } => {
                // VALUES clause - translate to WHERE ... IN ...
                for var in variables {
                    bound_vars.insert(Variable::from(var.clone()));
                }
            }
            GP::Service { .. } => {
                return Err(MapperError::MappingError(
                    "SERVICE clause not supported".into(),
                ));
            }
            GP::Path {
                subject,
                path: _,
                object,
            } => {
                // Property path - simplified translation
                let (subj_var, subj_cond) = self.term_to_cypher(subject)?;
                let (obj_var, obj_cond) = self.term_to_cypher(object)?;

                // Basic path translation (simplified)
                let path_pattern = format!("({})-[*]->({})", subj_var, obj_var);
                match_parts.push(path_pattern);

                if let Some(cond) = subj_cond {
                    where_parts.push(cond);
                }
                if let Some(cond) = obj_cond {
                    where_parts.push(cond);
                }

                self.add_term_variable(subject, bound_vars);
                self.add_term_variable(object, bound_vars);
            }
            GP::Minus { left, right } => {
                // MINUS becomes NOT EXISTS in Cypher
                self.process_pattern(left, match_parts, where_parts, bound_vars)?;
                let mut minus_match = Vec::new();
                let mut minus_where = Vec::new();
                let mut minus_vars = HashSet::new();
                self.process_pattern(right, &mut minus_match, &mut minus_where, &mut minus_vars)?;
                if !minus_match.is_empty() {
                    where_parts.push(format!("NOT EXISTS {{ MATCH {} }}", minus_match.join(", ")));
                }
            }
        }

        Ok(())
    }

    /// Translate a triple pattern to Cypher MATCH pattern
    fn translate_triple_pattern(
        &self,
        triple: &spargebra::term::TriplePattern,
    ) -> Result<(String, Vec<String>, HashSet<Variable>), MapperError> {
        let mut conditions = Vec::new();
        let mut bound_vars = HashSet::new();

        // Translate subject
        let (subj_var, subj_cond) = self.term_to_cypher(&triple.subject)?;
        if let Some(cond) = subj_cond {
            conditions.push(cond);
        }
        self.add_term_variable(&triple.subject, &mut bound_vars);

        // Translate predicate
        let pred_str = self.predicate_to_cypher(&triple.predicate)?;
        self.add_predicate_variable(&triple.predicate, &mut bound_vars);

        // Translate object
        let (obj_var, obj_cond) = self.term_to_cypher(&triple.object)?;
        if let Some(cond) = obj_cond {
            conditions.push(cond);
        }
        self.add_term_variable(&triple.object, &mut bound_vars);

        // Generate unique relationship variable
        let rel_var = self.next_var("r");

        // Build MATCH pattern
        let pattern = format!("({})-[{}{}]->({})", subj_var, rel_var, pred_str, obj_var);

        Ok((pattern, conditions, bound_vars))
    }

    /// Convert a term pattern to Cypher variable/value
    fn term_to_cypher(
        &self,
        term: &spargebra::term::TermPattern,
    ) -> Result<(String, Option<String>), MapperError> {
        use spargebra::term::TermPattern as TP;

        match term {
            TP::Variable(v) => Ok((v.as_str().to_string(), None)),
            TP::NamedNode(n) => {
                let var = self.next_var("n");
                let condition = format!("{}.uri = '{}'", var, escape_cypher_string(n.as_str()));
                Ok((var, Some(condition)))
            }
            TP::BlankNode(b) => {
                let var = self.next_var("b");
                let condition = format!("{}.uri = '_:{}'", var, b.as_str());
                Ok((var, Some(condition)))
            }
            TP::Literal(lit) => {
                let var = self.next_var("l");
                let value = escape_cypher_string(lit.value());
                let condition = format!("{}.value = '{}'", var, value);
                Ok((var, Some(condition)))
            }
            #[allow(unreachable_patterns)]
            _ => Err(MapperError::MappingError("Unsupported term pattern".into())),
        }
    }

    /// Convert predicate to Cypher relationship type
    fn predicate_to_cypher(
        &self,
        pred: &spargebra::term::NamedNodePattern,
    ) -> Result<String, MapperError> {
        use spargebra::term::NamedNodePattern as NNP;

        match pred {
            NNP::NamedNode(n) => {
                // Use predicate property for matching
                Ok(format!(
                    "{{predicate: '{}'}}",
                    escape_cypher_string(n.as_str())
                ))
            }
            NNP::Variable(_v) => {
                // Variable predicate - no type constraint
                Ok(String::new())
            }
        }
    }

    /// Add variable from term pattern to bound vars set
    fn add_term_variable(
        &self,
        term: &spargebra::term::TermPattern,
        bound_vars: &mut HashSet<Variable>,
    ) {
        if let spargebra::term::TermPattern::Variable(v) = term {
            bound_vars.insert(Variable::from(v.clone()));
        }
    }

    /// Add variable from predicate pattern to bound vars set
    fn add_predicate_variable(
        &self,
        pred: &spargebra::term::NamedNodePattern,
        bound_vars: &mut HashSet<Variable>,
    ) {
        if let spargebra::term::NamedNodePattern::Variable(v) = pred {
            bound_vars.insert(Variable::from(v.clone()));
        }
    }

    /// Translate a SPARQL expression to Cypher
    fn translate_expression(
        &self,
        expr: &spargebra::algebra::Expression,
    ) -> Result<Option<String>, MapperError> {
        use spargebra::algebra::{Expression as E, Function as F};

        let result = match expr {
            E::Equal(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} = {}", l, r)),
                    _ => None,
                }
            }
            E::SameTerm(left, right) => {
                // sameTerm is strict equality
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} = {}", l, r)),
                    _ => None,
                }
            }
            E::Less(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} < {}", l, r)),
                    _ => None,
                }
            }
            E::Greater(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} > {}", l, r)),
                    _ => None,
                }
            }
            E::LessOrEqual(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} <= {}", l, r)),
                    _ => None,
                }
            }
            E::GreaterOrEqual(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{} >= {}", l, r)),
                    _ => None,
                }
            }
            E::And(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("({} AND {})", l, r)),
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    _ => None,
                }
            }
            E::Or(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("({} OR {})", l, r)),
                    _ => None,
                }
            }
            E::Not(inner) => {
                let i = self.translate_expression(inner)?;
                i.map(|i| format!("NOT ({})", i))
            }
            E::Variable(v) => Some(v.as_str().to_string()),
            E::Literal(lit) => {
                let value = lit.value();
                Some(format!("'{}'", escape_cypher_string(value)))
            }
            E::NamedNode(node) => Some(format!("'{}'", escape_cypher_string(node.as_str()))),
            E::Bound(v) => Some(format!("{} IS NOT NULL", v.as_str())),
            E::If(cond, then_expr, else_expr) => {
                let c = self.translate_expression(cond)?;
                let t = self.translate_expression(then_expr)?;
                let e = self.translate_expression(else_expr)?;
                match (c, t, e) {
                    (Some(c), Some(t), Some(e)) => {
                        Some(format!("CASE WHEN {} THEN {} ELSE {} END", c, t, e))
                    }
                    _ => None,
                }
            }
            E::Coalesce(exprs) => {
                let parts: Vec<_> = exprs
                    .iter()
                    .filter_map(|e| self.translate_expression(e).ok().flatten())
                    .collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(format!("coalesce({})", parts.join(", ")))
                }
            }
            E::FunctionCall(func, args) => {
                // Map common SPARQL functions to Cypher
                let arg_strs: Vec<_> = args
                    .iter()
                    .filter_map(|a| self.translate_expression(a).ok().flatten())
                    .collect();

                match func {
                    F::Str => arg_strs.first().map(|a| format!("toString({})", a)),
                    F::StrLen => arg_strs.first().map(|a| format!("size({})", a)),
                    F::UCase => arg_strs.first().map(|a| format!("toUpper({})", a)),
                    F::LCase => arg_strs.first().map(|a| format!("toLower({})", a)),
                    F::Contains => {
                        if arg_strs.len() >= 2 {
                            Some(format!("{} CONTAINS {}", arg_strs[0], arg_strs[1]))
                        } else {
                            None
                        }
                    }
                    F::StrStarts => {
                        if arg_strs.len() >= 2 {
                            Some(format!("{} STARTS WITH {}", arg_strs[0], arg_strs[1]))
                        } else {
                            None
                        }
                    }
                    F::StrEnds => {
                        if arg_strs.len() >= 2 {
                            Some(format!("{} ENDS WITH {}", arg_strs[0], arg_strs[1]))
                        } else {
                            None
                        }
                    }
                    F::Regex => {
                        if arg_strs.len() >= 2 {
                            Some(format!("{} =~ {}", arg_strs[0], arg_strs[1]))
                        } else {
                            None
                        }
                    }
                    F::Abs => arg_strs.first().map(|a| format!("abs({})", a)),
                    F::Ceil => arg_strs.first().map(|a| format!("ceil({})", a)),
                    F::Floor => arg_strs.first().map(|a| format!("floor({})", a)),
                    F::Round => arg_strs.first().map(|a| format!("round({})", a)),
                    _ => None, // Unsupported function
                }
            }
            E::In(expr, list) => {
                let e = self.translate_expression(expr)?;
                let items: Vec<_> = list
                    .iter()
                    .filter_map(|i| self.translate_expression(i).ok().flatten())
                    .collect();
                match e {
                    Some(e) if !items.is_empty() => {
                        Some(format!("{} IN [{}]", e, items.join(", ")))
                    }
                    _ => None,
                }
            }
            _ => None, // Unsupported expression (Add, Subtract, Multiply, Divide, Exists, etc.)
        };

        Ok(result)
    }

    /// Build RETURN clause for SELECT
    fn build_return_clause(&self, variables: &[Variable], distinct: bool) -> String {
        let vars: Vec<String> = variables
            .iter()
            .map(|v| {
                // Return node/relationship properties as structured data
                format!(
                    "CASE WHEN {} IS NOT NULL THEN {{ uri: {}.uri, value: {}.value }} ELSE null END AS {}",
                    v.name, v.name, v.name, v.name
                )
            })
            .collect();

        let distinct_str = if distinct { "DISTINCT " } else { "" };
        format!("\nRETURN {}{}", distinct_str, vars.join(", "))
    }

    /// Build ORDER BY clause
    fn build_order_clause(
        &self,
        order_by: &[falkorsemantic_parser::sparql::OrderCondition],
    ) -> String {
        let parts: Vec<String> = order_by
            .iter()
            .filter_map(|cond| {
                // Translate the expression to Cypher
                let expr_str = self
                    .translate_expression(cond.expression.inner())
                    .ok()
                    .flatten()?;
                let dir = if cond.descending { " DESC" } else { "" };
                Some(format!("{}{}", expr_str, dir))
            })
            .collect();

        if parts.is_empty() {
            String::new()
        } else {
            format!("\nORDER BY {}", parts.join(", "))
        }
    }

    /// Generate a unique variable name
    fn next_var(&self, prefix: &str) -> String {
        let count = *self.var_counter.borrow();
        *self.var_counter.borrow_mut() += 1;
        format!("{}_{}", prefix, count)
    }
}

/// A translated Cypher query
#[derive(Debug, Clone)]
pub struct CypherQuery {
    /// The Cypher query string
    pub query: String,
    /// Variables to extract from results
    pub variables: Vec<String>,
    /// Type of query
    pub query_type: CypherQueryType,
}

/// Type of Cypher query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CypherQueryType {
    /// SELECT query returning bindings
    Select,
    /// ASK query returning boolean
    Ask,
    /// CONSTRUCT query returning triples
    Construct,
}

#[cfg(test)]
mod tests {
    use super::*;
    use falkorsemantic_parser::sparql::SparqlParser;

    fn translate(sparql: &str) -> Result<CypherQuery, MapperError> {
        let parser = SparqlParser::new();
        let query = parser
            .parse(sparql)
            .map_err(|e| MapperError::MappingError(e.to_string()))?;
        let translator = SparqlToCypher::new();
        translator.translate(&query)
    }

    #[test]
    fn test_simple_select() {
        let result = translate("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("MATCH"));
        assert!(cypher.query.contains("RETURN"));
        assert_eq!(cypher.query_type, CypherQueryType::Select);
    }

    #[test]
    fn test_select_with_uri() {
        let result = translate(
            "SELECT ?o WHERE { <http://example.org/Alice> <http://example.org/knows> ?o }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("example.org/Alice"));
        assert!(cypher.query.contains("example.org/knows"));
    }

    #[test]
    fn test_select_distinct() {
        let result = translate("SELECT DISTINCT ?s WHERE { ?s ?p ?o }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("DISTINCT"));
    }

    #[test]
    fn test_select_with_limit() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o } LIMIT 10");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("LIMIT 10"));
    }

    #[test]
    fn test_select_with_offset() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o } OFFSET 5");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("SKIP 5"));
    }

    #[test]
    fn test_ask_query() {
        let result = translate("ASK { ?s ?p ?o }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("count(*)"));
        assert_eq!(cypher.query_type, CypherQueryType::Ask);
    }

    #[test]
    fn test_select_with_filter() {
        let result = translate(
            "SELECT ?s ?age WHERE { ?s <http://example.org/age> ?age . FILTER(?age > 18) }",
        );
        // Filter translation may not be fully supported yet
        // Using correct SPARQL syntax with FILTER inside WHERE clause
        assert!(result.is_ok(), "Translation failed: {:?}", result.err());
    }

    #[test]
    fn test_variables_extraction() {
        let result = translate("SELECT ?name ?age WHERE { ?s <http://example.org/name> ?name . ?s <http://example.org/age> ?age }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.variables.contains(&"name".to_string()));
        assert!(cypher.variables.contains(&"age".to_string()));
    }

    #[test]
    fn test_select_with_order_by() {
        let result = translate(
            "SELECT ?s ?name WHERE { ?s <http://example.org/name> ?name } ORDER BY ?name",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(
            cypher.query.contains("ORDER BY"),
            "Should contain ORDER BY clause"
        );
        assert!(
            cypher.query.contains("name"),
            "ORDER BY should reference the name variable"
        );
    }

    #[test]
    fn test_select_with_order_by_desc() {
        let result = translate(
            "SELECT ?s ?age WHERE { ?s <http://example.org/age> ?age } ORDER BY DESC(?age)",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(
            cypher.query.contains("ORDER BY"),
            "Should contain ORDER BY clause"
        );
        assert!(
            cypher.query.contains("DESC"),
            "Should contain DESC for descending order"
        );
    }

    #[test]
    fn test_union_returns_error() {
        let result = translate("SELECT ?s WHERE { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } }");
        assert!(result.is_err(), "UNION queries should return an error");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("UNION"),
            "Error should mention UNION"
        );
    }
}
