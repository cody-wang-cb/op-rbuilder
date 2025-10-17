//! Thread-safe storage for transaction bundling data

use alloy_primitives::{Bytes, B256};
use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe store for bundled transaction data
/// Maps original transaction hash -> raw bundled transaction data
#[derive(Clone, Debug)]
pub struct TxBundleStore {
    inner: Arc<DashMap<B256, Bytes>>,
    max_size: Option<usize>,
}

impl TxBundleStore {
    /// Create a new transaction bundle store
    pub fn new(max_size: Option<usize>) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            max_size,
        }
    }

    /// Insert a bundled transaction mapping
    pub fn insert(&self, tx_hash: B256, bundled_tx_data: Bytes) {
        // Simple LRU-like eviction: if we exceed max_size, clear oldest entries
        if let Some(max) = self.max_size {
            if self.inner.len() >= max {
                // Clear half the cache when full (simple eviction strategy)
                let to_remove: Vec<B256> = self
                    .inner
                    .iter()
                    .take(max / 2)
                    .map(|entry| *entry.key())
                    .collect();
                
                for key in to_remove {
                    self.inner.remove(&key);
                }
            }
        }

        self.inner.insert(tx_hash, bundled_tx_data);
    }

    /// Get bundled transaction data for a given transaction hash
    pub fn get(&self, tx_hash: &B256) -> Option<Bytes> {
        self.inner.get(tx_hash).map(|v| v.clone())
    }

    /// Remove a bundled transaction mapping
    pub fn remove(&self, tx_hash: &B256) -> Option<Bytes> {
        self.inner.remove(tx_hash).map(|(_, v)| v)
    }

    /// Get the current number of stored mappings
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear all stored mappings
    pub fn clear(&self) {
        self.inner.clear();
    }
}

impl Default for TxBundleStore {
    fn default() -> Self {
        Self::new(Some(10000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let store = TxBundleStore::new(None);
        let tx_hash = B256::from([1u8; 32]);
        let data = Bytes::from(vec![1, 2, 3, 4]);

        store.insert(tx_hash, data.clone());
        assert_eq!(store.get(&tx_hash), Some(data));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_max_size_eviction() {
        let store = TxBundleStore::new(Some(10));
        
        // Insert 15 items
        for i in 0..15 {
            let tx_hash = B256::from([i as u8; 32]);
            let data = Bytes::from(vec![i]);
            store.insert(tx_hash, data);
        }

        // Should have evicted some entries
        assert!(store.len() <= 10);
    }

    #[test]
    fn test_remove() {
        let store = TxBundleStore::new(None);
        let tx_hash = B256::from([1u8; 32]);
        let data = Bytes::from(vec![1, 2, 3]);

        store.insert(tx_hash, data.clone());
        assert_eq!(store.remove(&tx_hash), Some(data));
        assert_eq!(store.get(&tx_hash), None);
    }
}

