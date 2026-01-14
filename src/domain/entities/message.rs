use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{ChannelId, User};

/// Unique identifier for a Discord message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

impl MessageId {
    /// Returns the underlying u64 value.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for MessageId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<&str> for MessageId {
    fn from(value: &str) -> Self {
        Self(value.parse().unwrap_or(0))
    }
}

/// Discord message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum MessageKind {
    #[default]
    Default = 0,
    RecipientAdd = 1,
    RecipientRemove = 2,
    Call = 3,
    ChannelNameChange = 4,
    ChannelIconChange = 5,
    ChannelPinnedMessage = 6,
    UserJoin = 7,
    GuildBoost = 8,
    GuildBoostTier1 = 9,
    GuildBoostTier2 = 10,
    GuildBoostTier3 = 11,
    ChannelFollowAdd = 12,
    GuildDiscoveryDisqualified = 14,
    GuildDiscoveryRequalified = 15,
    GuildDiscoveryGracePeriodInitialWarning = 16,
    GuildDiscoveryGracePeriodFinalWarning = 17,
    ThreadCreated = 18,
    Reply = 19,
    ChatInputCommand = 20,
    ThreadStarterMessage = 21,
    GuildInviteReminder = 22,
    ContextMenuCommand = 23,
    AutoModerationAction = 24,
    RoleSubscriptionPurchase = 25,
    InteractionPremiumUpsell = 26,
    StageStart = 27,
    StageEnd = 28,
    StageSpeaker = 29,
    StageTopic = 31,
    GuildApplicationPremiumSubscription = 32,
}

impl From<u8> for MessageKind {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::RecipientAdd,
            2 => Self::RecipientRemove,
            3 => Self::Call,
            4 => Self::ChannelNameChange,
            5 => Self::ChannelIconChange,
            6 => Self::ChannelPinnedMessage,
            7 => Self::UserJoin,
            8 => Self::GuildBoost,
            9 => Self::GuildBoostTier1,
            10 => Self::GuildBoostTier2,
            11 => Self::GuildBoostTier3,
            12 => Self::ChannelFollowAdd,
            14 => Self::GuildDiscoveryDisqualified,
            15 => Self::GuildDiscoveryRequalified,
            16 => Self::GuildDiscoveryGracePeriodInitialWarning,
            17 => Self::GuildDiscoveryGracePeriodFinalWarning,
            18 => Self::ThreadCreated,
            19 => Self::Reply,
            20 => Self::ChatInputCommand,
            21 => Self::ThreadStarterMessage,
            22 => Self::GuildInviteReminder,
            23 => Self::ContextMenuCommand,
            24 => Self::AutoModerationAction,
            25 => Self::RoleSubscriptionPurchase,
            26 => Self::InteractionPremiumUpsell,
            27 => Self::StageStart,
            28 => Self::StageEnd,
            29 => Self::StageSpeaker,
            31 => Self::StageTopic,
            32 => Self::GuildApplicationPremiumSubscription,
            _ => Self::Default,
        }
    }
}

impl MessageKind {
    /// Returns true if this is a regular user message.
    #[must_use]
    pub fn is_regular(self) -> bool {
        matches!(self, Self::Default | Self::Reply)
    }

    /// Returns true if this is a system message.
    #[must_use]
    pub fn is_system(self) -> bool {
        !self.is_regular()
    }
}

/// Discord message attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Attachment {
    id: String,
    filename: String,
    size: u64,
    url: String,
    content_type: Option<String>,
}

#[allow(missing_docs)]
impl Attachment {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        filename: impl Into<String>,
        size: u64,
        url: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            filename: filename.into(),
            size,
            url: url.into(),
            content_type: None,
        }
    }

    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    #[must_use]
    pub fn is_image(&self) -> bool {
        self.content_type
            .as_ref()
            .is_some_and(|ct| ct.starts_with("image/"))
    }
}

/// Reference to another message (for replies).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct MessageReference {
    message_id: Option<MessageId>,
    channel_id: Option<ChannelId>,
}

#[allow(missing_docs)]
impl MessageReference {
    #[must_use]
    pub fn new(message_id: Option<MessageId>, channel_id: Option<ChannelId>) -> Self {
        Self {
            message_id,
            channel_id,
        }
    }

    #[must_use]
    pub fn message_id(&self) -> Option<MessageId> {
        self.message_id
    }

    #[must_use]
    pub fn channel_id(&self) -> Option<ChannelId> {
        self.channel_id
    }
}

/// Author of a Discord message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct MessageAuthor {
    id: String,
    username: String,
    discriminator: String,
    avatar: Option<String>,
    bot: bool,
}

#[allow(missing_docs)]
impl MessageAuthor {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        username: impl Into<String>,
        discriminator: impl Into<String>,
        avatar: Option<String>,
        bot: bool,
    ) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            discriminator: discriminator.into(),
            avatar,
            bot,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    #[must_use]
    pub fn discriminator(&self) -> &str {
        &self.discriminator
    }

    #[must_use]
    pub fn avatar(&self) -> Option<&str> {
        self.avatar.as_deref()
    }

    #[must_use]
    pub fn is_bot(&self) -> bool {
        self.bot
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.username
    }
}

impl From<User> for MessageAuthor {
    fn from(user: User) -> Self {
        Self {
            id: user.id().to_string(),
            username: user.username().to_string(),
            discriminator: user.discriminator().to_string(),
            avatar: user.avatar().map(String::from),
            bot: user.is_bot(),
        }
    }
}

/// Discord message entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Message {
    id: MessageId,
    channel_id: ChannelId,
    author: MessageAuthor,
    content: String,
    timestamp: DateTime<Utc>,
    edited_timestamp: Option<DateTime<Utc>>,
    kind: MessageKind,
    attachments: Vec<Attachment>,
    reference: Option<MessageReference>,
    referenced: Option<Box<Message>>,
    pinned: bool,
}

#[allow(missing_docs)]
impl Message {
    #[must_use]
    pub fn new(
        id: impl Into<MessageId>,
        channel_id: impl Into<ChannelId>,
        author: MessageAuthor,
        content: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            channel_id: channel_id.into(),
            author,
            content: content.into(),
            timestamp,
            edited_timestamp: None,
            kind: MessageKind::Default,
            attachments: Vec::new(),
            reference: None,
            referenced: None,
            pinned: false,
        }
    }

    #[must_use]
    pub fn with_kind(mut self, kind: MessageKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub fn with_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    #[must_use]
    pub fn with_reference(mut self, reference: MessageReference) -> Self {
        self.reference = Some(reference);
        self
    }

    #[must_use]
    pub fn with_referenced(mut self, message: Message) -> Self {
        self.referenced = Some(Box::new(message));
        self
    }

    #[must_use]
    pub fn with_edited_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.edited_timestamp = Some(timestamp);
        self
    }

    #[must_use]
    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    #[must_use]
    pub fn id(&self) -> MessageId {
        self.id
    }

    #[must_use]
    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    #[must_use]
    pub fn author(&self) -> &MessageAuthor {
        &self.author
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    #[must_use]
    pub fn edited_timestamp(&self) -> Option<DateTime<Utc>> {
        self.edited_timestamp
    }

    #[must_use]
    pub fn kind(&self) -> MessageKind {
        self.kind
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn reference(&self) -> Option<&MessageReference> {
        self.reference.as_ref()
    }

    #[must_use]
    pub fn referenced(&self) -> Option<&Message> {
        self.referenced.as_deref()
    }

    #[must_use]
    pub fn is_edited(&self) -> bool {
        self.edited_timestamp.is_some()
    }

    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    #[must_use]
    pub fn is_reply(&self) -> bool {
        self.kind == MessageKind::Reply
    }

    #[must_use]
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    #[must_use]
    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%H:%M").to_string()
    }

    #[must_use]
    pub fn formatted_date(&self) -> String {
        self.timestamp.format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_author() -> MessageAuthor {
        MessageAuthor::new("123", "testuser", "0", None, false)
    }

    #[test]
    fn test_message_creation() {
        let author = create_test_author();
        let timestamp = Utc::now();
        let message = Message::new(1_u64, 100_u64, author.clone(), "Hello, world!", timestamp);

        assert_eq!(message.id().as_u64(), 1);
        assert_eq!(message.channel_id().as_u64(), 100);
        assert_eq!(message.content(), "Hello, world!");
        assert_eq!(message.author().username(), "testuser");
        assert!(!message.is_edited());
        assert!(!message.is_reply());
    }

    #[test]
    fn test_message_with_reply() {
        let author = create_test_author();
        let timestamp = Utc::now();
        let referenced = Message::new(1_u64, 100_u64, author.clone(), "Original", timestamp);
        let reply = Message::new(2_u64, 100_u64, author, "Reply", timestamp)
            .with_kind(MessageKind::Reply)
            .with_referenced(referenced);

        assert!(reply.is_reply());
        assert!(reply.referenced().is_some());
    }

    #[test]
    fn test_message_kind_is_regular() {
        assert!(MessageKind::Default.is_regular());
        assert!(MessageKind::Reply.is_regular());
        assert!(!MessageKind::UserJoin.is_regular());
        assert!(MessageKind::UserJoin.is_system());
    }

    #[test]
    fn test_attachment_is_image() {
        let image = Attachment::new("1", "photo.jpg", 1000, "https://example.com/photo.jpg")
            .with_content_type("image/jpeg");
        let file = Attachment::new("2", "document.pdf", 2000, "https://example.com/doc.pdf")
            .with_content_type("application/pdf");

        assert!(image.is_image());
        assert!(!file.is_image());
    }
}
