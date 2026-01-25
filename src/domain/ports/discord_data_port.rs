//! Discord data port for fetching guilds and channels.

use async_trait::async_trait;

use crate::domain::entities::{
    AuthToken, Channel, ChannelId, ForumThread, Guild, GuildId, Message, MessageId, ReadState,
};
use crate::domain::errors::AuthError;

/// Represents a direct message channel with a recipient.
#[derive(Debug, Clone)]
pub struct DirectMessageChannel {
    /// The unique channel ID for this DM conversation.
    pub channel_id: String,
    /// The recipient user's ID.
    pub recipient_id: String,
    /// The recipient's username.
    pub recipient_username: String,
    /// The recipient's discriminator.
    pub recipient_discriminator: String,
    /// The recipient's global name (display name).
    pub recipient_global_name: Option<String>,
    /// The ID of the last message sent in this channel.
    pub last_message_id: Option<MessageId>,
    /// Whether the channel has unread messages.
    pub has_unread: bool,
    /// The number of mentions in this channel.
    pub mention_count: u32,
}

/// Options for fetching messages from a channel.
#[derive(Debug, Clone, Default)]
pub struct FetchMessagesOptions {
    pub limit: Option<u8>,
    pub before: Option<u64>,
    pub after: Option<u64>,
    pub around: Option<u64>,
}

impl FetchMessagesOptions {
    #[must_use]
    pub const fn with_limit(mut self, limit: u8) -> Self {
        self.limit = Some(if limit < 100 { limit } else { 100 });
        self
    }

    #[must_use]
    pub const fn before_message(mut self, message_id: u64) -> Self {
        self.before = Some(message_id);
        self
    }

    #[must_use]
    pub const fn after_message(mut self, message_id: u64) -> Self {
        self.after = Some(message_id);
        self
    }
}

#[derive(Debug, Clone)]
pub struct SendMessageRequest {
    pub channel_id: ChannelId,
    pub content: String,
    pub reply_to: Option<MessageId>,
    pub attachments: Vec<std::path::PathBuf>,
}

impl SendMessageRequest {
    #[must_use]
    pub fn new(channel_id: ChannelId, content: impl Into<String>) -> Self {
        Self {
            channel_id,
            content: content.into(),
            reply_to: None,
            attachments: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_reply(mut self, message_id: MessageId) -> Self {
        self.reply_to = Some(message_id);
        self
    }

    #[must_use]
    pub fn with_attachments(mut self, attachments: Vec<std::path::PathBuf>) -> Self {
        self.attachments = attachments;
        self
    }
}

#[derive(Debug, Clone)]
pub struct EditMessageRequest {
    pub channel_id: ChannelId,
    pub message_id: MessageId,
    pub content: String,
}

impl EditMessageRequest {
    #[must_use]
    pub fn new(channel_id: ChannelId, message_id: MessageId, content: impl Into<String>) -> Self {
        Self {
            channel_id,
            message_id,
            content: content.into(),
        }
    }
}

/// Port for fetching Discord data (guilds, channels, DMs, etc).
#[async_trait]
pub trait DiscordDataPort: Send + Sync {
    /// Fetches all guilds the user is a member of.
    async fn fetch_guilds(&self, token: &AuthToken) -> Result<Vec<Guild>, AuthError>;

    /// Fetches all channels for a given guild.
    async fn fetch_channels(
        &self,
        token: &AuthToken,
        guild_id: u64,
    ) -> Result<Vec<Channel>, AuthError>;

    /// Fetches all direct message channels for the user.
    async fn fetch_dm_channels(
        &self,
        token: &AuthToken,
    ) -> Result<Vec<DirectMessageChannel>, AuthError>;

    /// Fetches read states for all channels.
    async fn fetch_read_states(&self, token: &AuthToken) -> Result<Vec<ReadState>, AuthError>;

    /// Fetches messages from a channel.
    async fn fetch_messages(
        &self,
        token: &AuthToken,
        channel_id: u64,
        options: FetchMessagesOptions,
    ) -> Result<Vec<Message>, AuthError>;

    /// Fetches historical messages before a specific message ID.
    async fn load_more_before_id(
        &self,
        token: &AuthToken,
        channel_id: u64,
        message_id: u64,
        limit: u8,
    ) -> Result<Vec<Message>, AuthError>;

    /// Sends a message to a channel.
    async fn send_message(
        &self,
        token: &AuthToken,
        request: SendMessageRequest,
    ) -> Result<Message, AuthError>;

    /// Edits an existing message.
    async fn edit_message(
        &self,
        token: &AuthToken,
        request: EditMessageRequest,
    ) -> Result<Message, AuthError>;

    /// Deletes a message.
    async fn delete_message(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<(), AuthError>;

    /// Sends a typing indicator to a channel.
    async fn send_typing_indicator(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
    ) -> Result<(), AuthError>;

    /// Acknowledges a message (marks as read).
    async fn acknowledge_message(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<(), AuthError>;

    /// Fetches forum threads for a channel.
    async fn fetch_forum_threads(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        offset: u32,
        limit: Option<u8>,
    ) -> Result<Vec<ForumThread>, AuthError>;
}
