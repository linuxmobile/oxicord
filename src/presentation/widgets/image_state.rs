//! Image attachment state for message rendering.

use std::sync::Arc;

use ratatui::layout::Rect;
use ratatui_image::Resize;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::domain::entities::{ImageId, ImageStatus};

pub const MAX_IMAGE_HEIGHT: u16 = 20;
pub const LOAD_BUFFER: usize = 5;

pub struct ImageAttachment {
    pub id: ImageId,
    pub url: String,
    pub image: Option<Arc<image::DynamicImage>>,
    pub protocol: Option<StatefulProtocol>,
    pub status: ImageStatus,
}

impl ImageAttachment {
    #[must_use]
    pub fn new(id: ImageId, url: String) -> Self {
        Self {
            id,
            url,
            image: None,
            protocol: None,
            status: ImageStatus::NotStarted,
        }
    }

    #[must_use]
    pub fn from_attachment(attachment: &crate::domain::entities::Attachment) -> Option<Self> {
        if !attachment.is_image() {
            return None;
        }

        let id = ImageId::from_url(&attachment.url);
        Some(Self::new(id, attachment.url.clone()))
    }

    pub fn set_loaded(&mut self, image: Arc<image::DynamicImage>) {
        self.image = Some(image);
        self.status = ImageStatus::Ready;
        self.protocol = None;
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

    pub fn update_protocol_if_needed(&mut self, picker: &Picker) {
        if self.protocol.is_some() {
            return;
        }

        if let Some(ref image) = self.image {
            self.protocol = Some(picker.new_resize_protocol((**image).clone()));
        }
    }

    pub fn clear_protocol(&mut self) {
        self.protocol = None;
    }

    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        if let Some(protocol) = &self.protocol {
            let area = Rect::new(0, 0, width, MAX_IMAGE_HEIGHT);
            return protocol.size_for(Resize::Fit(None), area).height;
        } else if self.image.is_some() || self.status.is_loading() {
            return 3;
        }
        0
    }

    #[must_use]
    pub fn width(&self, width: u16) -> u16 {
        if let Some(protocol) = &self.protocol {
            let area = Rect::new(0, 0, width, MAX_IMAGE_HEIGHT);
            return protocol.size_for(Resize::Fit(None), area).width;
        }
        0
    }
}

impl std::fmt::Debug for ImageAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageAttachment")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("has_image", &self.image.is_some())
            .field("has_protocol", &self.protocol.is_some())
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

pub struct ImageManager {
    picker: Picker,
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

        Self { picker }
    }

    #[must_use]
    pub fn halfblocks() -> Self {
        Self {
            picker: Picker::halfblocks(),
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

    pub const fn set_width(&mut self, _width: u16) {}

    #[must_use]
    pub const fn width(&self) -> u16 {
        0
    }

    pub fn update_visible_protocols(&self, attachments: &mut [&mut ImageAttachment]) {
        for attachment in attachments {
            attachment.update_protocol_if_needed(&self.picker);
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

        let memory_buffer = LOAD_BUFFER * 3;
        let memory_start = visible_start.saturating_sub(memory_buffer);
        let memory_end = visible_end + memory_buffer;

        for (idx, attachment) in attachments.iter_mut().enumerate() {
            if idx < buffer_start || idx > buffer_end {
                attachment.clear_protocol();
            }

            if (idx < memory_start || idx > memory_end) && attachment.image.is_some() {
                attachment.image = None;
                if matches!(attachment.status, ImageStatus::Ready) {
                    attachment.status = ImageStatus::NotStarted;
                }
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
    #[cfg(not(windows))]
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
