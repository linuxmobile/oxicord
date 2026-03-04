use crate::application::services::url_extractor::UrlExtractor;
use crate::domain::entities::Message;

#[derive(Debug, PartialEq, Eq)]
pub enum MessageContentAction {
    OpenImages,
    OpenLink(String),
    None,
}

impl MessageContentAction {
    #[must_use]
    pub fn label(&self) -> Option<&'static str> {
        match self {
            Self::OpenImages => Some("Open Image"),
            Self::OpenLink(_) => Some("Open Link"),
            Self::None => None,
        }
    }
}

pub struct MessageContentService;

impl MessageContentService {
    #[must_use]
    pub fn resolve(message: &Message) -> MessageContentAction {
        if message
            .attachments()
            .iter()
            .any(crate::domain::entities::Attachment::is_image)
        {
            return MessageContentAction::OpenImages;
        }

        let image_urls = UrlExtractor::extract_image_urls(message.content());
        if !image_urls.is_empty() {
            return MessageContentAction::OpenImages;
        }

        if let Some(url) = UrlExtractor::extract_first_url(message.content()) {
            return MessageContentAction::OpenLink(url);
        }

        MessageContentAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{
        Attachment, ChannelId, Message, MessageAuthor, MessageId, MessageKind,
    };
    use chrono::Local;

    fn create_dummy_message(content: &str, attachments: Vec<Attachment>) -> Message {
        Message::new(
            MessageId::from(1),
            ChannelId::from(1),
            MessageAuthor {
                id: "1".to_string(),
                username: "test".to_string(),
                discriminator: "0000".to_string(),
                avatar: None,
                bot: false,
                global_name: None,
                color: None,
            },
            content.to_string(),
            Local::now(),
            MessageKind::Default,
        )
        .with_attachments(attachments)
    }

    #[test]
    fn test_resolve_image_attachment() {
        let attachment =
            Attachment::new("1", "image.png", 100, "http://url").with_content_type("image/png");
        let message = create_dummy_message("text", vec![attachment]);

        assert_eq!(
            MessageContentService::resolve(&message),
            MessageContentAction::OpenImages
        );
    }

    #[test]
    fn test_resolve_inline_image() {
        let message = create_dummy_message("Check this https://example.com/image.png", vec![]);
        assert_eq!(
            MessageContentService::resolve(&message),
            MessageContentAction::OpenImages
        );
    }

    #[test]
    fn test_resolve_link() {
        let message = create_dummy_message("Check this https://google.com", vec![]);
        let action = MessageContentService::resolve(&message);
        match action {
            MessageContentAction::OpenLink(url) => assert_eq!(url, "https://google.com"),
            _ => panic!("Expected OpenLink"),
        }
    }

    #[test]
    fn test_resolve_none() {
        let message = create_dummy_message("Hello world", vec![]);
        assert_eq!(
            MessageContentService::resolve(&message),
            MessageContentAction::None
        );
    }

    #[test]
    fn test_priority_image_over_link() {
        let attachment =
            Attachment::new("1", "image.png", 100, "http://url").with_content_type("image/png");
        let message = create_dummy_message("https://google.com", vec![attachment]);
        assert_eq!(
            MessageContentService::resolve(&message),
            MessageContentAction::OpenImages
        );
    }
}
