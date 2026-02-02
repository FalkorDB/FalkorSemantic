//! Indexing Structures for RDF Data
//!
//! Provides various indexes for efficient RDF data lookup including:
//! - Namespace-based IRI index
//! - rdf:type index for fast type lookups
//! - Predicate index with selectivity estimates

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::{IriId, UNKNOWN_IRI_ID};

/// Well-known RDF predicates
pub mod rdf_predicates {
    pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
}

/// Index for namespace-based IRI lookups
///
/// Allows efficient retrieval of all IRIs within a namespace.
#[derive(Debug, Default)]
pub struct NamespaceIndex {
    /// Maps namespace prefix to set of IRI IDs
    namespace_to_ids: RwLock<HashMap<String, HashSet<IriId>>>,
    /// Maps IRI ID to its namespace
    id_to_namespace: RwLock<HashMap<IriId, String>>,
}

impl NamespaceIndex {
    /// Create a new empty namespace index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IRI to the index
    pub fn add(&self, id: IriId, namespace: &str) {
        {
            let mut ns_map = self.namespace_to_ids.write().unwrap();
            ns_map
                .entry(namespace.to_string())
                .or_default()
                .insert(id);
        }
        {
            let mut id_map = self.id_to_namespace.write().unwrap();
            id_map.insert(id, namespace.to_string());
        }
    }

    /// Get all IRI IDs in a namespace
    pub fn get_by_namespace(&self, namespace: &str) -> Vec<IriId> {
        let reader = self.namespace_to_ids.read().unwrap();
        reader
            .get(namespace)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get the namespace for an IRI ID
    pub fn get_namespace(&self, id: IriId) -> Option<String> {
        let reader = self.id_to_namespace.read().unwrap();
        reader.get(&id).cloned()
    }

    /// Get all namespaces in the index
    pub fn namespaces(&self) -> Vec<String> {
        let reader = self.namespace_to_ids.read().unwrap();
        reader.keys().cloned().collect()
    }

    /// Get the count of IRIs in a namespace
    pub fn namespace_count(&self, namespace: &str) -> usize {
        let reader = self.namespace_to_ids.read().unwrap();
        reader.get(namespace).map(|ids| ids.len()).unwrap_or(0)
    }

    /// Remove an IRI from the index
    pub fn remove(&self, id: IriId) {
        let namespace = {
            let mut id_map = self.id_to_namespace.write().unwrap();
            id_map.remove(&id)
        };

        if let Some(ns) = namespace {
            let mut ns_map = self.namespace_to_ids.write().unwrap();
            if let Some(ids) = ns_map.get_mut(&ns) {
                ids.remove(&id);
                if ids.is_empty() {
                    ns_map.remove(&ns);
                }
            }
        }
    }

    /// Clear the index
    pub fn clear(&self) {
        self.namespace_to_ids.write().unwrap().clear();
        self.id_to_namespace.write().unwrap().clear();
    }

    /// Get total number of indexed IRIs
    pub fn len(&self) -> usize {
        self.id_to_namespace.read().unwrap().len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Index for local name (suffix) based lookups
#[derive(Debug, Default)]
pub struct LocalNameIndex {
    /// Maps local name to set of IRI IDs
    local_to_ids: RwLock<HashMap<String, HashSet<IriId>>>,
}

impl LocalNameIndex {
    /// Create a new empty local name index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an IRI's local name to the index
    pub fn add(&self, id: IriId, local_name: &str) {
        let mut map = self.local_to_ids.write().unwrap();
        map.entry(local_name.to_string()).or_default().insert(id);
    }

    /// Get all IRI IDs with a given local name
    pub fn get_by_local_name(&self, local_name: &str) -> Vec<IriId> {
        let reader = self.local_to_ids.read().unwrap();
        reader
            .get(local_name)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if a local name exists in the index
    pub fn contains(&self, local_name: &str) -> bool {
        let reader = self.local_to_ids.read().unwrap();
        reader.contains_key(local_name)
    }

    /// Clear the index
    pub fn clear(&self) {
        self.local_to_ids.write().unwrap().clear();
    }
}

/// Index for rdf:type lookups
///
/// Maps type IRIs to the subjects that have that type.
#[derive(Debug, Default)]
pub struct TypeIndex {
    /// Maps type IRI ID to set of subject IRI IDs
    type_to_subjects: RwLock<HashMap<IriId, HashSet<IriId>>>,
    /// Maps subject IRI ID to set of type IRI IDs
    subject_to_types: RwLock<HashMap<IriId, HashSet<IriId>>>,
}

impl TypeIndex {
    /// Create a new empty type index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a type assertion (subject rdf:type type)
    pub fn add_type(&self, subject_id: IriId, type_id: IriId) {
        {
            let mut type_map = self.type_to_subjects.write().unwrap();
            type_map.entry(type_id).or_default().insert(subject_id);
        }
        {
            let mut subj_map = self.subject_to_types.write().unwrap();
            subj_map.entry(subject_id).or_default().insert(type_id);
        }
    }

    /// Get all subjects with a given type
    pub fn get_subjects_by_type(&self, type_id: IriId) -> Vec<IriId> {
        let reader = self.type_to_subjects.read().unwrap();
        reader
            .get(&type_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all types for a subject
    pub fn get_types_for_subject(&self, subject_id: IriId) -> Vec<IriId> {
        let reader = self.subject_to_types.read().unwrap();
        reader
            .get(&subject_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if a subject has a specific type
    pub fn has_type(&self, subject_id: IriId, type_id: IriId) -> bool {
        let reader = self.subject_to_types.read().unwrap();
        reader
            .get(&subject_id)
            .map(|types| types.contains(&type_id))
            .unwrap_or(false)
    }

    /// Get count of subjects with a given type
    pub fn type_count(&self, type_id: IriId) -> usize {
        let reader = self.type_to_subjects.read().unwrap();
        reader.get(&type_id).map(|ids| ids.len()).unwrap_or(0)
    }

    /// Get all distinct types in the index
    pub fn all_types(&self) -> Vec<IriId> {
        let reader = self.type_to_subjects.read().unwrap();
        reader.keys().copied().collect()
    }

    /// Remove a type assertion
    pub fn remove_type(&self, subject_id: IriId, type_id: IriId) {
        {
            let mut type_map = self.type_to_subjects.write().unwrap();
            if let Some(subjects) = type_map.get_mut(&type_id) {
                subjects.remove(&subject_id);
                if subjects.is_empty() {
                    type_map.remove(&type_id);
                }
            }
        }
        {
            let mut subj_map = self.subject_to_types.write().unwrap();
            if let Some(types) = subj_map.get_mut(&subject_id) {
                types.remove(&type_id);
                if types.is_empty() {
                    subj_map.remove(&subject_id);
                }
            }
        }
    }

    /// Remove all types for a subject
    pub fn remove_subject(&self, subject_id: IriId) {
        let types = {
            let mut subj_map = self.subject_to_types.write().unwrap();
            subj_map.remove(&subject_id)
        };

        if let Some(type_ids) = types {
            let mut type_map = self.type_to_subjects.write().unwrap();
            for type_id in type_ids {
                if let Some(subjects) = type_map.get_mut(&type_id) {
                    subjects.remove(&subject_id);
                    if subjects.is_empty() {
                        type_map.remove(&type_id);
                    }
                }
            }
        }
    }

    /// Clear the index
    pub fn clear(&self) {
        self.type_to_subjects.write().unwrap().clear();
        self.subject_to_types.write().unwrap().clear();
    }

    /// Get total number of type assertions
    pub fn len(&self) -> usize {
        let reader = self.type_to_subjects.read().unwrap();
        reader.values().map(|s| s.len()).sum()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.type_to_subjects.read().unwrap().is_empty()
    }
}

/// Index for predicate-based lookups with selectivity estimation
#[derive(Debug, Default)]
pub struct PredicateIndex {
    /// Maps predicate IRI ID to (subject, object) pairs
    predicate_to_edges: RwLock<HashMap<IriId, HashSet<(IriId, IriId)>>>,
    /// Maps predicate to count (for selectivity)
    predicate_counts: RwLock<HashMap<IriId, usize>>,
    /// Total number of triples indexed
    total_triples: RwLock<usize>,
}

impl PredicateIndex {
    /// Create a new empty predicate index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a triple to the index
    pub fn add(&self, predicate_id: IriId, subject_id: IriId, object_id: IriId) {
        {
            let mut edge_map = self.predicate_to_edges.write().unwrap();
            edge_map
                .entry(predicate_id)
                .or_default()
                .insert((subject_id, object_id));
        }
        {
            let mut count_map = self.predicate_counts.write().unwrap();
            *count_map.entry(predicate_id).or_insert(0) += 1;
        }
        {
            let mut total = self.total_triples.write().unwrap();
            *total += 1;
        }
    }

    /// Get all (subject, object) pairs for a predicate
    pub fn get_by_predicate(&self, predicate_id: IriId) -> Vec<(IriId, IriId)> {
        let reader = self.predicate_to_edges.read().unwrap();
        reader
            .get(&predicate_id)
            .map(|edges| edges.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get subjects for a predicate (ignoring object)
    pub fn get_subjects_by_predicate(&self, predicate_id: IriId) -> Vec<IriId> {
        let reader = self.predicate_to_edges.read().unwrap();
        reader
            .get(&predicate_id)
            .map(|edges| edges.iter().map(|(s, _)| *s).collect::<HashSet<_>>())
            .map(|set| set.into_iter().collect())
            .unwrap_or_default()
    }

    /// Get objects for a predicate (ignoring subject)
    pub fn get_objects_by_predicate(&self, predicate_id: IriId) -> Vec<IriId> {
        let reader = self.predicate_to_edges.read().unwrap();
        reader
            .get(&predicate_id)
            .map(|edges| edges.iter().map(|(_, o)| *o).collect::<HashSet<_>>())
            .map(|set| set.into_iter().collect())
            .unwrap_or_default()
    }

    /// Get count of triples with a given predicate
    pub fn predicate_count(&self, predicate_id: IriId) -> usize {
        let reader = self.predicate_counts.read().unwrap();
        reader.get(&predicate_id).copied().unwrap_or(0)
    }

    /// Get selectivity estimate for a predicate (0.0 to 1.0)
    ///
    /// Lower selectivity means more selective (fewer matching triples).
    pub fn selectivity(&self, predicate_id: IriId) -> f64 {
        let total = *self.total_triples.read().unwrap();
        if total == 0 {
            return 1.0;
        }
        let count = self.predicate_count(predicate_id);
        count as f64 / total as f64
    }

    /// Get all predicates sorted by selectivity (most selective first)
    pub fn predicates_by_selectivity(&self) -> Vec<(IriId, f64)> {
        let reader = self.predicate_counts.read().unwrap();
        let total = *self.total_triples.read().unwrap();

        if total == 0 {
            return Vec::new();
        }

        let mut predicates: Vec<_> = reader
            .iter()
            .map(|(&pred, &count)| (pred, count as f64 / total as f64))
            .collect();

        predicates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        predicates
    }

    /// Get all distinct predicates
    pub fn all_predicates(&self) -> Vec<IriId> {
        let reader = self.predicate_counts.read().unwrap();
        reader.keys().copied().collect()
    }

    /// Remove a triple from the index
    pub fn remove(&self, predicate_id: IriId, subject_id: IriId, object_id: IriId) {
        let removed = {
            let mut edge_map = self.predicate_to_edges.write().unwrap();
            if let Some(edges) = edge_map.get_mut(&predicate_id) {
                let removed = edges.remove(&(subject_id, object_id));
                if edges.is_empty() {
                    edge_map.remove(&predicate_id);
                }
                removed
            } else {
                false
            }
        };

        if removed {
            {
                let mut count_map = self.predicate_counts.write().unwrap();
                if let Some(count) = count_map.get_mut(&predicate_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        count_map.remove(&predicate_id);
                    }
                }
            }
            {
                let mut total = self.total_triples.write().unwrap();
                *total = total.saturating_sub(1);
            }
        }
    }

    /// Clear the index
    pub fn clear(&self) {
        self.predicate_to_edges.write().unwrap().clear();
        self.predicate_counts.write().unwrap().clear();
        *self.total_triples.write().unwrap() = 0;
    }

    /// Get total number of indexed triples
    pub fn len(&self) -> usize {
        *self.total_triples.read().unwrap()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Hint for query optimization based on available indexes
#[derive(Debug, Clone, PartialEq)]
pub enum IndexHint {
    /// Use type index for rdf:type pattern
    UseTypeIndex { type_id: IriId },
    /// Use predicate index
    UsePredicateIndex { predicate_id: IriId, selectivity: f64 },
    /// Use namespace index
    UseNamespaceIndex { namespace: String },
    /// No index available, use full scan
    FullScan,
}

impl IndexHint {
    /// Get the estimated cost of this hint (lower is better)
    pub fn estimated_cost(&self) -> f64 {
        match self {
            IndexHint::UseTypeIndex { .. } => 0.1,
            IndexHint::UsePredicateIndex { selectivity, .. } => *selectivity,
            IndexHint::UseNamespaceIndex { .. } => 0.5,
            IndexHint::FullScan => 1.0,
        }
    }

    /// Check if this hint uses an index
    pub fn uses_index(&self) -> bool {
        !matches!(self, IndexHint::FullScan)
    }
}

/// Combined index manager
#[derive(Debug, Default)]
pub struct IndexManager {
    /// Namespace index
    pub namespace_index: NamespaceIndex,
    /// Local name index
    pub local_name_index: LocalNameIndex,
    /// Type index
    pub type_index: TypeIndex,
    /// Predicate index
    pub predicate_index: PredicateIndex,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a triple to all relevant indexes
    pub fn index_triple(
        &self,
        subject_id: IriId,
        predicate_id: IriId,
        object_id: IriId,
        subject_namespace: Option<&str>,
        subject_local: Option<&str>,
        is_type_assertion: bool,
    ) {
        // Index subject by namespace and local name
        if let Some(ns) = subject_namespace {
            self.namespace_index.add(subject_id, ns);
        }
        if let Some(local) = subject_local {
            self.local_name_index.add(subject_id, local);
        }

        // Index predicate
        self.predicate_index.add(predicate_id, subject_id, object_id);

        // Index type assertion
        if is_type_assertion && object_id != UNKNOWN_IRI_ID {
            self.type_index.add_type(subject_id, object_id);
        }
    }

    /// Get index hint for a triple pattern
    pub fn get_hint(
        &self,
        predicate_id: Option<IriId>,
        is_type_pattern: bool,
        type_id: Option<IriId>,
    ) -> IndexHint {
        // Prefer type index for rdf:type patterns
        if is_type_pattern {
            if let Some(tid) = type_id {
                return IndexHint::UseTypeIndex { type_id: tid };
            }
        }

        // Use predicate index if predicate is known
        if let Some(pid) = predicate_id {
            let selectivity = self.predicate_index.selectivity(pid);
            return IndexHint::UsePredicateIndex {
                predicate_id: pid,
                selectivity,
            };
        }

        IndexHint::FullScan
    }

    /// Clear all indexes
    pub fn clear(&self) {
        self.namespace_index.clear();
        self.local_name_index.clear();
        self.type_index.clear();
        self.predicate_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_index() {
        let index = NamespaceIndex::new();

        index.add(1, "http://example.org/");
        index.add(2, "http://example.org/");
        index.add(3, "http://other.org/");

        let example_ids = index.get_by_namespace("http://example.org/");
        assert_eq!(example_ids.len(), 2);
        assert!(example_ids.contains(&1));
        assert!(example_ids.contains(&2));

        assert_eq!(index.namespace_count("http://example.org/"), 2);
        assert_eq!(index.namespace_count("http://other.org/"), 1);
    }

    #[test]
    fn test_local_name_index() {
        let index = LocalNameIndex::new();

        index.add(1, "Person");
        index.add(2, "Person");
        index.add(3, "Organization");

        let person_ids = index.get_by_local_name("Person");
        assert_eq!(person_ids.len(), 2);

        assert!(index.contains("Person"));
        assert!(!index.contains("Unknown"));
    }

    #[test]
    fn test_type_index() {
        let index = TypeIndex::new();

        // Alice and Bob are Person
        index.add_type(1, 100); // Alice is Person
        index.add_type(2, 100); // Bob is Person
        index.add_type(1, 101); // Alice is also Employee

        let persons = index.get_subjects_by_type(100);
        assert_eq!(persons.len(), 2);

        let alice_types = index.get_types_for_subject(1);
        assert_eq!(alice_types.len(), 2);

        assert!(index.has_type(1, 100));
        assert!(!index.has_type(2, 101));

        assert_eq!(index.type_count(100), 2);
        assert_eq!(index.type_count(101), 1);
    }

    #[test]
    fn test_predicate_index() {
        let index = PredicateIndex::new();

        // Add some triples
        index.add(10, 1, 2); // Alice knows Bob
        index.add(10, 1, 3); // Alice knows Carol
        index.add(20, 1, 4); // Alice hasAge 30

        assert_eq!(index.predicate_count(10), 2);
        assert_eq!(index.predicate_count(20), 1);

        let knows_edges = index.get_by_predicate(10);
        assert_eq!(knows_edges.len(), 2);

        // Selectivity: knows = 2/3, hasAge = 1/3
        assert!(index.selectivity(20) < index.selectivity(10));
    }

    #[test]
    fn test_predicate_selectivity() {
        let index = PredicateIndex::new();

        // Add 100 triples with predicate 1, 10 with predicate 2
        for i in 0..100 {
            index.add(1, i, i + 1000);
        }
        for i in 0..10 {
            index.add(2, i, i + 2000);
        }

        let sel1 = index.selectivity(1);
        let sel2 = index.selectivity(2);

        // Predicate 2 is more selective (fewer matches)
        assert!(sel2 < sel1);

        let by_selectivity = index.predicates_by_selectivity();
        assert_eq!(by_selectivity[0].0, 2); // Most selective first
    }

    #[test]
    fn test_index_hint() {
        let type_hint = IndexHint::UseTypeIndex { type_id: 100 };
        let pred_hint = IndexHint::UsePredicateIndex {
            predicate_id: 10,
            selectivity: 0.3,
        };
        let scan_hint = IndexHint::FullScan;

        assert!(type_hint.uses_index());
        assert!(pred_hint.uses_index());
        assert!(!scan_hint.uses_index());

        // Type index has lowest cost
        assert!(type_hint.estimated_cost() < pred_hint.estimated_cost());
        assert!(pred_hint.estimated_cost() < scan_hint.estimated_cost());
    }

    #[test]
    fn test_index_manager() {
        let manager = IndexManager::new();

        // Index a type triple: Alice rdf:type Person
        manager.index_triple(
            1,  // subject: Alice
            10, // predicate: rdf:type
            100, // object: Person
            Some("http://example.org/"),
            Some("Alice"),
            true,
        );

        // Check indexes
        assert!(manager.namespace_index.get_by_namespace("http://example.org/").contains(&1));
        assert!(manager.local_name_index.get_by_local_name("Alice").contains(&1));
        assert!(manager.type_index.has_type(1, 100));
        assert_eq!(manager.predicate_index.predicate_count(10), 1);
    }

    #[test]
    fn test_index_manager_hints() {
        let manager = IndexManager::new();

        // Add some data
        manager.predicate_index.add(10, 1, 2);
        manager.type_index.add_type(1, 100);

        // Type pattern should prefer type index
        let hint = manager.get_hint(Some(10), true, Some(100));
        assert!(matches!(hint, IndexHint::UseTypeIndex { .. }));

        // Non-type pattern with known predicate uses predicate index
        let hint = manager.get_hint(Some(10), false, None);
        assert!(matches!(hint, IndexHint::UsePredicateIndex { .. }));

        // Unknown predicate falls back to scan
        let hint = manager.get_hint(None, false, None);
        assert!(matches!(hint, IndexHint::FullScan));
    }
}
