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
    pub fn get_or_insert(&self, iri: &Iri) -> IriId {
        let iri_str = iri.as_str();

        // First, try to read with a read lock
        {
            let reader = self.iri_to_id.read().unwrap();
            if let Some(&id) = reader.get(iri_str) {
                return id;
            }
        }

        // Not found, need to insert with write lock
        let mut writer = self.iri_to_id.write().unwrap();

        // Double-check after acquiring write lock
        if let Some(&id) = writer.get(iri_str) {
            return id;
        }

        // Assign new ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        writer.insert(iri_str.to_string(), id);

        // Also update reverse mapping
        let mut reverse = self.id_to_iri.write().unwrap();
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
    pub fn import(&self, entries: Vec<(IriId, String)>) -> Result<(), StorageError> {
        let mut iri_writer = self.iri_to_id.write().unwrap();
        let mut id_writer = self.id_to_iri.write().unwrap();

        let mut max_id = self.next_id.load(Ordering::SeqCst);

        for (id, iri) in entries {
            iri_writer.insert(iri.clone(), id);
            id_writer.insert(id, iri);
            if id >= max_id {
                max_id = id + 1;
            }
        }

        self.next_id.store(max_id, Ordering::SeqCst);
        Ok(())
    }

    /// Clear all entries from the dictionary
    pub fn clear(&self) {
        let mut iri_writer = self.iri_to_id.write().unwrap();
        let mut id_writer = self.id_to_iri.write().unwrap();
        iri_writer.clear();
        id_writer.clear();
        self.next_id.store(1, Ordering::SeqCst);
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
}
