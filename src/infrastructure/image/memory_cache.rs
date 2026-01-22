//! In-memory LRU image cache implementation.

use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use crate::domain::entities::ImageId;
use crate::domain::ports::ImageCachePort;

/// Default maximum number of images to cache in memory.
pub const DEFAULT_CACHE_SIZE: usize = 50;

/// In-memory LRU cache for decoded images.
/// Thread-safe and optimized for frequent reads.
pub struct MemoryImageCache {
    cache: Arc<RwLock<LruCache<ImageId, Arc<image::DynamicImage>>>>,
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
}

impl MemoryImageCache {
    /// Creates a new cache with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN);
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(cap))),
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Creates a new cache with the default capacity.
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }

    /// Returns cache statistics.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        CacheStats {
            hits,
            misses,
            hit_rate,
            size: self.len(),
        }
    }

    /// Peeks at an image without promoting it in the LRU.
    /// Use this in read-only contexts to avoid write locks.
    pub async fn peek(&self, id: &ImageId) -> Option<Arc<image::DynamicImage>> {
        let cache = self.cache.read().await;
        cache.peek(id).cloned()
    }

    /// Gets multiple images at once, reducing lock contention.
    pub async fn get_batch(&self, ids: &[ImageId]) -> Vec<(ImageId, Arc<image::DynamicImage>)> {
        let mut cache = self.cache.write().await;
        let mut results = Vec::with_capacity(ids.len());

        for id in ids {
            if let Some(img) = cache.get(id) {
                results.push((id.clone(), img.clone()));
            }
        }

        results
    }
}

impl Default for MemoryImageCache {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

/// Statistics about cache performance.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Hit rate as a percentage.
    pub hit_rate: f64,
    /// Current number of cached images.
    pub size: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cache: {} images, {:.1}% hit rate ({} hits, {} misses)",
            self.size, self.hit_rate, self.hits, self.misses
        )
    }
}

#[async_trait::async_trait]
impl ImageCachePort for MemoryImageCache {
    async fn get(&self, id: &ImageId) -> Option<Arc<image::DynamicImage>> {
        let mut cache = self.cache.write().await;
        if let Some(img) = cache.get(id) {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            trace!(id = %id, "Memory cache hit");
            Some(img.clone())
        } else {
            self.misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            trace!(id = %id, "Memory cache miss");
            None
        }
    }

    async fn put(&self, id: ImageId, image: Arc<image::DynamicImage>) {
        let mut cache = self.cache.write().await;
        debug!(id = %id, "Storing image in memory cache");
        cache.put(id, image);
    }

    async fn evict(&self, id: &ImageId) {
        let mut cache = self.cache.write().await;
        if cache.pop(id).is_some() {
            debug!(id = %id, "Evicted image from memory cache");
        }
    }

    fn len(&self) -> usize {
        let cache = self.cache.try_read();
        cache.map(|c| c.len()).unwrap_or(0)
    }

    async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cleared memory image cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let cache = MemoryImageCache::new(10);
        let id = ImageId::new("test1");
        let img = Arc::new(image::DynamicImage::new_rgb8(100, 100));

        cache.put(id.clone(), img.clone()).await;
        let retrieved = cache.get(&id).await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().width(), 100);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = MemoryImageCache::new(10);
        let id = ImageId::new("nonexistent");

        let result = cache.get(&id).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let cache = MemoryImageCache::new(2);

        let id1 = ImageId::new("test1");
        let id2 = ImageId::new("test2");
        let id3 = ImageId::new("test3");

        let img = Arc::new(image::DynamicImage::new_rgb8(10, 10));

        cache.put(id1.clone(), img.clone()).await;
        cache.put(id2.clone(), img.clone()).await;
        cache.put(id3.clone(), img.clone()).await;

        assert!(cache.get(&id1).await.is_none());
        assert!(cache.get(&id2).await.is_some());
        assert!(cache.get(&id3).await.is_some());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = MemoryImageCache::new(10);
        let id = ImageId::new("test1");
        let img = Arc::new(image::DynamicImage::new_rgb8(10, 10));

        cache.put(id.clone(), img).await;

        let _ = cache.get(&id).await;
        let _ = cache.get(&ImageId::new("missing")).await;

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.size, 1);
    }

    #[tokio::test]
    async fn test_peek_does_not_promote() {
        let cache = MemoryImageCache::new(2);

        let id1 = ImageId::new("test1");
        let id2 = ImageId::new("test2");
        let img = Arc::new(image::DynamicImage::new_rgb8(10, 10));

        cache.put(id1.clone(), img.clone()).await;
        cache.put(id2.clone(), img.clone()).await;

        let _ = cache.peek(&id1).await;

        let id3 = ImageId::new("test3");
        cache.put(id3.clone(), img).await;

        assert!(cache.peek(&id1).await.is_none());
    }
}
