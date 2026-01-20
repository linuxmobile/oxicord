//! Image attachment state for message rendering.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::domain::entities::{ImageId, ImageStatus};

pub const MAX_IMAGE_HEIGHT: u16 = 20;
const MAX_IMAGE_WIDTH: u32 = 800;
pub const LOAD_BUFFER: usize = 5;

pub struct ImageAttachment {
    pub id: ImageId,
    pub url: String,
    pub image: Option<Arc<image::DynamicImage>>,
    pub protocol: Option<StatefulProtocol>,
    pub last_width: u16,
    pub status: ImageStatus,
    rendered_height: u16,
    rendered_width: u16,
    pub last_area: Option<Rect>,
    pub last_hash: u64,
    protocol_version: u64,
}

impl ImageAttachment {
    #[must_use]
    pub fn new(id: ImageId, url: String) -> Self {
        Self {
            id,
            url,
            image: None,
            protocol: None,
            last_width: 0,
            status: ImageStatus::NotStarted,
            rendered_height: 0,
            rendered_width: 0,
            last_area: None,
            last_hash: 0,
            protocol_version: 0,
        }
    }

    #[must_use]
    pub fn from_attachment(attachment: &crate::domain::entities::Attachment) -> Option<Self> {
        if !attachment.is_image() {
            return None;
        }

        let id = ImageId::from_url(attachment.url());
        Some(Self::new(id, attachment.url().to_string()))
    }

    pub fn set_loaded(&mut self, image: Arc<image::DynamicImage>) {
        self.image = Some(image);
        self.status = ImageStatus::Ready;
        self.protocol = None;
        self.last_width = 0;
        self.rendered_height = 0;
        self.rendered_width = 0;
    }

    pub fn set_downloading(&mut self) {
        self.status = ImageStatus::Downloading;
    }

    pub fn set_failed(&mut self, error: String) {
        self.status = ImageStatus::Failed(error);
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.image.is_some() && self.status.is_ready()
    }

    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.status.is_loading()
    }

    #[must_use]
    pub const fn needs_load(&self) -> bool {
        self.status.is_not_started()
    }

    pub fn update_state_and_check_redraw(&mut self, new_area: Rect) -> bool {
        let mut hasher = DefaultHasher::new();
        self.id.hash(&mut hasher);

        if let Some(img) = &self.image {
            img.width().hash(&mut hasher);
            img.height().hash(&mut hasher);
            img.as_bytes().len().hash(&mut hasher);
        }
        self.protocol_version.hash(&mut hasher);

        let current_hash = hasher.finish();

        if self.last_area == Some(new_area) && self.last_hash == current_hash {
            return false;
        }

        self.last_area = Some(new_area);
        self.last_hash = current_hash;
        true
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn update_protocol_if_needed(&mut self, picker: &Picker, terminal_width: u16) -> bool {
        let needs_update =
            self.image.is_some() && (self.protocol.is_none() || self.last_width != terminal_width);

        if !needs_update {
            return false;
        }

        let Some(ref image) = self.image else {
            return false;
        };

        let (font_width, font_height) = picker.font_size();

        if font_width == 0 || font_height == 0 {
            return false;
        }

        let max_height_pixels = u32::from(MAX_IMAGE_HEIGHT) * u32::from(font_height);
        let available_width = terminal_width.saturating_sub(10);
        let max_width_pixels = u32::from(available_width) * u32::from(font_width);

        let (width, height) = (image.width(), image.height());

        let scale_w = f64::from(max_width_pixels.min(MAX_IMAGE_WIDTH)) / f64::from(width);
        let scale_h = f64::from(max_height_pixels) / f64::from(height);
        let scale = scale_w.min(scale_h).min(1.0);

        let (final_width, final_height) = if scale < 1.0 {
            let new_width = (f64::from(width) * scale) as u32;
            let new_height = (f64::from(height) * scale) as u32;
            (new_width.max(1), new_height.max(1))
        } else {
            (width, height)
        };

        let filter = if picker.protocol_type() == ProtocolType::Sixel {
            image::imageops::FilterType::Nearest
        } else {
            image::imageops::FilterType::Triangle
        };

        let resized_image = if scale < 1.0 {
            image.resize(final_width, final_height, filter)
        } else {
            (**image).clone()
        };

        self.rendered_height = (final_height as f32 / f32::from(font_height)).ceil() as u16;
        self.rendered_width = (final_width as f32 / f32::from(font_width)).ceil() as u16;
        self.protocol = Some(picker.new_resize_protocol(resized_image));
        self.last_width = terminal_width;
        self.protocol_version = self.protocol_version.wrapping_add(1);

        true
    }

    pub fn clear_protocol(&mut self) {
        self.protocol = None;
        self.last_width = 0;
    }

    #[must_use]
    pub const fn height(&self) -> u16 {
        if self.rendered_height > 0 {
            self.rendered_height
        } else if self.image.is_some() || self.status.is_loading() {
            MAX_IMAGE_HEIGHT
        } else {
            0
        }
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.rendered_width
    }
}

impl std::fmt::Debug for ImageAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageAttachment")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("has_image", &self.image.is_some())
            .field("has_protocol", &self.protocol.is_some())
            .field("last_width", &self.last_width)
            .field("status", &self.status)
            .field("rendered_height", &self.rendered_height)
            .field("rendered_width", &self.rendered_width)
            .finish()
    }
}

pub struct ImageManager {
    picker: Picker,
    current_width: u16,
}

impl ImageManager {
    #[must_use]
    pub fn new() -> Self {
        let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        let caps = picker.capabilities();
        let has_sixel = caps
            .iter()
            .any(|c| matches!(c, ratatui_image::picker::Capability::Sixel));
        let has_kitty = caps
            .iter()
            .any(|c| matches!(c, ratatui_image::picker::Capability::Kitty));

        if has_sixel && !has_kitty && picker.protocol_type() == ProtocolType::Halfblocks {
            picker.set_protocol_type(ProtocolType::Sixel);
        }

        Self {
            picker,
            current_width: 0,
        }
    }

    #[must_use]
    pub fn halfblocks() -> Self {
        let picker = Picker::halfblocks();
        Self {
            picker,
            current_width: 0,
        }
    }

    #[deprecated(since = "0.2.0", note = "use `new()` instead")]
    #[must_use]
    pub fn from_query() -> Self {
        Self::new()
    }

    #[must_use]
    pub fn protocol_type(&self) -> ProtocolType {
        self.picker.protocol_type()
    }

    #[must_use]
    pub const fn picker(&self) -> &Picker {
        &self.picker
    }

    pub const fn set_width(&mut self, width: u16) {
        self.current_width = width;
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.current_width
    }

    pub fn update_visible_protocols(&self, attachments: &mut [&mut ImageAttachment], width: u16) {
        for attachment in attachments {
            attachment.update_protocol_if_needed(&self.picker, width);
        }
    }

    pub fn clear_distant_protocols(
        &self,
        attachments: &mut [&mut ImageAttachment],
        visible_start: usize,
        visible_end: usize,
    ) {
        let buffer_start = visible_start.saturating_sub(LOAD_BUFFER);
        let buffer_end = visible_end + LOAD_BUFFER;

        for (idx, attachment) in attachments.iter_mut().enumerate() {
            if idx < buffer_start || idx > buffer_end {
                attachment.clear_protocol();
            }
        }
    }

    #[must_use]
    pub fn collect_needed_loads(
        attachments: &[ImageAttachment],
        visible_start: usize,
        visible_end: usize,
    ) -> Vec<(ImageId, String)> {
        let buffer_start = visible_start.saturating_sub(LOAD_BUFFER);
        let buffer_end = visible_end + LOAD_BUFFER;

        attachments
            .iter()
            .enumerate()
            .filter(|(idx, attachment)| {
                *idx >= buffer_start && *idx <= buffer_end && attachment.needs_load()
            })
            .map(|(_, attachment)| (attachment.id.clone(), attachment.url.clone()))
            .collect()
    }
}

impl Default for ImageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_attachment_creation() {
        let id = ImageId::new("test");
        let attachment =
            ImageAttachment::new(id.clone(), "https://example.com/img.png".to_string());

        assert_eq!(attachment.id, id);
        assert!(!attachment.is_ready());
        assert!(attachment.needs_load());
    }

    #[test]
    fn test_image_attachment_loading_flow() {
        let id = ImageId::new("test");
        let mut attachment = ImageAttachment::new(id, "https://example.com/img.png".to_string());

        assert!(attachment.needs_load());

        attachment.set_downloading();
        assert!(attachment.is_loading());
        assert!(!attachment.needs_load());

        let img = Arc::new(image::DynamicImage::new_rgb8(100, 100));
        attachment.set_loaded(img);
        assert!(attachment.is_ready());
        assert!(!attachment.is_loading());
    }

    #[test]
    fn test_image_attachment_failure() {
        let id = ImageId::new("test");
        let mut attachment = ImageAttachment::new(id, "https://example.com/img.png".to_string());

        attachment.set_failed("Network error".to_string());
        assert!(attachment.status.is_failed());
        assert!(!attachment.is_ready());
    }

    #[test]
    fn test_image_manager_creation() {
        let manager = ImageManager::new();
        assert_eq!(manager.width(), 0);
    }

    #[test]
    fn test_collect_needed_loads() {
        let attachments = vec![
            ImageAttachment::new(ImageId::new("0"), "url0".to_string()),
            ImageAttachment::new(ImageId::new("1"), "url1".to_string()),
            ImageAttachment::new(ImageId::new("2"), "url2".to_string()),
            ImageAttachment::new(ImageId::new("3"), "url3".to_string()),
            ImageAttachment::new(ImageId::new("4"), "url4".to_string()),
        ];

        let needed = ImageManager::collect_needed_loads(&attachments, 1, 2);
        assert!(needed.len() >= 2);
    }
}
