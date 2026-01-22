//! Disk-based image cache for persistence across sessions.

use std::path::PathBuf;
use std::sync::Arc;

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

        let cache = Self {
            cache_dir,
            max_size,
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

        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to create cache file: {e}")))?;

        file.write_all(bytes)
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to write cache file: {e}")))?;

        file.flush()
            .await
            .map_err(|e| CacheError::IoError(format!("Failed to flush cache file: {e}")))?;

        debug!(id = %id, path = %path.display(), size = bytes.len(), "Stored image in disk cache");

        self.cleanup_if_needed().await;

        Ok(())
    }

    /// Removes an image from disk cache.
    pub async fn evict(&self, id: &ImageId) {
        let path = self.cache_path(id);
        if let Err(e) = fs::remove_file(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(id = %id, error = %e, "Failed to evict from disk cache");
            }
        } else {
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

        debug!("Cleared disk cache");
        Ok(())
    }

    /// Returns the current cache size in bytes.
    pub async fn current_size(&self) -> u64 {
        let mut total = 0u64;

        let Ok(mut entries) = fs::read_dir(&self.cache_dir).await else {
            return 0;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await
                && meta.is_file()
            {
                total += meta.len();
            }
        }

        total
    }

    /// Returns the number of cached files.
    pub async fn len(&self) -> usize {
        let Ok(mut entries) = fs::read_dir(&self.cache_dir).await else {
            return 0;
        };

        let mut count = 0;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "img") {
                count += 1;
            }
        }

        count
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

        let mut freed = 0u64;
        let target = current_size - self.max_size + (self.max_size / 10);

        for (path, _, size) in files {
            if freed >= target {
                break;
            }

            if let Err(e) = fs::remove_file(&path).await {
                warn!(path = %path.display(), error = %e, "Failed to remove old cache file");
            } else {
                debug!(path = %path.display(), "Removed old cache file");
                freed += size;
            }
        }

        debug!(freed = freed, "Disk cache cleanup complete");
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
}
