//! Disk-based image cache for persistence across sessions.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, trace, warn};

use crate::domain::entities::ImageId;
use crate::domain::ports::{CacheError, CacheResult};

/// Maximum disk cache size in bytes (200 MB default).
pub const DEFAULT_MAX_CACHE_SIZE: u64 = 200 * 1024 * 1024;

/// Disk-based image cache that persists raw image bytes.
pub struct DiskImageCache {
    cache_dir: PathBuf,
    max_size: u64,
    current_size: AtomicU64,
    item_count: AtomicUsize,
}

impl DiskImageCache {
    /// Creates a new disk cache in the specified directory.
    ///
    /// # Errors
    /// Returns error if cache directory cannot be created.
    pub async fn new(cache_dir: PathBuf, max_size: u64) -> CacheResult<Self> {
        fs::create_dir_all(&cache_dir)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to create cache dir: {e}")))?;
        let mut total_size = 0u64;
        let mut count = 0usize;

        let mut entries = fs::read_dir(&cache_dir)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to read cache dir: {e}")))?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "img")
                && let Ok(meta) = entry.metadata().await
            {
                total_size += meta.len();
                count += 1;
            }
        }

        let cache = Self {
            cache_dir,
            max_size,
            current_size: AtomicU64::new(total_size),
            item_count: AtomicUsize::new(count),
        };

        cache.cleanup_if_needed().await;

        Ok(cache)
    }

    /// Creates a cache in the default location (~/.cache/oxicord/images/).
    ///
    /// # Errors
    /// Returns error if cache directory cannot be created.
    pub async fn default_location() -> CacheResult<Self> {
        let cache_dir = dirs_cache_path();
        Self::new(cache_dir, DEFAULT_MAX_CACHE_SIZE).await
    }

    /// Returns the path for a cached image.
    fn cache_path(&self, id: &ImageId) -> PathBuf {
        self.cache_dir.join(format!("{}.img", id.as_str()))
    }

    /// Gets raw image bytes from disk cache.
    pub async fn get_bytes(&self, id: &ImageId) -> Option<Vec<u8>> {
        let path = self.cache_path(id);
        if let Ok(bytes) = fs::read(&path).await {
            trace!(id = %id, path = %path.display(), "Disk cache hit");
            let _ = fs::File::open(&path).await;
            Some(bytes)
        } else {
            trace!(id = %id, "Disk cache miss");
            None
        }
    }

    /// Loads and decodes an image from disk cache.
    pub async fn get(&self, id: &ImageId) -> Option<Arc<image::DynamicImage>> {
        let bytes = self.get_bytes(id).await?;

        let result = tokio::task::spawn_blocking(move || image::load_from_memory(&bytes)).await;

        match result {
            Ok(Ok(img)) => {
                debug!(id = %id, "Decoded image from disk cache");
                Some(Arc::new(img))
            }
            Ok(Err(e)) => {
                warn!(id = %id, error = %e, "Failed to decode cached image");
                None
            }
            Err(e) => {
                error!(id = %id, error = %e, "Decode task panicked");
                None
            }
        }
    }

    /// Stores raw bytes in the disk cache.
    ///
    /// # Errors
    /// Returns error if file cannot be created or written.
    pub async fn put_bytes(&self, id: &ImageId, bytes: &[u8]) -> CacheResult<()> {
        let path = self.cache_path(id);

        let old_size = fs::metadata(&path).await.map(|m| m.len()).ok();

        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to create cache file: {e}")))?;

        file.write_all(bytes)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to write cache file: {e}")))?;

        file.flush()
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to flush cache file: {e}")))?;
        let new_size = bytes.len() as u64;
        if let Some(old) = old_size {
            if new_size > old {
                self.current_size
                    .fetch_add(new_size - old, Ordering::Relaxed);
            } else {
                self.current_size
                    .fetch_sub(old - new_size, Ordering::Relaxed);
            }
        } else {
            self.current_size.fetch_add(new_size, Ordering::Relaxed);
            self.item_count.fetch_add(1, Ordering::Relaxed);
        }

        debug!(id = %id, path = %path.display(), size = bytes.len(), "Stored image in disk cache");

        self.cleanup_if_needed().await;

        Ok(())
    }

    /// Removes an image from disk cache.
    pub async fn evict(&self, id: &ImageId) {
        let path = self.cache_path(id);
        let size = fs::metadata(&path).await.map(|m| m.len()).ok();
        if let Err(e) = fs::remove_file(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(id = %id, error = %e, "Failed to evict from disk cache");
            }
        } else if let Some(s) = size {
            self.current_size.fetch_sub(s, Ordering::Relaxed);
            self.item_count.fetch_sub(1, Ordering::Relaxed);
            debug!(id = %id, "Evicted from disk cache");
        }
    }

    /// Clears the entire disk cache.
    ///
    /// # Errors
    /// Returns error if cache directory cannot be read.
    pub async fn clear(&self) -> CacheResult<()> {
        let mut entries = fs::read_dir(&self.cache_dir)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to read cache dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to read entry: {e}")))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "img")
                && fs::remove_file(&path).await.is_err()
            {
                warn!(path = %path.display(), "Failed to remove cache file");
            }
        }
        self.current_size.store(0, Ordering::Relaxed);
        self.item_count.store(0, Ordering::Relaxed);
        debug!("Cleared disk cache");
        Ok(())
    }

    /// Returns the current cache size in bytes.
    #[allow(clippy::unused_async)]
    pub async fn current_size(&self) -> u64 {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Returns the number of cached files.
    #[allow(clippy::unused_async)]
    pub async fn len(&self) -> usize {
        self.item_count.load(Ordering::Relaxed)
    }

    /// Returns true if the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Cleans up old cache entries if over size limit.
    async fn cleanup_if_needed(&self) {
        let current_size = self.current_size().await;
        if current_size <= self.max_size {
            return;
        }

        debug!(
            current_size = current_size,
            max_size = self.max_size,
            "Disk cache over limit, cleaning up"
        );

        let Ok(mut entries) = fs::read_dir(&self.cache_dir).await else {
            return;
        };

        let mut files: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "img") {
                continue;
            }

            if let Ok(meta) = entry.metadata().await {
                let accessed = meta.accessed().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                files.push((path, accessed, meta.len()));
            }
        }

        files.sort_by_key(|(_, time, _)| *time);

        let mut freed_size = 0u64;
        let mut freed_count = 0usize;
        let target = current_size - self.max_size + (self.max_size / 10);

        for (path, _, size) in files {
            if freed_size >= target {
                break;
            }

            if let Err(e) = fs::remove_file(&path).await {
                warn!(path = %path.display(), error = %e, "Failed to remove old cache file");
            } else {
                debug!(path = %path.display(), "Removed old cache file");
                freed_size += size;
                freed_count += 1;
            }
        }
        self.current_size.fetch_sub(freed_size, Ordering::Relaxed);
        self.item_count.fetch_sub(freed_count, Ordering::Relaxed);

        debug!(
            freed_size = freed_size,
            freed_count = freed_count,
            "Disk cache cleanup complete"
        );
    }

    /// Checks if an image is cached.
    pub async fn contains(&self, id: &ImageId) -> bool {
        let path = self.cache_path(id);
        fs::try_exists(&path).await.unwrap_or(false)
    }
}

/// Returns the default cache directory path.
fn dirs_cache_path() -> PathBuf {
    directories::ProjectDirs::from("com", "linuxmobile", "oxicord").map_or_else(
        || {
            std::env::temp_dir()
                .join("oxicord")
                .join("cache")
                .join("images")
        },
        |dirs| dirs.cache_dir().join("images"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_cache() -> (DiskImageCache, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskImageCache::new(temp_dir.path().to_path_buf(), 1024 * 1024)
            .await
            .unwrap();
        (cache, temp_dir)
    }

    #[tokio::test]
    async fn test_put_and_get_bytes() {
        let (cache, _temp) = create_test_cache().await;
        let id = ImageId::new("test1");
        let data = b"test image data";

        cache.put_bytes(&id, data).await.unwrap();
        let retrieved = cache.get_bytes(&id).await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), data);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let (cache, _temp) = create_test_cache().await;
        let id = ImageId::new("nonexistent");

        let result = cache.get_bytes(&id).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_evict() {
        let (cache, _temp) = create_test_cache().await;
        let id = ImageId::new("test1");

        cache.put_bytes(&id, b"test").await.unwrap();
        assert!(cache.contains(&id).await);

        cache.evict(&id).await;
        assert!(!cache.contains(&id).await);
    }

    #[tokio::test]
    async fn test_clear() {
        let (cache, _temp) = create_test_cache().await;

        cache
            .put_bytes(&ImageId::new("test1"), b"data1")
            .await
            .unwrap();
        cache
            .put_bytes(&ImageId::new("test2"), b"data2")
            .await
            .unwrap();

        assert_eq!(cache.len().await, 2);

        cache.clear().await.unwrap();
        assert_eq!(cache.len().await, 0);
    }
    #[tokio::test]
    async fn test_atomic_counters_sync() {
        let (cache, _temp) = create_test_cache().await;

        assert_eq!(cache.current_size().await, 0);
        assert_eq!(cache.len().await, 0);

        cache
            .put_bytes(&ImageId::new("test1"), b"hello")
            .await
            .unwrap();
        cache
            .put_bytes(&ImageId::new("test2"), b"world!")
            .await
            .unwrap();

        assert_eq!(cache.len().await, 2);
        assert_eq!(cache.current_size().await, 11);

        cache
            .put_bytes(&ImageId::new("test1"), b"hey")
            .await
            .unwrap();
        assert_eq!(cache.len().await, 2);
        assert_eq!(cache.current_size().await, 9);

        cache.evict(&ImageId::new("test2")).await;
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.current_size().await, 3);

        cache.clear().await.unwrap();
        assert_eq!(cache.len().await, 0);
        assert_eq!(cache.current_size().await, 0);
    }

    #[tokio::test]
    async fn test_cleanup_updates_counters() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskImageCache::new(temp_dir.path().to_path_buf(), 10)
            .await
            .unwrap();

        cache
            .put_bytes(&ImageId::new("test1"), b"123456")
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        cache
            .put_bytes(&ImageId::new("test2"), b"123456")
            .await
            .unwrap();

        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.current_size().await, 6);
    }
}
