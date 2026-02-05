//! IRI Dictionary
//!
//! Provides efficient storage and lookup of IRIs using dictionary encoding.
//! Each unique IRI is assigned a numeric ID for compact storage.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use falkorsemantic_parser::rdf::Iri;

use crate::StorageError;

/// A unique identifier for an IRI in the dictionary
pub type IriId = u64;

/// Reserved ID for unknown/invalid IRIs
pub const UNKNOWN_IRI_ID: IriId = 0;

/// IRI Dictionary for efficient storage and lookup
///
/// Maps IRIs to numeric IDs and vice versa. Thread-safe for concurrent access.
#[derive(Debug)]
pub struct IriDictionary {
    /// IRI string to ID mapping
    iri_to_id: RwLock<HashMap<String, IriId>>,
    /// ID to IRI string mapping
    id_to_iri: RwLock<HashMap<IriId, String>>,
    /// Next available ID
    next_id: AtomicU64,
}

impl IriDictionary {
    /// Create a new empty dictionary
    #[must_use] 
    pub fn new() -> Self {
        Self {
            iri_to_id: RwLock::new(HashMap::new()),
            id_to_iri: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1), // Start at 1, 0 is reserved
        }
    }

    /// Get or create an ID for an IRI
    ///
    /// If the IRI already exists, returns the existing ID.
    /// Otherwise, assigns a new ID and stores the mapping.
    ///
    /// # Thread Safety
    /// This method uses double-checked locking for performance. When inserting,
    /// both forward and reverse mappings are updated atomically while holding
    /// both write locks to prevent inconsistency.
    pub fn get_or_insert(&self, iri: &Iri) -> IriId {
        let iri_str = iri.as_str();

        // First, try to read with a read lock (fast path)
        {
            let reader = self.iri_to_id.read().unwrap();
            if let Some(&id) = reader.get(iri_str) {
                return id;
            }
        }

        // Not found, need to insert with write locks
        // IMPORTANT: Acquire both locks together to prevent race conditions
        // Always acquire in the same order (iri_to_id first) to prevent deadlocks
        let mut forward = self.iri_to_id.write().unwrap();
        let mut reverse = self.id_to_iri.write().unwrap();

        // Double-check after acquiring write locks (another thread may have inserted)
        if let Some(&id) = forward.get(iri_str) {
            return id;
        }

        // Assign new ID and update both mappings atomically
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        forward.insert(iri_str.to_string(), id);
        reverse.insert(id, iri_str.to_string());

        id
    }

    /// Get the ID for an IRI without inserting
    pub fn get_id(&self, iri: &Iri) -> Option<IriId> {
        let reader = self.iri_to_id.read().unwrap();
        reader.get(iri.as_str()).copied()
    }

    /// Get the IRI for an ID (reverse lookup)
    pub fn get_iri(&self, id: IriId) -> Option<Iri> {
        let reader = self.id_to_iri.read().unwrap();
        reader.get(&id).map(|s| Iri::new_unchecked(s.clone()))
    }

    /// Check if an IRI is in the dictionary
    pub fn contains(&self, iri: &Iri) -> bool {
        let reader = self.iri_to_id.read().unwrap();
        reader.contains_key(iri.as_str())
    }

    /// Check if an ID is in the dictionary
    pub fn contains_id(&self, id: IriId) -> bool {
        let reader = self.id_to_iri.read().unwrap();
        reader.contains_key(&id)
    }

    /// Get the number of entries in the dictionary
    pub fn len(&self) -> usize {
        let reader = self.iri_to_id.read().unwrap();
        reader.len()
    }

    /// Check if the dictionary is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all IRIs in the dictionary
    pub fn all_iris(&self) -> Vec<Iri> {
        let reader = self.id_to_iri.read().unwrap();
        reader
            .values()
            .map(|s| Iri::new_unchecked(s.clone()))
            .collect()
    }

    /// Get all IDs in the dictionary
    pub fn all_ids(&self) -> Vec<IriId> {
        let reader = self.id_to_iri.read().unwrap();
        reader.keys().copied().collect()
    }

    /// Export the dictionary as a vector of (id, iri) pairs
    pub fn export(&self) -> Vec<(IriId, String)> {
        let reader = self.id_to_iri.read().unwrap();
        reader.iter().map(|(&id, iri)| (id, iri.clone())).collect()
    }

    /// Import entries into the dictionary
    ///
    /// Overwrites existing mappings if there are conflicts.
    ///
    /// # Thread Safety
    /// Acquires both write locks in consistent order (`iri_to_id` first) and
    /// updates `next_id` while still holding locks to ensure consistency.
    pub fn import(&self, entries: Vec<(IriId, String)>) -> Result<(), StorageError> {
        // Always acquire locks in same order: iri_to_id first, then id_to_iri
        let mut forward = self.iri_to_id.write().unwrap();
        let mut reverse = self.id_to_iri.write().unwrap();

        let mut max_id = self.next_id.load(Ordering::SeqCst);

        for (id, iri) in entries {
            forward.insert(iri.clone(), id);
            reverse.insert(id, iri);
            if id >= max_id {
                max_id = id + 1;
            }
        }

        // Update next_id while still holding locks
        self.next_id.store(max_id, Ordering::SeqCst);
        Ok(())
    }

    /// Clear all entries from the dictionary
    ///
    /// # Thread Safety
    /// Acquires both write locks and resets `next_id` atomically.
    pub fn clear(&self) {
        // Always acquire locks in same order: iri_to_id first, then id_to_iri
        let mut forward = self.iri_to_id.write().unwrap();
        let mut reverse = self.id_to_iri.write().unwrap();

        // Reset next_id while holding both locks for consistency
        self.next_id.store(1, Ordering::SeqCst);

        forward.clear();
        reverse.clear();
    }
}

impl Default for IriDictionary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_dictionary_insert_and_lookup() {
        let dict = IriDictionary::new();

        let iri1 = test_iri("http://example.org/resource1");
        let iri2 = test_iri("http://example.org/resource2");

        let id1 = dict.get_or_insert(&iri1);
        let id2 = dict.get_or_insert(&iri2);

        assert_ne!(id1, id2);
        assert_ne!(id1, UNKNOWN_IRI_ID);
        assert_ne!(id2, UNKNOWN_IRI_ID);

        // Same IRI should return same ID
        let id1_again = dict.get_or_insert(&iri1);
        assert_eq!(id1, id1_again);
    }

    #[test]
    fn test_dictionary_reverse_lookup() {
        let dict = IriDictionary::new();

        let iri = test_iri("http://example.org/resource");
        let id = dict.get_or_insert(&iri);

        let retrieved = dict.get_iri(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().as_str(), iri.as_str());
    }

    #[test]
    fn test_dictionary_get_without_insert() {
        let dict = IriDictionary::new();

        let iri = test_iri("http://example.org/resource");
        assert!(dict.get_id(&iri).is_none());

        dict.get_or_insert(&iri);
        assert!(dict.get_id(&iri).is_some());
    }

    #[test]
    fn test_dictionary_export_import() {
        let dict1 = IriDictionary::new();
        let iri1 = test_iri("http://example.org/a");
        let iri2 = test_iri("http://example.org/b");

        dict1.get_or_insert(&iri1);
        dict1.get_or_insert(&iri2);

        let exported = dict1.export();
        assert_eq!(exported.len(), 2);

        let dict2 = IriDictionary::new();
        dict2.import(exported).unwrap();

        assert_eq!(dict2.len(), 2);
        assert!(dict2.contains(&iri1));
        assert!(dict2.contains(&iri2));
    }

    #[test]
    fn test_dictionary_clear() {
        let dict = IriDictionary::new();
        let iri = test_iri("http://example.org/resource");

        dict.get_or_insert(&iri);
        assert!(!dict.is_empty());

        dict.clear();
        assert!(dict.is_empty());
    }

    #[test]
    fn test_dictionary_concurrent_insert() {
        use std::sync::Arc;
        use std::thread;

        let dict = Arc::new(IriDictionary::new());
        let mut handles = vec![];

        // Spawn multiple threads inserting the same IRIs
        for i in 0..4 {
            let dict_clone = Arc::clone(&dict);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let iri = test_iri(&format!("http://example.org/resource/{}", j));
                    dict_clone.get_or_insert(&iri);
                }
                // Also do some lookups
                for j in 0..100 {
                    let iri = test_iri(&format!("http://example.org/resource/{}", j));
                    let id = dict_clone.get_or_insert(&iri);
                    // Verify reverse lookup works
                    let retrieved = dict_clone.get_iri(id);
                    assert!(
                        retrieved.is_some(),
                        "Thread {} failed reverse lookup for {}",
                        i,
                        j
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Should have exactly 100 unique entries
        assert_eq!(dict.len(), 100);

        // Verify all entries have valid forward and reverse mappings
        for j in 0..100 {
            let iri = test_iri(&format!("http://example.org/resource/{}", j));
            let id = dict.get_id(&iri);
            assert!(id.is_some(), "Missing ID for resource {}", j);
            let retrieved = dict.get_iri(id.unwrap());
            assert!(
                retrieved.is_some(),
                "Missing reverse mapping for resource {}",
                j
            );
            assert_eq!(retrieved.unwrap().as_str(), iri.as_str());
        }
    }
}
