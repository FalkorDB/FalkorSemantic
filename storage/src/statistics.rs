//! Statistics Collection for Query Optimization
//!
//! Collects and maintains statistics about RDF graph data to enable
//! cost-based query optimization.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use crate::IriId;

/// Graph statistics for query optimization
#[derive(Debug, Default)]
pub struct Statistics {
    /// Total number of triples
    triple_count: AtomicU64,
    /// Number of distinct subjects
    distinct_subjects: RwLock<u64>,
    /// Number of distinct predicates
    distinct_predicates: RwLock<u64>,
    /// Number of distinct objects
    distinct_objects: RwLock<u64>,
    /// Predicate frequency distribution
    predicate_frequency: RwLock<HashMap<IriId, u64>>,
    /// Type distribution (type IRI ID -> count)
    type_distribution: RwLock<HashMap<IriId, u64>>,
    /// Subject degree distribution (average outgoing edges)
    avg_subject_degree: RwLock<f64>,
    /// Object degree distribution (average incoming edges)
    avg_object_degree: RwLock<f64>,
    /// Histogram buckets for predicate selectivity
    predicate_histograms: RwLock<HashMap<IriId, SelectivityHistogram>>,
}

/// Histogram for selectivity estimation
#[derive(Debug, Clone, Default)]
pub struct SelectivityHistogram {
    /// Number of distinct subjects for this predicate
    pub distinct_subjects: u64,
    /// Number of distinct objects for this predicate
    pub distinct_objects: u64,
    /// Total occurrences
    pub count: u64,
    /// Average objects per subject
    pub avg_objects_per_subject: f64,
}

impl SelectivityHistogram {
    /// Estimate selectivity given a bound subject
    pub fn selectivity_with_subject(&self) -> f64 {
        if self.count == 0 {
            return 1.0;
        }
        1.0 / self.distinct_subjects as f64
    }

    /// Estimate selectivity given a bound object
    pub fn selectivity_with_object(&self) -> f64 {
        if self.count == 0 {
            return 1.0;
        }
        1.0 / self.distinct_objects as f64
    }

    /// Estimate selectivity given both subject and object bound
    pub fn selectivity_with_both(&self) -> f64 {
        if self.count == 0 {
            return 1.0;
        }
        1.0 / self.count as f64
    }
}

impl Statistics {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Get total triple count
    pub fn triple_count(&self) -> u64 {
        self.triple_count.load(Ordering::Relaxed)
    }

    /// Get distinct subject count
    pub fn distinct_subjects(&self) -> u64 {
        *self.distinct_subjects.read().unwrap()
    }

    /// Get distinct predicate count
    pub fn distinct_predicates(&self) -> u64 {
        *self.distinct_predicates.read().unwrap()
    }

    /// Get distinct object count
    pub fn distinct_objects(&self) -> u64 {
        *self.distinct_objects.read().unwrap()
    }

    /// Get frequency for a predicate
    pub fn predicate_frequency(&self, predicate_id: IriId) -> u64 {
        let reader = self.predicate_frequency.read().unwrap();
        reader.get(&predicate_id).copied().unwrap_or(0)
    }

    /// Get count for a type
    pub fn type_count(&self, type_id: IriId) -> u64 {
        let reader = self.type_distribution.read().unwrap();
        reader.get(&type_id).copied().unwrap_or(0)
    }

    /// Get predicate selectivity (fraction of triples with this predicate)
    pub fn predicate_selectivity(&self, predicate_id: IriId) -> f64 {
        let total = self.triple_count();
        if total == 0 {
            return 1.0;
        }
        self.predicate_frequency(predicate_id) as f64 / total as f64
    }

    /// Get type selectivity (fraction of subjects with this type)
    pub fn type_selectivity(&self, type_id: IriId) -> f64 {
        let subjects = self.distinct_subjects();
        if subjects == 0 {
            return 1.0;
        }
        self.type_count(type_id) as f64 / subjects as f64
    }

    /// Get histogram for a predicate
    pub fn predicate_histogram(&self, predicate_id: IriId) -> Option<SelectivityHistogram> {
        let reader = self.predicate_histograms.read().unwrap();
        reader.get(&predicate_id).cloned()
    }

    /// Estimate join cardinality between two triple patterns
    pub fn estimate_join_cardinality(
        &self,
        predicate1: Option<IriId>,
        predicate2: Option<IriId>,
        shared_variable: bool,
    ) -> f64 {
        let total = self.triple_count() as f64;
        if total == 0.0 {
            return 0.0;
        }

        let sel1 = predicate1.map_or(1.0, |p| self.predicate_selectivity(p));
        let sel2 = predicate2.map_or(1.0, |p| self.predicate_selectivity(p));

        if shared_variable {
            // With shared variable, use independence assumption with adjustment
            total * sel1 * sel2 * 0.1 // Adjustment factor for correlation
        } else {
            // Cartesian product
            total * sel1 * total * sel2
        }
    }

    /// Increment triple count
    pub fn increment_triple_count(&self) {
        self.triple_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement triple count
    pub fn decrement_triple_count(&self) {
        self.triple_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Update predicate frequency
    pub fn update_predicate_frequency(&self, predicate_id: IriId, delta: i64) {
        let mut writer = self.predicate_frequency.write().unwrap();
        let entry = writer.entry(predicate_id).or_insert(0);
        if delta > 0 {
            *entry += delta as u64;
        } else {
            *entry = entry.saturating_sub((-delta) as u64);
        }
    }

    /// Update type distribution
    pub fn update_type_count(&self, type_id: IriId, delta: i64) {
        let mut writer = self.type_distribution.write().unwrap();
        let entry = writer.entry(type_id).or_insert(0);
        if delta > 0 {
            *entry += delta as u64;
        } else {
            *entry = entry.saturating_sub((-delta) as u64);
        }
    }

    /// Set distinct counts
    pub fn set_distinct_counts(&self, subjects: u64, predicates: u64, objects: u64) {
        *self.distinct_subjects.write().unwrap() = subjects;
        *self.distinct_predicates.write().unwrap() = predicates;
        *self.distinct_objects.write().unwrap() = objects;
    }

    /// Set average degrees
    pub fn set_avg_degrees(&self, subject_degree: f64, object_degree: f64) {
        *self.avg_subject_degree.write().unwrap() = subject_degree;
        *self.avg_object_degree.write().unwrap() = object_degree;
    }

    /// Update histogram for a predicate
    pub fn update_predicate_histogram(&self, predicate_id: IriId, histogram: SelectivityHistogram) {
        let mut writer = self.predicate_histograms.write().unwrap();
        writer.insert(predicate_id, histogram);
    }

    /// Clear all statistics
    pub fn clear(&self) {
        self.triple_count.store(0, Ordering::Relaxed);
        *self.distinct_subjects.write().unwrap() = 0;
        *self.distinct_predicates.write().unwrap() = 0;
        *self.distinct_objects.write().unwrap() = 0;
        self.predicate_frequency.write().unwrap().clear();
        self.type_distribution.write().unwrap().clear();
        *self.avg_subject_degree.write().unwrap() = 0.0;
        *self.avg_object_degree.write().unwrap() = 0.0;
        self.predicate_histograms.write().unwrap().clear();
    }

    /// Export statistics as a summary
    pub fn summary(&self) -> StatisticsSummary {
        let pred_freq = self.predicate_frequency.read().unwrap();
        let type_dist = self.type_distribution.read().unwrap();

        StatisticsSummary {
            triple_count: self.triple_count(),
            distinct_subjects: self.distinct_subjects(),
            distinct_predicates: self.distinct_predicates(),
            distinct_objects: self.distinct_objects(),
            predicate_count: pred_freq.len(),
            type_count: type_dist.len(),
            avg_subject_degree: *self.avg_subject_degree.read().unwrap(),
            avg_object_degree: *self.avg_object_degree.read().unwrap(),
        }
    }
}

/// Summary of graph statistics
#[derive(Debug, Clone)]
pub struct StatisticsSummary {
    pub triple_count: u64,
    pub distinct_subjects: u64,
    pub distinct_predicates: u64,
    pub distinct_objects: u64,
    pub predicate_count: usize,
    pub type_count: usize,
    pub avg_subject_degree: f64,
    pub avg_object_degree: f64,
}

impl StatisticsSummary {
    /// Get graph density (edges / possible edges)
    pub fn density(&self) -> f64 {
        if self.distinct_subjects == 0 || self.distinct_objects == 0 {
            return 0.0;
        }
        self.triple_count as f64 / (self.distinct_subjects * self.distinct_objects) as f64
    }
}

/// Builder for collecting statistics from data
#[derive(Debug, Default)]
pub struct StatisticsCollector {
    /// Seen subjects
    subjects: RwLock<std::collections::HashSet<IriId>>,
    /// Seen predicates
    predicates: RwLock<std::collections::HashSet<IriId>>,
    /// Seen objects
    objects: RwLock<std::collections::HashSet<IriId>>,
    /// Predicate to (subject, object) pairs
    predicate_edges: RwLock<HashMap<IriId, Vec<(IriId, IriId)>>>,
    /// Type assertions
    types: RwLock<HashMap<IriId, std::collections::HashSet<IriId>>>,
    /// Triple count
    count: AtomicU64,
}

impl StatisticsCollector {
    /// Create a new collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a triple to the collector
    pub fn add_triple(&self, subject_id: IriId, predicate_id: IriId, object_id: IriId) {
        self.subjects.write().unwrap().insert(subject_id);
        self.predicates.write().unwrap().insert(predicate_id);
        self.objects.write().unwrap().insert(object_id);

        self.predicate_edges
            .write()
            .unwrap()
            .entry(predicate_id)
            .or_default()
            .push((subject_id, object_id));

        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Add a type assertion
    pub fn add_type(&self, subject_id: IriId, type_id: IriId) {
        self.types
            .write()
            .unwrap()
            .entry(type_id)
            .or_default()
            .insert(subject_id);
    }

    /// Build statistics from collected data
    pub fn build(&self) -> Statistics {
        let stats = Statistics::new();

        // Set triple count
        let count = self.count.load(Ordering::Relaxed);
        stats.triple_count.store(count, Ordering::Relaxed);

        // Set distinct counts
        let subjects = self.subjects.read().unwrap().len() as u64;
        let predicates = self.predicates.read().unwrap().len() as u64;
        let objects = self.objects.read().unwrap().len() as u64;
        stats.set_distinct_counts(subjects, predicates, objects);

        // Set predicate frequencies and histograms
        {
            let pred_edges = self.predicate_edges.read().unwrap();
            for (pred_id, edges) in pred_edges.iter() {
                stats.update_predicate_frequency(*pred_id, edges.len() as i64);

                // Build histogram
                let distinct_subjects: std::collections::HashSet<_> =
                    edges.iter().map(|(s, _)| *s).collect();
                let distinct_objects: std::collections::HashSet<_> =
                    edges.iter().map(|(_, o)| *o).collect();

                let histogram = SelectivityHistogram {
                    distinct_subjects: distinct_subjects.len() as u64,
                    distinct_objects: distinct_objects.len() as u64,
                    count: edges.len() as u64,
                    avg_objects_per_subject: if distinct_subjects.is_empty() {
                        0.0
                    } else {
                        edges.len() as f64 / distinct_subjects.len() as f64
                    },
                };
                stats.update_predicate_histogram(*pred_id, histogram);
            }
        }

        // Set type distribution
        {
            let types = self.types.read().unwrap();
            for (type_id, subjects) in types.iter() {
                stats.update_type_count(*type_id, subjects.len() as i64);
            }
        }

        // Calculate average degrees
        if subjects > 0 {
            stats.set_avg_degrees(count as f64 / subjects as f64, count as f64 / objects as f64);
        }

        stats
    }

    /// Clear the collector
    pub fn clear(&self) {
        self.subjects.write().unwrap().clear();
        self.predicates.write().unwrap().clear();
        self.objects.write().unwrap().clear();
        self.predicate_edges.write().unwrap().clear();
        self.types.write().unwrap().clear();
        self.count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_basic() {
        let stats = Statistics::new();

        stats.increment_triple_count();
        stats.increment_triple_count();
        stats.increment_triple_count();

        assert_eq!(stats.triple_count(), 3);

        stats.update_predicate_frequency(10, 2);
        stats.update_predicate_frequency(20, 1);

        assert_eq!(stats.predicate_frequency(10), 2);
        assert_eq!(stats.predicate_frequency(20), 1);
    }

    #[test]
    fn test_statistics_selectivity() {
        let stats = Statistics::new();

        // Add 100 triples
        for _ in 0..100 {
            stats.increment_triple_count();
        }

        // 30 with predicate 1, 10 with predicate 2
        stats.update_predicate_frequency(1, 30);
        stats.update_predicate_frequency(2, 10);

        let sel1 = stats.predicate_selectivity(1);
        let sel2 = stats.predicate_selectivity(2);

        assert!((sel1 - 0.3).abs() < 0.001);
        assert!((sel2 - 0.1).abs() < 0.001);

        // Predicate 2 is more selective
        assert!(sel2 < sel1);
    }

    #[test]
    fn test_selectivity_histogram() {
        let hist = SelectivityHistogram {
            distinct_subjects: 10,
            distinct_objects: 5,
            count: 20,
            avg_objects_per_subject: 2.0,
        };

        assert!((hist.selectivity_with_subject() - 0.1).abs() < 0.001);
        assert!((hist.selectivity_with_object() - 0.2).abs() < 0.001);
        assert!((hist.selectivity_with_both() - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_statistics_collector() {
        let collector = StatisticsCollector::new();

        // Add some triples
        collector.add_triple(1, 10, 100); // Alice knows Bob
        collector.add_triple(1, 10, 101); // Alice knows Carol
        collector.add_triple(2, 10, 100); // Bob knows Bob
        collector.add_triple(1, 20, 200); // Alice age 30

        collector.add_type(1, 1000); // Alice is Person
        collector.add_type(2, 1000); // Bob is Person

        let stats = collector.build();

        assert_eq!(stats.triple_count(), 4);
        assert_eq!(stats.distinct_subjects(), 2);
        assert_eq!(stats.distinct_predicates(), 2);

        assert_eq!(stats.predicate_frequency(10), 3);
        assert_eq!(stats.predicate_frequency(20), 1);

        assert_eq!(stats.type_count(1000), 2);

        // Check histogram
        let hist = stats.predicate_histogram(10).unwrap();
        assert_eq!(hist.count, 3);
        assert_eq!(hist.distinct_subjects, 2);
    }

    #[test]
    fn test_statistics_summary() {
        let stats = Statistics::new();

        for _ in 0..100 {
            stats.increment_triple_count();
        }
        stats.set_distinct_counts(10, 5, 20);
        stats.set_avg_degrees(10.0, 5.0);

        let summary = stats.summary();

        assert_eq!(summary.triple_count, 100);
        assert_eq!(summary.distinct_subjects, 10);
        assert_eq!(summary.distinct_predicates, 5);
        assert_eq!(summary.distinct_objects, 20);

        // Density = 100 / (10 * 20) = 0.5
        assert!((summary.density() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_join_cardinality_estimation() {
        let stats = Statistics::new();

        // 1000 triples
        for _ in 0..1000 {
            stats.increment_triple_count();
        }

        // Predicate 1: 100 occurrences (10%)
        // Predicate 2: 10 occurrences (1%)
        stats.update_predicate_frequency(1, 100);
        stats.update_predicate_frequency(2, 10);

        let card = stats.estimate_join_cardinality(Some(1), Some(2), true);

        // With shared variable: 1000 * 0.1 * 0.01 * 0.1 = 0.1
        assert!(card > 0.0 && card < 10.0);
    }
}
