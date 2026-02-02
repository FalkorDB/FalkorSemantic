//! Query Optimization Module
//!
//! Provides query optimization including BGP join ordering and cost estimation.

mod optimizer;

pub use optimizer::{CostModel, JoinOrderOptimizer, OptimizedPlan, TriplePatternCost};
