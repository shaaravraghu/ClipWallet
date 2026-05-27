use crate::clipboard::types::ClipEntry;
use crate::storage::{disk, encrypted, RamStore};
use std::sync::{Arc, RwLock};

pub trait CacheLayer: Send + Sync {
    fn name(&self) -> &'static str;
    fn retrieve(&self, id: u64) -> anyhow::Result<Option<ClipEntry>>;
}

pub struct RamLayer {
    ram: Arc<RwLock<RamStore>>,
}

impl RamLayer {
    pub fn new(ram: Arc<RwLock<RamStore>>) -> Self {
        Self { ram }
    }
}

impl CacheLayer for RamLayer {
    fn name(&self) -> &'static str {
        "RAM Cache"
    }

    fn retrieve(&self, id: u64) -> anyhow::Result<Option<ClipEntry>> {
        let ram = self.ram.read().unwrap();
        
        // Check dynamic ring
        if let Some(entry) = ram.dynamic_ring.iter().find(|e| e.id == id) {
            return Ok(Some(entry.clone()));
        }

        // Check static slots
        if let Some(entry) = ram.static_slots.iter().flatten().find(|e| e.id == id) {
            return Ok(Some(entry.clone()));
        }

        Ok(None)
    }
}

pub struct DiskLayer;

impl CacheLayer for DiskLayer {
    fn name(&self) -> &'static str {
        "Disk Cache"
    }

    fn retrieve(&self, id: u64) -> anyhow::Result<Option<ClipEntry>> {
        disk::retrieve_by_id(id)
    }
}

pub struct VaultLayer;

impl CacheLayer for VaultLayer {
    fn name(&self) -> &'static str {
        "Encrypted Vault"
    }

    fn retrieve(&self, id: u64) -> anyhow::Result<Option<ClipEntry>> {
        match encrypted::load_from_vault(id) {
            Ok(mut entry) => {
                entry.encrypted = false;
                Ok(Some(entry))
            }
            Err(_) => Ok(None),
        }
    }
}

pub struct FallbackChain {
    layers: Vec<Box<dyn CacheLayer>>,
    ram: Arc<RwLock<RamStore>>,
}

impl FallbackChain {
    pub fn new(ram: Arc<RwLock<RamStore>>) -> Self {
        Self {
            layers: vec![
                Box::new(RamLayer::new(Arc::clone(&ram))),
                Box::new(DiskLayer),
                Box::new(VaultLayer),
            ],
            ram,
        }
    }

    pub fn retrieve(&self, id: u64) -> anyhow::Result<Option<ClipEntry>> {
        for layer in &self.layers {
            match layer.retrieve(id) {
                Ok(Some(entry)) => {
                    tracing::info!("[Fallback] Hit in layer '{}' for id={}", layer.name(), id);
                    
                    // Promote back to RAM cache if it was found in a slower layer
                    if layer.name() != "RAM Cache" {
                        let mut ram_write = self.ram.write().unwrap();
                        
                        // To avoid duplicating in dynamic ring if we are retrieving a static slot,
                        // we should be careful. But without context, we add it to the dynamic ring
                        // as a quick cache. If it was static, the user is pasting by slot anyway.
                        // For a pure ID retrieval, putting it in the dynamic ring promotes it.
                        let _evicted = ram_write.push_dynamic(entry.clone());
                        tracing::info!("[Fallback] Promoted id={} to RAM Cache", id);
                    }
                    
                    return Ok(Some(entry));
                }
                Ok(None) => {
                    tracing::debug!("[Fallback] Miss in layer '{}' for id={}", layer.name(), id);
                }
                Err(e) => {
                    tracing::warn!("[Fallback] Layer '{}' failed: {}", layer.name(), e);
                    // Prevent a failed layer from terminating the entire retrieval flow
                }
            }
        }
        
        tracing::info!("[Fallback] Cache Miss across all layers for id={}", id);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_chain_initialization() {
        let ram = Arc::new(RwLock::new(RamStore::new()));
        let chain = FallbackChain::new(ram);
        assert_eq!(chain.layers.len(), 3);
        assert_eq!(chain.layers[0].name(), "RAM Cache");
        assert_eq!(chain.layers[1].name(), "Disk Cache");
        assert_eq!(chain.layers[2].name(), "Encrypted Vault");
    }
}
