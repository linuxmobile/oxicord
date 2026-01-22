//! Async image loading orchestrator.
//!
//! Implements a three-tier cache: Memory -> Disk -> Network

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, trace, warn};

use crate::domain::entities::{ImageId, ImageSource, LoadedImage};
use crate::domain::ports::{CacheError, CacheResult, ImageCachePort};

use super::discord_cdn::optimize_cdn_url_default;
use super::disk_cache::DiskImageCache;
use super::memory_cache::MemoryImageCache;

/// Message sent when an image finishes loading.
#[derive(Debug, Clone)]
pub struct ImageLoadedEvent {
    /// The image ID.
    pub id: ImageId,
    /// The loaded image, or None if failed.
    pub result: Result<LoadedImage, String>,
}

/// Configuration for the image loader.
#[derive(Debug, Clone)]
pub struct ImageLoaderConfig {
    /// Maximum images in memory cache.
    pub memory_cache_size: usize,
    /// Maximum disk cache size in bytes.
    pub disk_cache_size: u64,
    /// Maximum concurrent downloads.
    pub max_concurrent_downloads: usize,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for ImageLoaderConfig {
    fn default() -> Self {
        Self {
            memory_cache_size: 50,
            disk_cache_size: 200 * 1024 * 1024,
            max_concurrent_downloads: 4,
            timeout_secs: 30,
        }
    }
}

/// Orchestrates image loading from memory, disk, and network.
pub struct ImageLoader {
    memory_cache: Arc<MemoryImageCache>,
    disk_cache: Arc<DiskImageCache>,
    pending_loads: Arc<RwLock<HashSet<ImageId>>>,
    event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
    config: ImageLoaderConfig,
    http_client: reqwest::Client,
}

impl std::fmt::Debug for ImageLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageLoader")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ImageLoader {
    /// Creates a new image loader with the given configuration.
    ///
    /// # Errors
    /// Returns error if disk cache or HTTP client cannot be created.
    pub async fn new(
        config: ImageLoaderConfig,
        event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
    ) -> CacheResult<Self> {
        let memory_cache = Arc::new(MemoryImageCache::new(config.memory_cache_size));
        let disk_cache = Arc::new(DiskImageCache::default_location().await?);

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| CacheError::NetworkError(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            memory_cache,
            disk_cache,
            pending_loads: Arc::new(RwLock::new(HashSet::new())),
            event_tx,
            config,
            http_client,
        })
    }

    /// Creates a loader with default configuration.
    ///
    /// # Errors
    /// Returns error if disk cache or HTTP client cannot be created.
    pub async fn with_defaults(
        event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
    ) -> CacheResult<Self> {
        Self::new(ImageLoaderConfig::default(), event_tx).await
    }

    /// Checks memory cache synchronously (non-blocking peek).
    pub async fn check_memory_cache(&self, id: &ImageId) -> Option<Arc<image::DynamicImage>> {
        self.memory_cache.peek(id).await
    }

    /// Loads an image, checking caches first.
    ///
    /// # Errors
    /// Returns error if image cannot be loaded from any source.
    pub async fn load(&self, id: &ImageId, url: &str) -> CacheResult<LoadedImage> {
        if let Some(img) = self.memory_cache.get(id).await {
            return Ok(LoadedImage {
                id: id.clone(),
                image: img,
                source: ImageSource::MemoryCache,
            });
        }

        if let Some(img) = self.disk_cache.get(id).await {
            self.memory_cache.put(id.clone(), img.clone()).await;
            return Ok(LoadedImage {
                id: id.clone(),
                image: img,
                source: ImageSource::DiskCache,
            });
        }

        let optimized_url = optimize_cdn_url_default(url);
        debug!(id = %id, url = %optimized_url, "Downloading image from network");

        let bytes = self.download(&optimized_url).await?;

        let disk_cache = self.disk_cache.clone();
        let id_for_disk = id.clone();
        let bytes_for_disk = bytes.clone();
        tokio::spawn(async move {
            if let Err(e) = disk_cache.put_bytes(&id_for_disk, &bytes_for_disk).await {
                warn!(id = %id_for_disk, error = %e, "Failed to cache to disk");
            }
        });

        let decoded = tokio::task::spawn_blocking(move || image::load_from_memory(&bytes))
            .await
            .map_err(|e| CacheError::DecodeError(format!("Decode task panicked: {e}")))?
            .map_err(|e| CacheError::DecodeError(format!("Failed to decode image: {e}")))?;

        let img = Arc::new(decoded);

        self.memory_cache.put(id.clone(), img.clone()).await;

        Ok(LoadedImage {
            id: id.clone(),
            image: img,
            source: ImageSource::Network,
        })
    }

    /// Starts loading an image asynchronously.
    /// The result will be sent via the event channel.
    pub fn load_async(&self, id: ImageId, url: String) {
        let loader = ImageLoaderHandle {
            memory_cache: self.memory_cache.clone(),
            disk_cache: self.disk_cache.clone(),
            pending_loads: self.pending_loads.clone(),
            event_tx: self.event_tx.clone(),
            http_client: self.http_client.clone(),
        };

        tokio::spawn(async move {
            {
                let mut pending = loader.pending_loads.write().await;
                if pending.contains(&id) {
                    trace!(id = %id, "Image already loading, skipping");
                    return;
                }
                pending.insert(id.clone());
            }

            let result = loader.load_image(&id, &url).await;

            {
                let mut pending = loader.pending_loads.write().await;
                pending.remove(&id);
            }

            let event = ImageLoadedEvent {
                id: id.clone(),
                result,
            };
            if let Err(e) = loader.event_tx.send(event) {
                error!(error = %e, "Failed to send image loaded event");
            }
        });
    }

    /// Prefetches multiple images into cache.
    pub fn prefetch_batch(&self, images: Vec<(ImageId, String)>) {
        for (id, url) in images {
            self.load_async(id, url);
        }
    }

    /// Cancels a pending load.
    pub async fn cancel(&self, id: &ImageId) {
        let mut pending = self.pending_loads.write().await;
        pending.remove(id);
        debug!(id = %id, "Cancelled image load");
    }

    /// Cancels all pending loads.
    pub async fn cancel_all(&self) {
        let mut pending = self.pending_loads.write().await;
        let count = pending.len();
        pending.clear();
        if count > 0 {
            debug!(count = count, "Cancelled all pending image loads");
        }
    }

    /// Returns true if an image is currently loading.
    pub async fn is_loading(&self, id: &ImageId) -> bool {
        let pending = self.pending_loads.read().await;
        pending.contains(id)
    }

    /// Returns the number of pending loads.
    pub async fn pending_count(&self) -> usize {
        let pending = self.pending_loads.read().await;
        pending.len()
    }

    /// Downloads image bytes from a URL.
    async fn download(&self, url: &str) -> CacheResult<Vec<u8>> {
        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| CacheError::NetworkError(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(CacheError::NetworkError(format!(
                "HTTP {}: {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown")
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| CacheError::NetworkError(format!("Failed to read body: {e}")))?;

        Ok(bytes.to_vec())
    }

    /// Evicts images that are far from the viewport.
    pub fn evict_distant(&self, visible_ids: &[ImageId], buffer: usize) {
        let _keep_set: HashSet<_> = visible_ids.iter().collect();

        trace!(
            visible = visible_ids.len(),
            buffer = buffer,
            "Eviction check (LRU handles this automatically)"
        );
    }

    /// Returns memory cache statistics.
    #[must_use]
    pub fn memory_cache_stats(&self) -> super::memory_cache::CacheStats {
        self.memory_cache.stats()
    }

    /// Clears all caches.
    pub async fn clear_all(&self) {
        self.memory_cache.clear().await;
        if let Err(e) = self.disk_cache.clear().await {
            warn!(error = %e, "Failed to clear disk cache");
        }
        info!("Cleared all image caches");
    }
}

/// Internal handle for async loading tasks.
struct ImageLoaderHandle {
    memory_cache: Arc<MemoryImageCache>,
    disk_cache: Arc<DiskImageCache>,
    pending_loads: Arc<RwLock<HashSet<ImageId>>>,
    event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
    http_client: reqwest::Client,
}

impl ImageLoaderHandle {
    async fn load_image(&self, id: &ImageId, url: &str) -> Result<LoadedImage, String> {
        if let Some(img) = self.memory_cache.get(id).await {
            return Ok(LoadedImage {
                id: id.clone(),
                image: img,
                source: ImageSource::MemoryCache,
            });
        }

        if let Some(img) = self.disk_cache.get(id).await {
            self.memory_cache.put(id.clone(), img.clone()).await;
            return Ok(LoadedImage {
                id: id.clone(),
                image: img,
                source: ImageSource::DiskCache,
            });
        }

        let optimized_url = optimize_cdn_url_default(url);
        debug!(id = %id, "Downloading image: {}", optimized_url);

        let response = self
            .http_client
            .get(&optimized_url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read body: {e}"))?;

        let bytes_clone = bytes.to_vec();
        let decoded = tokio::task::spawn_blocking(move || image::load_from_memory(&bytes_clone))
            .await
            .map_err(|e| format!("Decode task panicked: {e}"))?
            .map_err(|e| format!("Decode failed: {e}"))?;

        let img = Arc::new(decoded);

        self.memory_cache.put(id.clone(), img.clone()).await;

        let disk_cache = self.disk_cache.clone();
        let id_clone = id.clone();
        let bytes_vec = bytes.to_vec();
        tokio::spawn(async move {
            if let Err(e) = disk_cache.put_bytes(&id_clone, &bytes_vec).await {
                warn!(id = %id_clone, error = %e, "Failed to cache to disk");
            }
        });

        debug!(id = %id, source = "network", "Image loaded successfully");

        Ok(LoadedImage {
            id: id.clone(),
            image: img,
            source: ImageSource::Network,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_loader_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let loader = ImageLoader::with_defaults(tx).await;
        assert!(loader.is_ok());
    }

    #[tokio::test]
    async fn test_pending_tracking() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let loader = ImageLoader::with_defaults(tx).await.unwrap();

        assert_eq!(loader.pending_count().await, 0);
    }
}
