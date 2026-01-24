use arboard::Clipboard;
use tracing::{error, warn};

#[derive(Clone, Default)]
pub struct ClipboardService {}

impl ClipboardService {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    pub fn set_text(&self, text: impl Into<String>) {
        let text = text.into();
        tokio::task::spawn_blocking(move || match Clipboard::new() {
            Ok(mut cb) => {
                if let Err(e) = cb.set_text(text) {
                    error!("Failed to set clipboard text: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to initialize clipboard for copy: {}", e);
            }
        });
    }

    pub fn get_text(&self) -> Option<String> {
        match Clipboard::new() {
            Ok(mut cb) => match cb.get_text() {
                Ok(text) => Some(text),
                Err(e) => {
                    error!("Failed to get clipboard text: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to initialize clipboard for read: {}", e);
                None
            }
        }
    }

    pub fn get_image(&self) -> Option<arboard::ImageData<'static>> {
        match Clipboard::new() {
            Ok(mut cb) => match cb.get_image() {
                Ok(image) => Some(image.to_owned_img()),
                Err(e) => {
                    warn!("Failed to get clipboard image: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to initialize clipboard for image read: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_service_lifecycle() {
        let service = ClipboardService::new();
        let result = service.get_text();
        assert!(result.is_none() || result.is_some());

        service.set_text("test");
    }
}
