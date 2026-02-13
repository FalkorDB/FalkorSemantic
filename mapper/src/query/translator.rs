//! SPARQL to Cypher Translator
//!
//! Translates SPARQL queries to Cypher queries for execution against `FalkorDB`.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use falkorsemantic_parser::sparql::{GraphPattern, Query, SelectQuery, Variable};

use crate::graph::{escape_cypher_string, sanitize_identifier};
use crate::MapperError;

/// How a SPARQL variable is bound in the generated Cypher query
#[derive(Debug, Clone)]
enum VarBinding {
    /// Bound as a graph node (existing behavior)
    Node,
    /// Could be either node or property — needs OPTIONAL MATCH edge + property fallback
    Dual {
        subject_var: String,
        prop_key: String,
        edge_obj_var: String,
    },
}

/// Result of translating a single triple pattern
struct TripleResult {
    /// Pattern for the required MATCH clause (e.g., "(s)-[r]->(o)" or "(s)")
    required_match: Option<String>,
    /// Pattern for OPTIONAL MATCH (for dual binding patterns)
    optional_match: Option<String>,
    /// WHERE conditions for the required MATCH
    conditions: Vec<String>,
    /// WHERE condition for the OPTIONAL MATCH
    optional_condition: Option<String>,
    /// Variables bound by this triple
    bound_vars: HashSet<Variable>,
}

/// Extract a property key from a predicate IRI, mirroring how `generate_literal_edge()` stores literals
fn predicate_to_property_key(predicate_iri: &str) -> String {
    let local_name = if let Some(pos) = predicate_iri.rfind('#') {
        &predicate_iri[pos + 1..]
    } else if let Some(pos) = predicate_iri.rfind('/') {
        &predicate_iri[pos + 1..]
    } else {
        predicate_iri
    };
    sanitize_identifier(local_name)
}

/// Translates SPARQL queries to Cypher
#[derive(Debug, Default)]
pub struct SparqlToCypher {
    /// Variable counter for generating unique Cypher variables
    var_counter: std::cell::RefCell<usize>,
    /// Tracks how each SPARQL variable is bound in the generated Cypher
    var_bindings: std::cell::RefCell<HashMap<String, VarBinding>>,
}

impl SparqlToCypher {
    /// Create a new translator
    #[must_use]
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
        // Check if the top-level pattern is a UNION
        if let spargebra::algebra::GraphPattern::Union { .. } = select.pattern.inner() {
            return self.translate_select_union(select);
        }

        self.var_bindings.borrow_mut().clear();
        let mut cypher = String::new();

        // Translate the graph pattern to MATCH clause
        let (match_clause, where_clause, optional_clauses, bound_vars) =
            self.translate_graph_pattern(&select.pattern)?;

        cypher.push_str(&match_clause);

        if !where_clause.is_empty() {
            write!(cypher, "\nWHERE {where_clause}").unwrap();
        }

        cypher.push_str(&optional_clauses);

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
            write!(cypher, "\nLIMIT {limit}").unwrap();
        }
        if let Some(offset) = select.offset {
            write!(cypher, "\nSKIP {offset}").unwrap();
        }

        Ok(CypherQuery {
            query: cypher,
            variables: return_vars.iter().map(|v| v.name.clone()).collect(),
            query_type: CypherQueryType::Select,
        })
    }

    /// Translate a SELECT query whose top-level pattern is a UNION
    fn translate_select_union(&self, select: &SelectQuery) -> Result<CypherQuery, MapperError> {
        // Flatten nested UNIONs into a list of branches
        let mut branches = Vec::new();
        Self::flatten_union(select.pattern.inner(), &mut branches);

        // Single pass: process each branch once, collecting match/where parts and variables
        let mut processed_branches = Vec::with_capacity(branches.len());
        let mut all_vars = HashSet::new();
        for branch in &branches {
            self.var_bindings.borrow_mut().clear();
            let mut match_parts = Vec::new();
            let mut where_parts = Vec::new();
            let mut optional_parts = Vec::new();
            let mut bound_vars = HashSet::new();
            self.process_pattern(
                branch,
                &mut match_parts,
                &mut where_parts,
                &mut optional_parts,
                &mut bound_vars,
            )?;
            all_vars.extend(bound_vars);
            let branch_bindings = self.var_bindings.borrow().clone();
            processed_branches.push((match_parts, where_parts, optional_parts, branch_bindings));
        }

        // Determine the return variables
        let return_vars = if select.is_select_all() {
            let mut vars: Vec<_> = all_vars.into_iter().collect();
            vars.sort_by(|a, b| a.name.cmp(&b.name));
            vars
        } else {
            select.projected_variables()
        };

        // Build a complete Cypher query for each branch from the processed results
        let mut branch_queries = Vec::with_capacity(processed_branches.len());
        for (match_parts, where_parts, optional_parts, branch_bindings) in &processed_branches {
            // Restore bindings for this branch's RETURN clause
            *self.var_bindings.borrow_mut() = branch_bindings.clone();

            let mut cypher = String::new();

            let match_clause = if match_parts.is_empty() {
                "MATCH (n)".to_string()
            } else {
                format!("MATCH {}", match_parts.join(", "))
            };
            cypher.push_str(&match_clause);

            if !where_parts.is_empty() {
                write!(cypher, "\nWHERE {}", where_parts.join(" AND ")).unwrap();
            }

            for opt in optional_parts {
                cypher.push_str(opt);
            }

            let return_clause = self.build_return_clause(&return_vars, select.distinct);
            cypher.push_str(&return_clause);

            branch_queries.push(cypher);
        }

        // Use UNION (deduplicating) when DISTINCT, otherwise UNION ALL
        let union_separator = if select.distinct {
            "\nUNION\n"
        } else {
            "\nUNION ALL\n"
        };
        let union_body = branch_queries.join(union_separator);

        let has_modifiers =
            select.order_by.is_some() || select.limit.is_some() || select.offset.is_some();

        // Wrap in CALL { ... } subquery when ORDER BY/LIMIT/SKIP are needed,
        // since FalkorDB does not allow these directly after a UNION
        let mut cypher = if has_modifiers {
            let return_items: Vec<String> = return_vars.iter().map(|v| v.name.clone()).collect();
            let projection = if return_items.is_empty() {
                "*".to_string()
            } else {
                return_items.join(", ")
            };
            format!("CALL {{\n{union_body}\n}}\nRETURN {projection}",)
        } else {
            union_body
        };

        // ORDER BY, LIMIT, OFFSET apply to the entire UNION result
        if let Some(ref order_by) = select.order_by {
            let order_clause = self.build_order_clause(order_by);
            cypher.push_str(&order_clause);
        }
        if let Some(limit) = select.limit {
            write!(cypher, "\nLIMIT {limit}").unwrap();
        }
        if let Some(offset) = select.offset {
            write!(cypher, "\nSKIP {offset}").unwrap();
        }

        Ok(CypherQuery {
            query: cypher,
            variables: return_vars.iter().map(|v| v.name.clone()).collect(),
            query_type: CypherQueryType::Select,
        })
    }

    /// Flatten nested UNION patterns into a list of non-UNION branches
    fn flatten_union<'a>(
        pattern: &'a spargebra::algebra::GraphPattern,
        branches: &mut Vec<&'a spargebra::algebra::GraphPattern>,
    ) {
        use spargebra::algebra::GraphPattern as GP;
        if let GP::Union { left, right } = pattern {
            Self::flatten_union(left, branches);
            Self::flatten_union(right, branches);
        } else {
            branches.push(pattern);
        }
    }

    /// Translate an ASK query
    fn translate_ask(&self, pattern: &GraphPattern) -> Result<CypherQuery, MapperError> {
        self.var_bindings.borrow_mut().clear();
        let (match_clause, where_clause, optional_clauses, _) =
            self.translate_graph_pattern(pattern)?;

        let mut cypher = match_clause;
        if !where_clause.is_empty() {
            write!(cypher, "\nWHERE {where_clause}").unwrap();
        }
        cypher.push_str(&optional_clauses);
        cypher.push_str("\nRETURN count(*) > 0 AS result");

        Ok(CypherQuery {
            query: cypher,
            variables: vec!["result".to_string()],
            query_type: CypherQueryType::Ask,
        })
    }

    /// Translate a graph pattern to MATCH, WHERE, and OPTIONAL MATCH clauses
    fn translate_graph_pattern(
        &self,
        pattern: &GraphPattern,
    ) -> Result<(String, String, String, HashSet<Variable>), MapperError> {
        let mut match_parts = Vec::new();
        let mut where_parts = Vec::new();
        let mut optional_parts = Vec::new();
        let mut bound_vars = HashSet::new();

        self.process_pattern(
            pattern.inner(),
            &mut match_parts,
            &mut where_parts,
            &mut optional_parts,
            &mut bound_vars,
        )?;

        let match_clause = if match_parts.is_empty() {
            "MATCH (n)".to_string() // Fallback for empty patterns
        } else {
            format!("MATCH {}", match_parts.join(", "))
        };

        let where_clause = where_parts.join(" AND ");
        let optional_clauses = optional_parts.join("");

        Ok((match_clause, where_clause, optional_clauses, bound_vars))
    }

    /// Process a spargebra graph pattern recursively
    fn process_pattern(
        &self,
        pattern: &spargebra::algebra::GraphPattern,
        match_parts: &mut Vec<String>,
        where_parts: &mut Vec<String>,
        optional_parts: &mut Vec<String>,
        bound_vars: &mut HashSet<Variable>,
    ) -> Result<(), MapperError> {
        use spargebra::algebra::GraphPattern as GP;

        match pattern {
            GP::Bgp { patterns } => {
                for triple in patterns {
                    let result = self.translate_triple_pattern(triple)?;
                    if let Some(m) = result.required_match {
                        match_parts.push(m);
                    }
                    if let Some(opt) = result.optional_match {
                        let mut opt_clause = format!("\nOPTIONAL MATCH {opt}");
                        if let Some(cond) = result.optional_condition {
                            write!(opt_clause, "\nWHERE {cond}").unwrap();
                        }
                        optional_parts.push(opt_clause);
                    }
                    where_parts.extend(result.conditions);
                    bound_vars.extend(result.bound_vars);
                }
            }
            GP::Join { left, right } => {
                self.process_pattern(left, match_parts, where_parts, optional_parts, bound_vars)?;
                self.process_pattern(right, match_parts, where_parts, optional_parts, bound_vars)?;
            }
            GP::LeftJoin {
                left,
                right,
                expression,
            } => {
                self.process_pattern(left, match_parts, where_parts, optional_parts, bound_vars)?;
                // OPTIONAL becomes OPTIONAL MATCH in Cypher
                let mut opt_match = Vec::new();
                let mut opt_where = Vec::new();
                let mut opt_optional = Vec::new();
                self.process_pattern(
                    right,
                    &mut opt_match,
                    &mut opt_where,
                    &mut opt_optional,
                    bound_vars,
                )?;
                if !opt_match.is_empty() {
                    match_parts.push(format!("OPTIONAL MATCH {}", opt_match.join(", ")));
                }
                optional_parts.extend(opt_optional);
                // Handle the expression (filter on optional)
                if let Some(expr) = expression {
                    if let Some(filter) = self.translate_expression(expr)? {
                        where_parts.push(filter);
                    }
                }
            }
            GP::Filter { inner, expr } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
                if let Some(filter) = self.translate_expression(expr)? {
                    where_parts.push(filter);
                }
            }
            GP::Union { .. } => {
                // Top-level UNION is handled in translate_select_union().
                // Nested UNION (e.g. UNION inside a JOIN) is not yet supported
                // because Cypher UNION must appear at the top level.
                return Err(MapperError::MappingError(
                    "Nested SPARQL UNION patterns are not yet supported. \
                     UNION must be the top-level graph pattern in the query."
                        .into(),
                ));
            }
            GP::Extend {
                inner,
                variable,
                expression: _,
            } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
                bound_vars.insert(Variable::from(variable.clone()));
                // BIND becomes WITH ... AS in Cypher
            }
            GP::OrderBy { inner, .. } | GP::Distinct { inner } | GP::Reduced { inner } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
            }
            GP::Project { inner, variables } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
                for v in variables {
                    bound_vars.insert(Variable::from(v.clone()));
                }
            }
            GP::Slice { inner, .. } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
            }
            GP::Graph { inner, .. } => {
                // Named graph - translate inner pattern
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
            }
            GP::Group { inner, .. } => {
                self.process_pattern(inner, match_parts, where_parts, optional_parts, bound_vars)?;
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
                let path_pattern = format!("({subj_var})-[*]->({obj_var})");
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
                self.process_pattern(left, match_parts, where_parts, optional_parts, bound_vars)?;
                let mut minus_match = Vec::new();
                let mut minus_where = Vec::new();
                let mut minus_optional = Vec::new();
                let mut minus_vars = HashSet::new();
                self.process_pattern(
                    right,
                    &mut minus_match,
                    &mut minus_where,
                    &mut minus_optional,
                    &mut minus_vars,
                )?;
                if !minus_match.is_empty() {
                    where_parts.push(format!("NOT EXISTS {{ MATCH {} }}", minus_match.join(", ")));
                }
            }
        }

        Ok(())
    }

    /// Translate a triple pattern to Cypher MATCH pattern
    ///
    /// Dispatches based on object type:
    /// - Literal constant + known predicate → property WHERE (no edge)
    /// - Variable object + known predicate → dual pattern (OPTIONAL MATCH edge + property fallback)
    /// - Named node / blank node / variable predicate → edge pattern (existing behavior)
    fn translate_triple_pattern(
        &self,
        triple: &spargebra::term::TriplePattern,
    ) -> Result<TripleResult, MapperError> {
        use spargebra::term::NamedNodePattern as NNP;
        use spargebra::term::TermPattern as TP;

        let mut conditions = Vec::new();
        let mut bound_vars = HashSet::new();

        // Translate subject
        let (subj_var, subj_cond) = self.term_to_cypher(&triple.subject)?;
        if let Some(cond) = subj_cond {
            conditions.push(cond);
        }
        self.add_term_variable(&triple.subject, &mut bound_vars);

        // Register subject as Node binding (don't overwrite existing bindings)
        if let TP::Variable(v) = &triple.subject {
            self.var_bindings
                .borrow_mut()
                .entry(v.as_str().to_string())
                .or_insert(VarBinding::Node);
        }

        // Get predicate IRI if known
        let predicate_iri = match &triple.predicate {
            NNP::NamedNode(n) => Some(n.as_str().to_string()),
            NNP::Variable(_) => None,
        };

        match (&triple.object, &predicate_iri) {
            // Case 1: Literal constant object with known predicate → property access
            (TP::Literal(lit), Some(pred_iri)) => {
                let prop_key = predicate_to_property_key(pred_iri);
                let value = escape_cypher_string(lit.value());
                conditions.push(format!("{subj_var}.{prop_key} = '{value}'"));

                Ok(TripleResult {
                    required_match: Some(format!("({subj_var})")),
                    optional_match: None,
                    conditions,
                    optional_condition: None,
                    bound_vars,
                })
            }

            // Case 2: Variable object with known predicate → dual pattern
            (TP::Variable(v), Some(pred_iri)) => {
                let prop_key = predicate_to_property_key(pred_iri);
                let pred_str = format!("{{predicate: '{}'}}", escape_cypher_string(pred_iri));
                let rel_var = self.next_var("r");
                let edge_obj_var = format!("{}_edge", v.as_str());

                // Register dual binding
                self.var_bindings.borrow_mut().insert(
                    v.as_str().to_string(),
                    VarBinding::Dual {
                        subject_var: subj_var.clone(),
                        prop_key: prop_key.clone(),
                        edge_obj_var: edge_obj_var.clone(),
                    },
                );

                let optional = format!("({subj_var})-[{rel_var}{pred_str}]->({edge_obj_var})");
                let opt_cond =
                    format!("({edge_obj_var} IS NOT NULL OR {subj_var}.{prop_key} IS NOT NULL)");

                bound_vars.insert(Variable::from(v.clone()));

                Ok(TripleResult {
                    required_match: Some(format!("({subj_var})")),
                    optional_match: Some(optional),
                    conditions,
                    optional_condition: Some(opt_cond),
                    bound_vars,
                })
            }

            // Case 3: All other cases → edge pattern (existing behavior)
            // Named node / blank node objects, or any object with variable predicate
            _ => {
                let pred_str = self.predicate_to_cypher(&triple.predicate)?;
                self.add_predicate_variable(&triple.predicate, &mut bound_vars);

                let (obj_var, obj_cond) = self.term_to_cypher(&triple.object)?;
                if let Some(cond) = obj_cond {
                    conditions.push(cond);
                }
                self.add_term_variable(&triple.object, &mut bound_vars);

                // Register node binding for variable objects
                if let TP::Variable(v) = &triple.object {
                    self.var_bindings
                        .borrow_mut()
                        .entry(v.as_str().to_string())
                        .or_insert(VarBinding::Node);
                }

                let rel_var = self.next_var("r");
                let pattern = format!("({subj_var})-[{rel_var}{pred_str}]->({obj_var})");

                Ok(TripleResult {
                    required_match: Some(pattern),
                    optional_match: None,
                    conditions,
                    optional_condition: None,
                    bound_vars,
                })
            }
        }
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
                let condition = format!("{var}.value = '{value}'");
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
                    (Some(l), Some(r)) => Some(format!("{l} = {r}")),
                    _ => None,
                }
            }
            E::SameTerm(left, right) => {
                // sameTerm is strict equality
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{l} = {r}")),
                    _ => None,
                }
            }
            E::Less(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{l} < {r}")),
                    _ => None,
                }
            }
            E::Greater(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{l} > {r}")),
                    _ => None,
                }
            }
            E::LessOrEqual(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{l} <= {r}")),
                    _ => None,
                }
            }
            E::GreaterOrEqual(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("{l} >= {r}")),
                    _ => None,
                }
            }
            E::And(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("({l} AND {r})")),
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    _ => None,
                }
            }
            E::Or(left, right) => {
                let l = self.translate_expression(left)?;
                let r = self.translate_expression(right)?;
                match (l, r) {
                    (Some(l), Some(r)) => Some(format!("({l} OR {r})")),
                    _ => None,
                }
            }
            E::Not(inner) => {
                let i = self.translate_expression(inner)?;
                i.map(|i| format!("NOT ({i})"))
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
                        Some(format!("CASE WHEN {c} THEN {t} ELSE {e} END"))
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
                    F::Str => arg_strs.first().map(|a| format!("toString({a})")),
                    F::StrLen => arg_strs.first().map(|a| format!("size({a})")),
                    F::UCase => arg_strs.first().map(|a| format!("toUpper({a})")),
                    F::LCase => arg_strs.first().map(|a| format!("toLower({a})")),
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
                    F::Abs => arg_strs.first().map(|a| format!("abs({a})")),
                    F::Ceil => arg_strs.first().map(|a| format!("ceil({a})")),
                    F::Floor => arg_strs.first().map(|a| format!("floor({a})")),
                    F::Round => arg_strs.first().map(|a| format!("round({a})")),
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
    ///
    /// Uses `var_bindings` to determine how to project each variable:
    /// - Node: `{uri: v.uri, value: v.value}`
    /// - Dual: prioritize edge node, fall back to property
    fn build_return_clause(&self, variables: &[Variable], distinct: bool) -> String {
        let bindings = self.var_bindings.borrow();
        let vars: Vec<String> = variables
            .iter()
            .map(|v| {
                match bindings.get(&v.name) {
                    Some(VarBinding::Dual {
                        subject_var,
                        prop_key,
                        edge_obj_var,
                    }) => {
                        format!(
                            "CASE WHEN {edge_obj_var} IS NOT NULL THEN \
                             {{ uri: {edge_obj_var}.uri, value: {edge_obj_var}.value }} \
                             WHEN {subject_var}.{prop_key} IS NOT NULL THEN \
                             {{ value: toString({subject_var}.{prop_key}) }} \
                             ELSE null END AS {}",
                            v.name
                        )
                    }
                    _ => {
                        // Node binding or unknown — default behavior
                        format!(
                            "CASE WHEN {} IS NOT NULL THEN {{ uri: {}.uri, value: {}.value }} ELSE null END AS {}",
                            v.name, v.name, v.name, v.name
                        )
                    }
                }
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
                Some(format!("{expr_str}{dir}"))
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
        format!("{prefix}_{count}")
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
    fn test_union_two_branches() {
        let result = translate(
            "SELECT ?s WHERE { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } }",
        );
        assert!(
            result.is_ok(),
            "UNION translation failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        assert!(
            cypher.query.contains("UNION ALL"),
            "Should contain UNION ALL, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("example.org/a"),
            "Should contain first branch predicate"
        );
        assert!(
            cypher.query.contains("example.org/b"),
            "Should contain second branch predicate"
        );
        assert_eq!(cypher.query_type, CypherQueryType::Select);
    }

    #[test]
    fn test_union_both_branches_have_match_and_return() {
        let result = translate(
            "SELECT ?s ?o WHERE { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } }",
        );
        assert!(
            result.is_ok(),
            "UNION translation failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        let parts: Vec<&str> = cypher.query.split("UNION ALL").collect();
        assert_eq!(parts.len(), 2, "Should have exactly 2 branches");
        for (i, part) in parts.iter().enumerate() {
            assert!(
                part.contains("MATCH"),
                "Branch {i} should contain MATCH, got: {part}"
            );
            assert!(
                part.contains("RETURN"),
                "Branch {i} should contain RETURN, got: {part}"
            );
        }
    }

    #[test]
    fn test_union_with_three_branches() {
        let result = translate(
            "SELECT ?s WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
                UNION { ?s <http://example.org/c> ?o } \
            }",
        );
        assert!(result.is_ok(), "Triple UNION failed: {:?}", result.err());
        let cypher = result.unwrap();
        let parts: Vec<&str> = cypher.query.split("UNION ALL").collect();
        assert_eq!(
            parts.len(),
            3,
            "Should have 3 branches, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_with_filter() {
        let result = translate(
            "SELECT ?s ?o WHERE { \
                { ?s <http://example.org/a> ?o . FILTER(?o > 10) } \
                UNION \
                { ?s <http://example.org/b> ?o } \
            }",
        );
        assert!(
            result.is_ok(),
            "UNION with FILTER failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        assert!(cypher.query.contains("WHERE"));
    }

    #[test]
    fn test_union_with_limit_offset() {
        let result = translate(
            "SELECT ?s WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
            } LIMIT 10 OFFSET 5",
        );
        assert!(
            result.is_ok(),
            "UNION with LIMIT/OFFSET failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        assert!(cypher.query.contains("LIMIT 10"));
        assert!(cypher.query.contains("SKIP 5"));
        // Should be wrapped in CALL { ... } since modifiers are present
        assert!(
            cypher.query.contains("CALL {"),
            "UNION with LIMIT/OFFSET should be wrapped in CALL block, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_variables() {
        let result = translate(
            "SELECT ?s ?o WHERE { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.variables.contains(&"s".to_string()));
        assert!(cypher.variables.contains(&"o".to_string()));
    }

    #[test]
    fn test_union_select_star() {
        let result = translate(
            "SELECT * WHERE { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } }",
        );
        assert!(
            result.is_ok(),
            "UNION with SELECT * failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        // SELECT * should collect variables from all branches
        assert!(cypher.variables.contains(&"s".to_string()));
        assert!(cypher.variables.contains(&"o".to_string()));
    }

    #[test]
    fn test_union_with_order_by() {
        let result = translate(
            "SELECT ?s ?o WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
            } ORDER BY ?s",
        );
        assert!(
            result.is_ok(),
            "UNION with ORDER BY failed: {:?}",
            result.err()
        );
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        assert!(
            cypher.query.contains("ORDER BY"),
            "Should contain ORDER BY clause, got: {}",
            cypher.query
        );
        // Should be wrapped in CALL { ... } since ORDER BY is present
        assert!(
            cypher.query.contains("CALL {"),
            "UNION with ORDER BY should be wrapped in CALL block, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_with_uri_in_branch() {
        let result = translate(
            "SELECT ?s WHERE { \
                { ?s <http://example.org/type> <http://example.org/Cat> } \
                UNION \
                { ?s <http://example.org/type> <http://example.org/Dog> } \
            }",
        );
        assert!(result.is_ok(), "UNION with URIs failed: {:?}", result.err());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        // URI objects generate WHERE conditions
        let parts: Vec<&str> = cypher.query.split("UNION ALL").collect();
        assert_eq!(parts.len(), 2);
        assert!(
            parts[0].contains("WHERE"),
            "First branch should have WHERE for URI condition"
        );
        assert!(
            parts[1].contains("WHERE"),
            "Second branch should have WHERE for URI condition"
        );
    }

    #[test]
    fn test_union_distinct_uses_union_separator() {
        let result = translate(
            "SELECT DISTINCT ?s WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
            }",
        );
        assert!(result.is_ok(), "DISTINCT UNION failed: {:?}", result.err());
        let cypher = result.unwrap();
        // DISTINCT should use UNION (not UNION ALL) and RETURN DISTINCT
        assert!(
            !cypher.query.contains("UNION ALL"),
            "DISTINCT UNION should not use UNION ALL, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("\nUNION\n"),
            "DISTINCT UNION should use UNION separator, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("RETURN DISTINCT"),
            "DISTINCT UNION should use RETURN DISTINCT, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_non_distinct_uses_union_all() {
        let result = translate(
            "SELECT ?s WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
            }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(
            cypher.query.contains("UNION ALL"),
            "Non-distinct UNION should use UNION ALL, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_with_limit_wrapped_in_call() {
        let result = translate(
            "SELECT ?s ?o WHERE { \
                { ?s <http://example.org/a> ?o } \
                UNION { ?s <http://example.org/b> ?o } \
            } LIMIT 5",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        // Should be: CALL { <union> } RETURN ... LIMIT 5
        assert!(
            cypher.query.starts_with("CALL {"),
            "Should start with CALL block, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("LIMIT 5"),
            "Should have LIMIT, got: {}",
            cypher.query
        );
        // The RETURN after CALL should project the variables
        let after_call_close = cypher.query.split("}\n").last().unwrap();
        assert!(
            after_call_close.contains("RETURN"),
            "Should have RETURN after CALL block, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_nested_union_returns_error() {
        let result = translate(
            "SELECT ?s ?name WHERE { \
                ?s <http://example.org/name> ?name . \
                { { ?s <http://example.org/a> ?o } UNION { ?s <http://example.org/b> ?o } } \
            }",
        );
        assert!(result.is_err(), "Nested UNION should return an error");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("UNION") || err.to_string().contains("Nested"),
            "Error should mention UNION or Nested"
        );
    }

    // --- Literal property pattern tests ---

    #[test]
    fn test_literal_constant_object() {
        // ?s foaf:name "Alice" → property WHERE, no edge
        let result = translate("SELECT ?s WHERE { ?s <http://xmlns.com/foaf/0.1/name> \"Alice\" }");
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        // Should use property access, not edge pattern
        assert!(
            cypher.query.contains("s.name = 'Alice'"),
            "Should contain property access s.name = 'Alice', got: {}",
            cypher.query
        );
        // Should NOT have a relationship pattern for this predicate
        assert!(
            !cypher
                .query
                .contains("predicate: 'http://xmlns.com/foaf/0.1/name'"),
            "Should not use edge pattern for literal object, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_variable_object_dual_pattern() {
        // ?s foaf:name ?name → OPTIONAL MATCH + property fallback
        let result =
            translate("SELECT ?s ?name WHERE { ?s <http://xmlns.com/foaf/0.1/name> ?name }");
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        // Should have OPTIONAL MATCH for the edge pattern
        assert!(
            cypher.query.contains("OPTIONAL MATCH"),
            "Should contain OPTIONAL MATCH for dual pattern, got: {}",
            cypher.query
        );
        // Should reference the edge object variable
        assert!(
            cypher.query.contains("name_edge"),
            "Should contain name_edge variable, got: {}",
            cypher.query
        );
        // Should have property fallback in WHERE
        assert!(
            cypher.query.contains("s.name IS NOT NULL"),
            "Should have property fallback, got: {}",
            cypher.query
        );
        // RETURN should have dual CASE expression
        assert!(
            cypher.query.contains("name_edge IS NOT NULL"),
            "RETURN should check edge var first, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("toString(s.name)"),
            "RETURN should have property fallback with toString, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_iri_object_edge_only() {
        // ?s ex:knows <http://ex.org/Bob> → edge only (unchanged)
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/knows> <http://example.org/Bob> }");
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        // Should use edge pattern
        assert!(
            cypher
                .query
                .contains("predicate: 'http://example.org/knows'"),
            "Should have edge pattern with predicate, got: {}",
            cypher.query
        );
        // Should have URI condition for the object
        assert!(
            cypher.query.contains("example.org/Bob"),
            "Should reference Bob URI, got: {}",
            cypher.query
        );
        // Should NOT have OPTIONAL MATCH
        assert!(
            !cypher.query.contains("OPTIONAL MATCH"),
            "Should not have OPTIONAL MATCH for IRI object, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_mixed_literal_and_edge() {
        // Query with both literal predicate and IRI predicate
        // Both use dual patterns since we can't know at translation time whether objects are literals or IRIs
        let result = translate(
            "SELECT ?s ?name ?friend WHERE { \
                ?s <http://xmlns.com/foaf/0.1/name> ?name . \
                ?s <http://example.org/knows> ?friend \
            }",
        );
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        // Both should have OPTIONAL MATCH (dual patterns)
        assert!(
            cypher.query.contains("name_edge"),
            "Should have name_edge variable, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("friend_edge"),
            "Should have friend_edge variable, got: {}",
            cypher.query
        );
        // Both should have property fallbacks in RETURN
        assert!(
            cypher.query.contains("toString(s.name)"),
            "Should have name property fallback, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("toString(s.knows)"),
            "Should have knows property fallback, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_variable_predicate_edge_only() {
        // ?s ?p ?o → edge only (no property key derivable)
        let result = translate("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        // Should use edge pattern (no predicate constraint since ?p is variable)
        assert!(
            cypher.query.contains("MATCH (s)-["),
            "Should have edge pattern, got: {}",
            cypher.query
        );
        // Should NOT have OPTIONAL MATCH
        assert!(
            !cypher.query.contains("OPTIONAL MATCH"),
            "Should not have OPTIONAL MATCH for variable predicate, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_union_with_literal_object() {
        let result = translate(
            "SELECT ?s WHERE { \
                { ?s <http://xmlns.com/foaf/0.1/name> \"Alice\" } \
                UNION \
                { ?s <http://xmlns.com/foaf/0.1/name> \"Bob\" } \
            }",
        );
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("UNION ALL"));
        // Both branches should use property access
        assert!(
            cypher.query.contains("s.name = 'Alice'"),
            "Should have property access for Alice, got: {}",
            cypher.query
        );
        assert!(
            cypher.query.contains("s.name = 'Bob'"),
            "Should have property access for Bob, got: {}",
            cypher.query
        );
    }

    #[test]
    fn test_predicate_to_property_key_extraction() {
        // Test the property key extraction logic
        assert_eq!(
            predicate_to_property_key("http://xmlns.com/foaf/0.1/name"),
            "name"
        );
        // '#' delimiter: "type" starts with a letter, so no underscore prefix
        assert_eq!(
            predicate_to_property_key("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            "type"
        );
        assert_eq!(
            predicate_to_property_key("http://example.org/knows"),
            "knows"
        );
        // Numeric start gets underscore prefix
        assert_eq!(predicate_to_property_key("http://example.org/123"), "_123");
    }

    // --- Tests from main ---

    #[test]
    fn test_construct_returns_error() {
        let result = translate("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
        assert!(result.is_err(), "CONSTRUCT queries should return an error");
        assert!(result.unwrap_err().to_string().contains("CONSTRUCT"));
    }

    #[test]
    fn test_describe_returns_error() {
        let result = translate("DESCRIBE <http://example.org/resource>");
        assert!(result.is_err(), "DESCRIBE queries should return an error");
        assert!(result.unwrap_err().to_string().contains("DESCRIBE"));
    }

    #[test]
    fn test_select_star() {
        let result = translate("SELECT * WHERE { ?s ?p ?o }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("RETURN"));
    }

    #[test]
    fn test_optional_pattern() {
        let result = translate(
            "SELECT ?s ?r WHERE { ?s ?p ?o . OPTIONAL { ?s <http://example.org/q> ?r } }",
        );
        assert!(result.is_ok(), "OPTIONAL should translate successfully");
        let cypher = result.unwrap();
        assert!(cypher.query.contains("OPTIONAL MATCH"));
    }

    #[test]
    fn test_optional_with_filter_expression() {
        let result = translate(
            "SELECT ?s ?r WHERE { ?s <http://example.org/age> ?age . \
             OPTIONAL { ?s <http://example.org/q> ?r . FILTER(?r > 0) } }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_and_expression() {
        let result = translate(
            "SELECT ?s WHERE { ?s <http://example.org/x> ?x . FILTER(?x > 1 && ?x < 100) }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("AND"));
    }

    #[test]
    fn test_filter_or_expression() {
        let result = translate(
            "SELECT ?s WHERE { ?s <http://example.org/x> ?x . FILTER(?x < 1 || ?x > 100) }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("OR"));
    }

    #[test]
    fn test_filter_not_expression() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o . FILTER(!BOUND(?s)) }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("NOT"));
    }

    #[test]
    fn test_filter_bound_expression() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o . FILTER(BOUND(?s)) }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("IS NOT NULL"));
    }

    #[test]
    fn test_filter_same_term() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o . FILTER(sameTerm(?s, ?o)) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_less() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/age> ?age . FILTER(?age < 18) }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains('<'));
    }

    #[test]
    fn test_filter_less_or_equal() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/age> ?age . FILTER(?age <= 18) }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("<="));
    }

    #[test]
    fn test_filter_greater_or_equal() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/age> ?age . FILTER(?age >= 18) }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains(">="));
    }

    #[test]
    fn test_filter_named_node() {
        let result =
            translate("SELECT ?s WHERE { ?s ?p ?o . FILTER(?p = <http://example.org/type>) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_str_function() {
        let result = translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(str(?s) = "test") }"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_strlen_function() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o . FILTER(strlen(str(?s)) > 5) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_ucase_function() {
        let result = translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(ucase(str(?s)) = "TEST") }"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_lcase_function() {
        let result = translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(lcase(str(?s)) = "test") }"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_contains_function() {
        let result =
            translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(contains(str(?s), "example")) }"#);
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.to_uppercase().contains("CONTAINS"));
    }

    #[test]
    fn test_filter_strstarts_function() {
        let result =
            translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(strstarts(str(?s), "http")) }"#);
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("STARTS WITH"));
    }

    #[test]
    fn test_filter_strends_function() {
        let result =
            translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(strends(str(?s), "suffix")) }"#);
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("ENDS WITH"));
    }

    #[test]
    fn test_filter_regex_function() {
        let result = translate(r#"SELECT ?s WHERE { ?s ?p ?o . FILTER(regex(str(?s), ".*")) }"#);
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("=~"));
    }

    #[test]
    fn test_filter_abs_function() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/n> ?n . FILTER(abs(?n) > 0) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_ceil_function() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/n> ?n . FILTER(ceil(?n) > 0) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_floor_function() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/n> ?n . FILTER(floor(?n) > 0) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_round_function() {
        let result =
            translate("SELECT ?s WHERE { ?s <http://example.org/n> ?n . FILTER(round(?n) > 0) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_in_expression() {
        let result = translate(
            "SELECT ?s WHERE { ?s <http://example.org/type> ?t . \
             FILTER(?t IN (<http://example.org/Person>, <http://example.org/Animal>)) }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("IN"));
    }

    #[test]
    fn test_filter_if_expression() {
        let result = translate(
            "SELECT ?s WHERE { ?s <http://example.org/x> ?x . \
             FILTER(IF(BOUND(?x), ?x, 0) > 0) }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_filter_coalesce_expression() {
        let result = translate(
            "SELECT ?s WHERE { ?s <http://example.org/x> ?x . \
             FILTER(COALESCE(?x, 0) > 0) }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_minus_pattern() {
        let result = translate(
            "SELECT ?s WHERE { ?s ?p ?o . \
             MINUS { ?s <http://example.org/exclude> ?x } }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("NOT EXISTS"));
    }

    #[test]
    fn test_named_graph_pattern() {
        let result = translate("SELECT ?s WHERE { GRAPH <http://example.org/graph> { ?s ?p ?o } }");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("MATCH"));
    }

    #[test]
    fn test_values_pattern() {
        let result = translate(
            "SELECT ?s WHERE { \
             VALUES ?s { <http://example.org/a> <http://example.org/b> } \
             ?s ?p ?o }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_service_returns_error() {
        let result = translate(
            "SELECT ?s WHERE { \
             SERVICE <http://endpoint.example.org/sparql> { ?s ?p ?o } }",
        );
        assert!(result.is_err(), "SERVICE clause should return an error");
        assert!(result.unwrap_err().to_string().contains("SERVICE"));
    }

    #[test]
    fn test_bind_extend_pattern() {
        let result = translate(
            "SELECT ?s ?upper WHERE { \
             ?s <http://example.org/name> ?name . \
             BIND(ucase(?name) AS ?upper) }",
        );
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.variables.contains(&"upper".to_string()));
    }

    #[test]
    fn test_select_with_limit_and_offset() {
        let result = translate("SELECT ?s WHERE { ?s ?p ?o } LIMIT 10 OFFSET 5");
        assert!(result.is_ok());
        let cypher = result.unwrap();
        assert!(cypher.query.contains("LIMIT 10"));
        assert!(cypher.query.contains("SKIP 5"));
    }
}
