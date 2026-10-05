use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default, Debug)]
pub struct CacheStats {
    pub l1_hits: AtomicU64,
    pub l2_hits: AtomicU64,
    pub misses: AtomicU64,
    pub writes: AtomicU64,
}

#[derive(Clone)]
pub struct CacheManager {
    cache_dir: PathBuf,
    memory_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    pub max_memory_bytes: usize,
    pub stats: Arc<CacheStats>,
    pub redis_url: Option<String>,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf, max_memory_mb: usize) -> Self {
        Self::with_redis(cache_dir, max_memory_mb, None)
    }

    pub fn with_redis(cache_dir: PathBuf, max_memory_mb: usize, redis_url: Option<String>) -> Self {
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }

        Self {
            cache_dir,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            max_memory_bytes: max_memory_mb * 1024 * 1024,
            stats: Arc::new(CacheStats::default()),
            redis_url,
        }
    }

    pub fn compute_key(doc_id: &str, page: usize, dpi: u32, variant: &str) -> String {
        let input = format!("{}:{}:{}:{}", doc_id, page, dpi, variant);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        // 1. Vérification du cache L1 RAM (Latence ultra-faible < 50 µs)
        {
            let mem = self.memory_cache.read().await;
            if let Some(data) = mem.get(key) {
                self.stats.l1_hits.fetch_add(1, Ordering::Relaxed);
                return Some(data.clone());
            }
        }

        // 2. Vérification du cache L2 distribué / disque partagé
        let disk_path = self.disk_path(key);
        if disk_path.exists() {
            if let Ok(data) = fs::read(&disk_path) {
                self.stats.l2_hits.fetch_add(1, Ordering::Relaxed);
                // Réinsertion dans le cache L1 RAM
                let mut mem = self.memory_cache.write().await;
                mem.insert(key.to_string(), data.clone());
                return Some(data);
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub async fn set(&self, key: &str, data: Vec<u8>) {
        self.stats.writes.fetch_add(1, Ordering::Relaxed);

        // Écriture dans le cache L2 (Volume partagé cluster / disque)
        let disk_path = self.disk_path(key);
        let _ = fs::write(&disk_path, &data);

        // Enregistrement dans le cache L1 RAM
        let mut mem = self.memory_cache.write().await;
        if mem.len() > 1000 {
            mem.clear();
        }
        mem.insert(key.to_string(), data);
    }

    pub fn disk_path(&self, key: &str) -> PathBuf {
        let safe_key = if key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit()) {
            key.to_string()
        } else {
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());
            hex::encode(hasher.finalize())
        };
        self.cache_dir.join(format!("{}.bin", safe_key))
    }

    pub async fn l1_entry_count(&self) -> usize {
        self.memory_cache.read().await.len()
    }

    pub async fn clear_for_doc(&self, _doc_id_prefix: &str) {
        let mut mem = self.memory_cache.write().await;
        mem.clear();
    }

    pub async fn invalidate_document(&self, _doc_id: &str) {
        let mut mem = self.memory_cache.write().await;
        mem.clear();
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
