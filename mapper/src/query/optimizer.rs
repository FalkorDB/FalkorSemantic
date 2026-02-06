//! BGP Join Order Optimizer
//!
//! Optimizes the order of triple patterns in a Basic Graph Pattern (BGP)
//! to minimize query execution cost.

use std::collections::HashSet;

use falkorsemantic_parser::sparql::{TermPattern, TriplePattern, Variable};
use falkorsemantic_storage::{IndexHint, IndexManager, Statistics};

/// Cost model for query optimization
#[derive(Debug, Clone)]
pub struct CostModel {
    /// Base cost for a full scan
    pub full_scan_cost: f64,
    /// Cost multiplier for index access
    pub index_access_cost: f64,
    /// Cost multiplier for hash join
    pub hash_join_cost: f64,
    /// Cost multiplier for nested loop join
    pub nested_loop_cost: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            full_scan_cost: 1.0,
            index_access_cost: 0.1,
            hash_join_cost: 0.5,
            nested_loop_cost: 2.0,
        }
    }
}

/// Cost estimate for a triple pattern
#[derive(Debug, Clone)]
pub struct TriplePatternCost {
    /// The triple pattern
    pub pattern: TriplePattern,
    /// Estimated cardinality (number of matching triples)
    pub cardinality: f64,
    /// Selectivity (fraction of total triples)
    pub selectivity: f64,
    /// Index hint for execution
    pub index_hint: IndexHint,
    /// Variables bound by this pattern
    pub bound_variables: HashSet<Variable>,
}

impl TriplePatternCost {
    /// Get estimated execution cost
    #[must_use]
    pub fn cost(&self, cost_model: &CostModel) -> f64 {
        let base = self.cardinality;
        match &self.index_hint {
            IndexHint::FullScan => base * cost_model.full_scan_cost,
            IndexHint::UseTypeIndex { .. } => base * cost_model.index_access_cost * 0.5,
            IndexHint::UsePredicateIndex { selectivity, .. } => {
                base * cost_model.index_access_cost * selectivity
            }
            IndexHint::UseNamespaceIndex { .. } => base * cost_model.index_access_cost,
        }
    }
}

/// An optimized query plan
#[derive(Debug, Clone)]
pub struct OptimizedPlan {
    /// Ordered triple patterns (optimized join order)
    pub patterns: Vec<TriplePatternCost>,
    /// Total estimated cost
    pub total_cost: f64,
    /// Variables bound at each step
    pub bound_at_step: Vec<HashSet<Variable>>,
}

impl OptimizedPlan {
    /// Get the execution order of patterns
    pub fn execution_order(&self) -> impl Iterator<Item = &TriplePattern> {
        self.patterns.iter().map(|p| &p.pattern)
    }

    /// Check if plan uses any indexes
    #[must_use]
    pub fn uses_indexes(&self) -> bool {
        self.patterns.iter().any(|p| p.index_hint.uses_index())
    }
}

/// Optimizer for BGP join ordering
#[derive(Debug)]
pub struct JoinOrderOptimizer {
    /// Cost model
    cost_model: CostModel,
    /// Total triple count (for selectivity estimation)
    total_triples: u64,
}

impl JoinOrderOptimizer {
    /// Create a new optimizer
    #[must_use]
    pub fn new() -> Self {
        Self {
            cost_model: CostModel::default(),
            total_triples: 0,
        }
    }

    /// Create optimizer with custom cost model
    #[must_use]
    pub const fn with_cost_model(cost_model: CostModel) -> Self {
        Self {
            cost_model,
            total_triples: 0,
        }
    }

    /// Set the total triple count for selectivity estimation
    pub fn set_total_triples(&mut self, count: u64) {
        self.total_triples = count;
    }

    /// Optimize a list of triple patterns using statistics
    pub fn optimize_with_statistics(
        &self,
        patterns: &[TriplePattern],
        stats: &Statistics,
        indexes: Option<&IndexManager>,
    ) -> OptimizedPlan {
        if patterns.is_empty() {
            return OptimizedPlan {
                patterns: vec![],
                total_cost: 0.0,
                bound_at_step: vec![],
            };
        }

        // Calculate costs for each pattern
        let mut pattern_costs: Vec<TriplePatternCost> = patterns
            .iter()
            .map(|p| self.estimate_pattern_cost(p, stats, indexes))
            .collect();

        // Greedy optimization: pick lowest cost pattern that shares variables
        let mut ordered = Vec::with_capacity(patterns.len());
        let mut bound_vars: HashSet<Variable> = HashSet::new();
        let mut bound_at_step = Vec::new();

        while !pattern_costs.is_empty() {
            // Find best next pattern
            let best_idx = self.find_best_next_pattern(&pattern_costs, &bound_vars);
            let best = pattern_costs.remove(best_idx);

            // Update bound variables
            bound_vars.extend(best.bound_variables.clone());
            bound_at_step.push(bound_vars.clone());

            ordered.push(best);
        }

        // Calculate total cost
        let total_cost = self.calculate_plan_cost(&ordered);

        OptimizedPlan {
            patterns: ordered,
            total_cost,
            bound_at_step,
        }
    }

    /// Optimize patterns without statistics (heuristic-based)
    #[must_use]
    pub fn optimize_heuristic(&self, patterns: &[TriplePattern]) -> OptimizedPlan {
        if patterns.is_empty() {
            return OptimizedPlan {
                patterns: vec![],
                total_cost: 0.0,
                bound_at_step: vec![],
            };
        }

        let mut pattern_costs: Vec<TriplePatternCost> = patterns
            .iter()
            .map(|p| self.estimate_pattern_cost_heuristic(p))
            .collect();

        // Sort by selectivity heuristic
        pattern_costs.sort_by(|a, b| a.selectivity.partial_cmp(&b.selectivity).unwrap());

        let mut bound_vars: HashSet<Variable> = HashSet::new();
        let mut bound_at_step = Vec::new();

        for cost in &pattern_costs {
            bound_vars.extend(cost.bound_variables.clone());
            bound_at_step.push(bound_vars.clone());
        }

        let total_cost = self.calculate_plan_cost(&pattern_costs);

        OptimizedPlan {
            patterns: pattern_costs,
            total_cost,
            bound_at_step,
        }
    }

    fn estimate_pattern_cost(
        &self,
        pattern: &TriplePattern,
        stats: &Statistics,
        indexes: Option<&IndexManager>,
    ) -> TriplePatternCost {
        let bound_variables = pattern.variables();
        let total = stats.triple_count().max(1) as f64;

        // Check for rdf:type pattern
        let is_type_pattern = matches!(&pattern.predicate, TermPattern::NamedNode(n)
            if n.iri == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type");

        // Get predicate IRI ID if known
        let predicate_id = if let TermPattern::NamedNode(n) = &pattern.predicate {
            // Would need IRI dictionary lookup - use hash for now
            Some(Self::hash_iri(&n.iri))
        } else {
            None
        };

        // Get type IRI ID if this is a type pattern with bound object
        let type_id = if is_type_pattern {
            if let TermPattern::NamedNode(n) = &pattern.object {
                Some(Self::hash_iri(&n.iri))
            } else {
                None
            }
        } else {
            None
        };

        // Get index hint
        let index_hint = if let Some(idx) = indexes {
            idx.get_hint(predicate_id, is_type_pattern, type_id)
        } else if let Some(tid) = type_id.filter(|_| is_type_pattern) {
            IndexHint::UseTypeIndex { type_id: tid }
        } else if let Some(pid) = predicate_id {
            let selectivity = stats.predicate_selectivity(pid);
            IndexHint::UsePredicateIndex {
                predicate_id: pid,
                selectivity,
            }
        } else {
            IndexHint::FullScan
        };

        // Estimate cardinality
        let selectivity = self.estimate_selectivity(pattern, stats);
        let cardinality = total * selectivity;

        TriplePatternCost {
            pattern: pattern.clone(),
            cardinality,
            selectivity,
            index_hint,
            bound_variables,
        }
    }

    fn estimate_pattern_cost_heuristic(&self, pattern: &TriplePattern) -> TriplePatternCost {
        let bound_variables = pattern.variables();

        // Heuristic: count bound terms
        let bound_count = [&pattern.subject, &pattern.predicate, &pattern.object]
            .iter()
            .filter(|t| !t.is_variable())
            .count();

        // More bound terms = more selective
        let selectivity = match bound_count {
            3 => 0.001, // All bound - very selective
            2 => 0.01,  // Two bound
            1 => 0.1,   // One bound
            _ => 1.0,   // All variables - full scan
        };

        // Check for rdf:type pattern (typically selective)
        let is_type_pattern = matches!(&pattern.predicate, TermPattern::NamedNode(n)
            if n.iri == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type");

        let selectivity = if is_type_pattern {
            selectivity * 0.5 // Type patterns often more selective
        } else {
            selectivity
        };

        let cardinality = self.total_triples.max(1000) as f64 * selectivity;

        let index_hint = if is_type_pattern {
            if let TermPattern::NamedNode(n) = &pattern.object {
                IndexHint::UseTypeIndex {
                    type_id: Self::hash_iri(&n.iri),
                }
            } else {
                IndexHint::FullScan
            }
        } else if let TermPattern::NamedNode(n) = &pattern.predicate {
            IndexHint::UsePredicateIndex {
                predicate_id: Self::hash_iri(&n.iri),
                selectivity,
            }
        } else {
            IndexHint::FullScan
        };

        TriplePatternCost {
            pattern: pattern.clone(),
            cardinality,
            selectivity,
            index_hint,
            bound_variables,
        }
    }

    fn estimate_selectivity(&self, pattern: &TriplePattern, stats: &Statistics) -> f64 {
        let mut selectivity = 1.0;

        // Subject selectivity
        if !pattern.subject.is_variable() {
            let subjects = stats.distinct_subjects().max(1) as f64;
            selectivity *= 1.0 / subjects;
        }

        // Predicate selectivity
        if let TermPattern::NamedNode(n) = &pattern.predicate {
            let pred_id = Self::hash_iri(&n.iri);
            selectivity *= stats.predicate_selectivity(pred_id);
        }

        // Object selectivity
        if !pattern.object.is_variable() {
            let objects = stats.distinct_objects().max(1) as f64;
            selectivity *= 1.0 / objects;
        }

        selectivity.max(0.0001) // Minimum selectivity
    }

    fn find_best_next_pattern(
        &self,
        candidates: &[TriplePatternCost],
        bound_vars: &HashSet<Variable>,
    ) -> usize {
        if bound_vars.is_empty() {
            // First pattern: pick most selective
            return candidates
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.selectivity.partial_cmp(&b.selectivity).unwrap())
                .map_or(0, |(i, _)| i);
        }

        // Find pattern that shares variables and has lowest cost
        let mut best_idx = 0;
        let mut best_score = f64::MAX;

        for (i, cost) in candidates.iter().enumerate() {
            let shared = cost.bound_variables.intersection(bound_vars).count();

            // Prefer patterns that share variables
            let join_factor = if shared > 0 { 0.1 } else { 1.0 };
            let score = cost.cost(&self.cost_model) * join_factor;

            if score < best_score {
                best_score = score;
                best_idx = i;
            }
        }

        best_idx
    }

    fn calculate_plan_cost(&self, patterns: &[TriplePatternCost]) -> f64 {
        let mut total = 0.0;
        let mut cumulative_cardinality = 1.0;

        for cost in patterns {
            // Cost = pattern cost * cumulative cardinality (for nested loop)
            total += cost.cost(&self.cost_model) * cumulative_cardinality;
            cumulative_cardinality *= cost.cardinality.max(1.0);
        }

        total
    }

    fn hash_iri(iri: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        iri.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for JoinOrderOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falkorsemantic_parser::sparql::NamedNode;

    fn make_pattern(subject: &str, predicate: &str, object: &str) -> TriplePattern {
        let s = if subject.starts_with('?') {
            TermPattern::Variable(Variable::new(&subject[1..]))
        } else {
            TermPattern::NamedNode(NamedNode::new(subject))
        };

        let p = if predicate.starts_with('?') {
            TermPattern::Variable(Variable::new(&predicate[1..]))
        } else {
            TermPattern::NamedNode(NamedNode::new(predicate))
        };

        let o = if object.starts_with('?') {
            TermPattern::Variable(Variable::new(&object[1..]))
        } else {
            TermPattern::NamedNode(NamedNode::new(object))
        };

        TriplePattern {
            subject: s,
            predicate: p,
            object: o,
        }
    }

    #[test]
    fn test_optimizer_empty() {
        let optimizer = JoinOrderOptimizer::new();
        let plan = optimizer.optimize_heuristic(&[]);

        assert!(plan.patterns.is_empty());
        assert_eq!(plan.total_cost, 0.0);
    }

    #[test]
    fn test_optimizer_single_pattern() {
        let optimizer = JoinOrderOptimizer::new();
        let patterns = vec![make_pattern("?s", "?p", "?o")];

        let plan = optimizer.optimize_heuristic(&patterns);

        assert_eq!(plan.patterns.len(), 1);
    }

    #[test]
    fn test_optimizer_prefers_selective() {
        let optimizer = JoinOrderOptimizer::new();

        // Pattern with bound predicate should be more selective
        let patterns = vec![
            make_pattern("?s", "?p", "?o"),                       // All variables
            make_pattern("?s", "http://example.org/knows", "?o"), // Bound predicate
        ];

        let plan = optimizer.optimize_heuristic(&patterns);

        // Bound predicate pattern should come first
        assert!(!plan.patterns[0].pattern.predicate.is_variable());
    }

    #[test]
    fn test_optimizer_type_pattern() {
        let optimizer = JoinOrderOptimizer::new();

        let patterns = vec![
            make_pattern("?s", "http://example.org/name", "?name"),
            make_pattern(
                "?s",
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                "http://example.org/Person",
            ),
        ];

        let plan = optimizer.optimize_heuristic(&patterns);

        // Type pattern should be recognized and prioritized
        assert!(plan.patterns[0].index_hint.uses_index());
    }

    #[test]
    fn test_optimizer_with_statistics() {
        let optimizer = JoinOrderOptimizer::new();
        let stats = Statistics::new();

        // Set up some statistics
        for _ in 0..1000 {
            stats.increment_triple_count();
        }
        stats.set_distinct_counts(100, 10, 200);

        // Add predicate frequencies
        let pred1_id = JoinOrderOptimizer::hash_iri("http://example.org/knows");
        let pred2_id = JoinOrderOptimizer::hash_iri("http://example.org/name");
        stats.update_predicate_frequency(pred1_id, 500); // 50% of triples
        stats.update_predicate_frequency(pred2_id, 100); // 10% of triples

        let patterns = vec![
            make_pattern("?s", "http://example.org/knows", "?o"), // Less selective
            make_pattern("?s", "http://example.org/name", "?n"),  // More selective
        ];

        let plan = optimizer.optimize_with_statistics(&patterns, &stats, None);

        // More selective pattern should come first
        assert_eq!(plan.patterns.len(), 2);
    }

    #[test]
    fn test_cost_model() {
        let cost_model = CostModel::default();

        let pattern = TriplePatternCost {
            pattern: make_pattern("?s", "?p", "?o"),
            cardinality: 1000.0,
            selectivity: 1.0,
            index_hint: IndexHint::FullScan,
            bound_variables: HashSet::new(),
        };

        let full_scan_cost = pattern.cost(&cost_model);

        let indexed_pattern = TriplePatternCost {
            pattern: make_pattern("?s", "?p", "?o"),
            cardinality: 1000.0,
            selectivity: 0.1,
            index_hint: IndexHint::UsePredicateIndex {
                predicate_id: 1,
                selectivity: 0.1,
            },
            bound_variables: HashSet::new(),
        };

        let index_cost = indexed_pattern.cost(&cost_model);

        // Index access should be cheaper
        assert!(index_cost < full_scan_cost);
    }

    #[test]
    fn test_join_variable_sharing() {
        let optimizer = JoinOrderOptimizer::new();

        // Three patterns sharing ?person variable
        let patterns = vec![
            make_pattern("?person", "http://example.org/name", "?name"),
            make_pattern("?person", "http://example.org/age", "?age"),
            make_pattern("?other", "http://example.org/knows", "?other2"),
        ];

        let plan = optimizer.optimize_heuristic(&patterns);

        // Patterns sharing ?person should be adjacent
        let vars0 = &plan.patterns[0].bound_variables;
        let vars1 = &plan.patterns[1].bound_variables;

        // First two patterns should share variables
        assert!(!vars0.intersection(vars1).collect::<Vec<_>>().is_empty());
    }
}
