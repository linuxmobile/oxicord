//! Async image loading orchestrator.
//!
//! Implements a three-tier cache: Memory -> Disk -> Network

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::{RwLock, Semaphore, mpsc};
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
    semaphore: Arc<Semaphore>,
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
    pub fn new(
        config: ImageLoaderConfig,
        event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
        disk_cache: Arc<DiskImageCache>,
    ) -> CacheResult<Self> {
        let memory_cache = Arc::new(MemoryImageCache::new(config.memory_cache_size));

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| CacheError::NetworkError(format!("Failed to create HTTP client: {e}")))?;

        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));

        Ok(Self {
            memory_cache,
            disk_cache,
            pending_loads: Arc::new(RwLock::new(HashSet::new())),
            event_tx,
            config,
            http_client,
            semaphore,
        })
    }

    /// Creates a loader with default configuration.
    ///
    /// # Errors
    /// Returns error if disk cache or HTTP client cannot be created.
    pub async fn with_defaults(
        event_tx: mpsc::UnboundedSender<ImageLoadedEvent>,
    ) -> CacheResult<Self> {
        let disk_cache = Arc::new(DiskImageCache::default_location().await?);
        Self::new(ImageLoaderConfig::default(), event_tx, disk_cache)
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

        let (bytes, _content_type) = self.download(&optimized_url).await?;

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
            semaphore: self.semaphore.clone(),
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

            let _permit = match loader.semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    error!("Semaphore closed, cancelling load");
                    return;
                }
            };

            let result = loader.load_image(&id, &url).await;

            drop(_permit);

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
    async fn download(&self, url: &str) -> CacheResult<(Vec<u8>, Option<String>)> {
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

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let bytes = response
            .bytes()
            .await
            .map_err(|e| CacheError::NetworkError(format!("Failed to read body: {e}")))?;

        Ok((bytes.to_vec(), content_type))
    }

    /// Exports an image to a temporary file for external viewing.
    ///
    /// This method ensures the image is cached, then copies it to a temporary file
    /// with the correct extension derived from the content type or URL.
    /// The file is placed in a `view` subdirectory of the cache to avoid cluttering /tmp.
    ///
    /// # Errors
    /// Returns error if download fails or file I/O fails.
    pub async fn export_for_viewing(
        &self,
        id: &ImageId,
        url: &str,
    ) -> CacheResult<std::path::PathBuf> {
        let (bytes, content_type) = if let Some(cached_bytes) = self.disk_cache.get_bytes(id).await
        {
            (cached_bytes, None)
        } else {
            let optimized_url = optimize_cdn_url_default(url);
            let (bytes, ctype) = self.download(&optimized_url).await?;
            let _ = self.disk_cache.put_bytes(id, &bytes).await;
            (bytes, ctype)
        };

        let ext = if let Some(ctype) = content_type {
            match ctype.as_str() {
                "image/jpeg" => "jpg",
                "image/gif" => "gif",
                "image/webp" => "webp",
                _ => "png",
            }
        } else if url.contains(".png") {
            "png"
        } else if url.contains(".jpg") || url.contains(".jpeg") {
            "jpg"
        } else if url.contains(".gif") {
            "gif"
        } else if url.contains(".webp") {
            "webp"
        } else {
            "png"
        };

        let temp_dir = std::env::temp_dir().join("oxicord").join("view");
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to create temp view dir: {e}")))?;

        let filename = format!("{}.{}", id.as_str(), ext);
        let path = temp_dir.join(filename);

        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to write export file: {e}")))?;

        Ok(path)
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
    semaphore: Arc<Semaphore>,
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
        let decoded =
            tokio::task::spawn_blocking(move || -> Result<image::DynamicImage, String> {
                let img = image::load_from_memory(&bytes_clone)
                    .map_err(|e| format!("Decode failed: {e}"))?;

                if img.width() > 800 {
                    Ok(img.resize(800, 600, image::imageops::FilterType::Lanczos3))
                } else {
                    Ok(img)
                }
            })
            .await
            .map_err(|e| format!("Decode task panicked: {e}"))??;

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
    async fn test_loader_creation() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, _rx) = mpsc::unbounded_channel();
        let temp_dir = tempfile::TempDir::new()?;
        let disk_cache =
            Arc::new(DiskImageCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).await?);

        let loader = ImageLoader::new(ImageLoaderConfig::default(), tx, disk_cache);
        assert!(loader.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_pending_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, _rx) = mpsc::unbounded_channel();
        let temp_dir = tempfile::TempDir::new()?;
        let disk_cache =
            Arc::new(DiskImageCache::new(temp_dir.path().to_path_buf(), 1024 * 1024).await?);

        let loader = ImageLoader::new(ImageLoaderConfig::default(), tx, disk_cache)?;

        assert_eq!(loader.pending_count().await, 0);
        Ok(())
    }
}
