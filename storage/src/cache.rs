//! Caching Structures
//!
//! Provides caching for frequently accessed data including:
//! - Namespace mappings (prefix expansion/contraction)
//! - Query plan cache

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// A simple LRU cache with optional TTL
#[derive(Debug)]
pub struct LruCache<K, V> {
    /// Maximum capacity
    capacity: usize,
    /// Cache entries with access time
    entries: RwLock<HashMap<K, CacheEntry<V>>>,
    /// Optional TTL for entries
    ttl: Option<Duration>,
}

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    last_access: Instant,
    created: Instant,
}

impl<K: Eq + Hash + Clone, V: Clone> LruCache<K, V> {
    /// Create a new LRU cache with given capacity
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: RwLock::new(HashMap::with_capacity(capacity)),
            ttl: None,
        }
    }

    /// Create a new LRU cache with TTL
    #[must_use]
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            entries: RwLock::new(HashMap::with_capacity(capacity)),
            ttl: Some(ttl),
        }
    }

    /// Get a value from the cache
    pub fn get(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get_mut(key) {
            // Check TTL
            if let Some(ttl) = self.ttl {
                if entry.created.elapsed() > ttl {
                    entries.remove(key);
                    return None;
                }
            }

            entry.last_access = Instant::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Insert a value into the cache
    pub fn insert(&self, key: K, value: V) {
        let mut entries = self.entries.write().unwrap();

        // Evict if at capacity
        if entries.len() >= self.capacity && !entries.contains_key(&key) {
            self.evict_lru(&mut entries);
        }

        let now = Instant::now();
        entries.insert(
            key,
            CacheEntry {
                value,
                last_access: now,
                created: now,
            },
        );
    }

    /// Remove a value from the cache
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.write().unwrap();
        entries.remove(key).map(|e| e.value)
    }

    /// Check if key exists (without updating access time)
    pub fn contains(&self, key: &K) -> bool {
        let entries = self.entries.read().unwrap();
        if let Some(entry) = entries.get(key) {
            if let Some(ttl) = self.ttl {
                return entry.created.elapsed() <= ttl;
            }
            true
        } else {
            false
        }
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
    }

    /// Get current size
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Evict expired entries
    pub fn evict_expired(&self) {
        if let Some(ttl) = self.ttl {
            let mut entries = self.entries.write().unwrap();
            entries.retain(|_, entry| entry.created.elapsed() <= ttl);
        }
    }

    fn evict_lru(&self, entries: &mut HashMap<K, CacheEntry<V>>) {
        if entries.is_empty() {
            return;
        }

        // Find LRU entry
        let lru_key = entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            entries.remove(&key);
        }
    }
}

impl<K, V> Default for LruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Cache for namespace prefix expansions
#[derive(Debug, Default)]
pub struct NamespaceCache {
    /// Prefix to namespace IRI cache
    expansion_cache: LruCache<String, String>,
    /// Namespace IRI to prefix cache
    contraction_cache: LruCache<String, String>,
}

impl NamespaceCache {
    /// Create a new namespace cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            expansion_cache: LruCache::new(1000),
            contraction_cache: LruCache::new(1000),
        }
    }

    /// Create with custom capacity
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            expansion_cache: LruCache::new(capacity),
            contraction_cache: LruCache::new(capacity),
        }
    }

    /// Cache a prefix expansion
    pub fn cache_expansion(&self, prefix: &str, namespace: &str) {
        self.expansion_cache
            .insert(prefix.to_string(), namespace.to_string());
    }

    /// Get cached expansion for a prefix
    pub fn get_expansion(&self, prefix: &str) -> Option<String> {
        self.expansion_cache.get(&prefix.to_string())
    }

    /// Cache a namespace contraction
    pub fn cache_contraction(&self, namespace: &str, prefix: &str) {
        self.contraction_cache
            .insert(namespace.to_string(), prefix.to_string());
    }

    /// Get cached prefix for a namespace
    pub fn get_contraction(&self, namespace: &str) -> Option<String> {
        self.contraction_cache.get(&namespace.to_string())
    }

    /// Clear all caches
    pub fn clear(&self) {
        self.expansion_cache.clear();
        self.contraction_cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            expansion_size: self.expansion_cache.len(),
            contraction_size: self.contraction_cache.len(),
        }
    }
}

/// Statistics for namespace cache
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub expansion_size: usize,
    pub contraction_size: usize,
}

/// Cache for query plans
#[derive(Debug)]
pub struct QueryPlanCache {
    /// Maps query hash to cached plan
    cache: LruCache<u64, CachedPlan>,
}

/// A cached query plan
#[derive(Debug, Clone)]
pub struct CachedPlan {
    /// The original SPARQL query (for verification)
    pub query: String,
    /// The optimized Cypher statements
    pub cypher_statements: Vec<String>,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Cache hit count
    pub hit_count: u64,
}

impl QueryPlanCache {
    /// Create a new query plan cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(500),
        }
    }

    /// Create with custom capacity
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(capacity),
        }
    }

    /// Create with TTL
    #[must_use]
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            cache: LruCache::with_ttl(capacity, ttl),
        }
    }

    /// Get a cached plan for a query
    pub fn get(&self, query: &str) -> Option<CachedPlan> {
        let hash = Self::hash_query(query);
        let normalized = Self::normalize_query(query);
        self.cache.get(&hash).and_then(|plan| {
            // Verify normalized query matches (handle hash collisions)
            if plan.query == normalized {
                Some(plan)
            } else {
                None
            }
        })
    }

    /// Cache a query plan
    pub fn insert(&self, query: &str, cypher_statements: Vec<String>, estimated_cost: f64) {
        let hash = Self::hash_query(query);
        let normalized = Self::normalize_query(query);
        self.cache.insert(
            hash,
            CachedPlan {
                query: normalized,
                cypher_statements,
                estimated_cost,
                hit_count: 0,
            },
        );
    }

    /// Invalidate a cached plan
    pub fn invalidate(&self, query: &str) {
        let hash = Self::hash_query(query);
        self.cache.remove(&hash);
    }

    /// Clear all cached plans
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Evict expired entries
    pub fn evict_expired(&self) {
        self.cache.evict_expired();
    }

    fn normalize_query(query: &str) -> String {
        query.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn hash_query(query: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        let normalized = Self::normalize_query(query);
        normalized.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for QueryPlanCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_basic() {
        let cache: LruCache<String, i32> = LruCache::new(3);

        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        cache.insert("c".to_string(), 3);

        assert_eq!(cache.get(&"a".to_string()), Some(1));
        assert_eq!(cache.get(&"b".to_string()), Some(2));
        assert_eq!(cache.get(&"c".to_string()), Some(3));
        assert_eq!(cache.get(&"d".to_string()), None);
    }

    #[test]
    fn test_lru_cache_eviction() {
        let cache: LruCache<String, i32> = LruCache::new(2);

        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);

        // Access 'a' to make it more recent
        cache.get(&"a".to_string());

        // Insert 'c' should evict 'b' (least recently used)
        cache.insert("c".to_string(), 3);

        assert_eq!(cache.get(&"a".to_string()), Some(1));
        assert_eq!(cache.get(&"b".to_string()), None); // Evicted
        assert_eq!(cache.get(&"c".to_string()), Some(3));
    }

    #[test]
    fn test_lru_cache_ttl() {
        let cache: LruCache<String, i32> = LruCache::with_ttl(10, Duration::from_millis(50));

        cache.insert("a".to_string(), 1);
        assert_eq!(cache.get(&"a".to_string()), Some(1));

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(60));

        assert_eq!(cache.get(&"a".to_string()), None);
    }

    #[test]
    fn test_namespace_cache() {
        let cache = NamespaceCache::new();

        cache.cache_expansion("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#");
        cache.cache_contraction("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "rdf");

        assert_eq!(
            cache.get_expansion("rdf"),
            Some("http://www.w3.org/1999/02/22-rdf-syntax-ns#".to_string())
        );
        assert_eq!(
            cache.get_contraction("http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
            Some("rdf".to_string())
        );
        assert_eq!(cache.get_expansion("unknown"), None);
    }

    #[test]
    fn test_query_plan_cache() {
        let cache = QueryPlanCache::new();

        let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
        let cypher = vec!["MATCH (s)-[r]->(o) RETURN s, r, o".to_string()];

        cache.insert(query, cypher.clone(), 1.0);

        let cached = cache.get(query);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().cypher_statements, cypher);

        // Normalized queries should match
        let query_normalized = "SELECT  ?s  ?p  ?o  WHERE  {  ?s  ?p  ?o  }";
        let cached = cache.get(query_normalized);
        assert!(cached.is_some());
    }

    #[test]
    fn test_query_plan_cache_invalidation() {
        let cache = QueryPlanCache::new();

        let query = "SELECT * WHERE { ?s ?p ?o }";
        cache.insert(query, vec!["MATCH ...".to_string()], 1.0);

        assert!(cache.get(query).is_some());

        cache.invalidate(query);

        assert!(cache.get(query).is_none());
    }
}
