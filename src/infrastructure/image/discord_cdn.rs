//! Discord CDN URL optimization.

/// Default target width for optimized images.
pub const DEFAULT_WIDTH: u32 = 800;

/// Default target height for optimized images.
pub const DEFAULT_HEIGHT: u32 = 600;

/// Optimizes a Discord CDN URL by adding format and size parameters.
/// This significantly reduces bandwidth usage and RAM consumption.
///
/// # Arguments
/// * `url` - The original image URL
/// * `width` - Target width (default: 800)
/// * `height` - Target height (default: 600)
///
/// # Returns
/// The optimized URL with query parameters, or the original URL if not a Discord CDN URL.
#[must_use]
pub fn optimize_cdn_url(url: &str, width: u32, height: u32) -> String {
    if !url.contains("cdn.discordapp.com") && !url.contains("media.discordapp.net") {
        return url.to_string();
    }

    let (base_url, existing_params) = if let Some(idx) = url.find('?') {
        (&url[..idx], Some(&url[idx + 1..]))
    } else {
        (url, None)
    };

    let mut params = vec![
        format!("format=webp"),
        format!("width={width}"),
        format!("height={height}"),
    ];

    if let Some(existing) = existing_params {
        for param in existing.split('&') {
            let key = param.split('=').next().unwrap_or("");
            if !["format", "width", "height", "size", "quality"].contains(&key) {
                params.push(param.to_string());
            }
        }
    }

    format!("{}?{}", base_url, params.join("&"))
}

/// Optimizes a URL with default dimensions.
#[must_use]
pub fn optimize_cdn_url_default(url: &str) -> String {
    optimize_cdn_url(url, DEFAULT_WIDTH, DEFAULT_HEIGHT)
}

/// Extracts the attachment ID from a Discord CDN URL.
#[must_use]
pub fn extract_attachment_id(url: &str) -> Option<String> {
    if !url.contains("discordapp.com") && !url.contains("discordapp.net") {
        return None;
    }

    let path = url.split("attachments/").nth(1)?;

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 {
        Some(parts[1].split('?').next()?.to_string())
    } else {
        None
    }
}

/// Checks if a URL is a Discord CDN URL.
#[must_use]
pub fn is_discord_cdn_url(url: &str) -> bool {
    url.contains("cdn.discordapp.com") || url.contains("media.discordapp.net")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_basic_url() {
        let url = "https://cdn.discordapp.com/attachments/123/456/image.png";
        let optimized = optimize_cdn_url_default(url);

        assert!(optimized.contains("format=webp"));
        assert!(optimized.contains("width=800"));
        assert!(optimized.contains("height=600"));
    }

    #[test]
    fn test_optimize_with_existing_params() {
        let url = "https://cdn.discordapp.com/attachments/123/456/image.png?ex=abc123";
        let optimized = optimize_cdn_url_default(url);

        assert!(optimized.contains("format=webp"));
        assert!(optimized.contains("ex=abc123"));
    }

    #[test]
    fn test_non_discord_url_unchanged() {
        let url = "https://example.com/image.png";
        let optimized = optimize_cdn_url_default(url);

        assert_eq!(optimized, url);
    }

    #[test]
    fn test_extract_attachment_id() {
        let url = "https://cdn.discordapp.com/attachments/123/456789/image.png";
        let id = extract_attachment_id(url);

        assert_eq!(id, Some("456789".to_string()));
    }

    #[test]
    fn test_extract_attachment_id_with_params() {
        let url = "https://cdn.discordapp.com/attachments/123/456789/image.png?format=webp";
        let id = extract_attachment_id(url);

        assert_eq!(id, Some("456789".to_string()));
    }

    #[test]
    fn test_is_discord_cdn_url() {
        assert!(is_discord_cdn_url(
            "https://cdn.discordapp.com/attachments/1/2/img.png"
        ));
        assert!(is_discord_cdn_url(
            "https://media.discordapp.net/attachments/1/2/img.png"
        ));
        assert!(!is_discord_cdn_url("https://example.com/image.png"));
    }

    #[test]
    fn test_custom_dimensions() {
        let url = "https://cdn.discordapp.com/attachments/123/456/image.png";
        let optimized = optimize_cdn_url(url, 400, 300);

        assert!(optimized.contains("width=400"));
        assert!(optimized.contains("height=300"));
    }
}
