//! Domain types for image handling.

use std::sync::Arc;

/// Unique identifier for a cached image.
/// Generated from a hash of the URL or Discord attachment ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageId(pub String);

impl ImageId {
    /// Creates a new `ImageId` from any string-like input.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Creates an `ImageId` from a URL by hashing it.
    #[must_use]
    pub fn from_url(url: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let result = hasher.finalize();
        Self(hex::encode(&result[..16]))
    }

    /// Returns the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ImageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ImageId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ImageId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Status of an image in the loading pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImageStatus {
    /// Image loading has not started.
    #[default]
    NotStarted,
    /// Image is being downloaded from the network.
    Downloading,
    /// Image is being decoded (CPU-intensive).
    Decoding,
    /// Image is fully loaded and ready for display.
    Ready,
    /// Image loading failed with an error message.
    Failed(String),
}

impl ImageStatus {
    /// Returns true if the image is ready for rendering.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns true if the image is currently being loaded.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Downloading | Self::Decoding)
    }

    /// Returns true if loading failed.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns true if loading hasn't started yet.
    #[must_use]
    pub const fn is_not_started(&self) -> bool {
        matches!(self, Self::NotStarted)
    }
}

/// Metadata about an image attachment.
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Unique identifier for caching.
    pub id: ImageId,
    /// Original URL of the image.
    pub url: String,
    /// Discord attachment ID if applicable.
    pub attachment_id: Option<String>,
    /// Original filename.
    pub filename: String,
    /// Content type (e.g., "image/png").
    pub content_type: Option<String>,
    /// File size in bytes.
    pub size: u64,
    /// Original width if known.
    pub width: Option<u32>,
    /// Original height if known.
    pub height: Option<u32>,
}

impl ImageMetadata {
    /// Creates new image metadata from an attachment URL.
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        attachment_id: Option<String>,
        filename: impl Into<String>,
        size: u64,
    ) -> Self {
        let url = url.into();
        let id = ImageId::from_url(&url);
        Self {
            id,
            url,
            attachment_id,
            filename: filename.into(),
            content_type: None,
            size,
            width: None,
            height: None,
        }
    }

    /// Sets the content type.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets the dimensions.
    #[must_use]
    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Returns true if this appears to be an image based on content type.
    #[must_use]
    pub fn is_image(&self) -> bool {
        self.content_type
            .as_ref()
            .is_some_and(|ct| ct.starts_with("image/"))
    }
}

/// Result of an image load operation.
#[derive(Debug, Clone)]
pub struct LoadedImage {
    /// The image identifier.
    pub id: ImageId,
    /// The decoded image data.
    pub image: Arc<image::DynamicImage>,
    /// Source of the image (cache, disk, network).
    pub source: ImageSource,
}

/// Where an image was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    /// Loaded from in-memory LRU cache.
    MemoryCache,
    /// Loaded from disk cache.
    DiskCache,
    /// Downloaded from network.
    Network,
}

impl std::fmt::Display for ImageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MemoryCache => write!(f, "memory"),
            Self::DiskCache => write!(f, "disk"),
            Self::Network => write!(f, "network"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_id_from_url() {
        let url = "https://cdn.discordapp.com/attachments/123/456/image.png";
        let id = ImageId::from_url(url);
        assert!(!id.0.is_empty());
        assert_eq!(id.0.len(), 32);
    }

    #[test]
    fn test_image_id_consistency() {
        let url = "https://example.com/image.png";
        let id1 = ImageId::from_url(url);
        let id2 = ImageId::from_url(url);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_image_status_transitions() {
        let status = ImageStatus::NotStarted;
        assert!(status.is_not_started());
        assert!(!status.is_loading());

        let status = ImageStatus::Downloading;
        assert!(status.is_loading());
        assert!(!status.is_ready());

        let status = ImageStatus::Ready;
        assert!(status.is_ready());
        assert!(!status.is_failed());

        let status = ImageStatus::Failed("error".to_string());
        assert!(status.is_failed());
    }

    #[test]
    fn test_image_metadata_creation() {
        let meta = ImageMetadata::new(
            "https://example.com/test.png",
            Some("123456".to_string()),
            "test.png",
            1024,
        )
        .with_content_type("image/png")
        .with_dimensions(800, 600);

        assert!(meta.is_image());
        assert_eq!(meta.width, Some(800));
        assert_eq!(meta.height, Some(600));
    }
}
