//! Discord API response DTOs.

use serde::Deserialize;

/// Discord API user response structure.
#[derive(Debug, Deserialize)]
pub struct UserResponse {
    /// Discord user ID.
    pub id: String,
    /// Discord username.
    pub username: String,
    /// User discriminator tag.
    pub discriminator: String,
    /// Optional avatar hash.
    pub avatar: Option<String>,
    /// Whether the user is a bot.
    #[serde(default)]
    pub bot: bool,
}

/// Discord API error response structure.
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    /// Error message from Discord.
    pub message: String,
}

/// Discord API guild response structure from /users/@me/guilds.
#[derive(Debug, Deserialize)]
pub struct GuildResponse {
    /// Guild ID (snowflake as string).
    pub id: String,
    /// Guild name.
    #[serde(default)]
    pub name: String,
    /// Guild icon hash (null if no custom icon).
    pub icon: Option<String>,
    /// Whether the current user owns this guild.
    #[serde(default)]
    #[allow(dead_code)]
    pub owner: bool,
    /// User's permissions in this guild (string of permission bits).
    #[serde(default)]
    #[allow(dead_code)]
    pub permissions: Option<String>,
    /// Guild feature flags.
    #[serde(default)]
    #[allow(dead_code)]
    pub features: Vec<String>,
}

/// Discord API channel response structure.
#[derive(Debug, Deserialize)]
pub struct ChannelResponse {
    /// Channel ID.
    pub id: String,
    /// Channel type.
    #[serde(rename = "type")]
    pub kind: u8,
    /// Guild ID (if guild channel).
    #[allow(dead_code)]
    pub guild_id: Option<String>,
    /// Channel name.
    pub name: Option<String>,
    /// Parent category ID.
    pub parent_id: Option<String>,
    /// Position in channel list.
    #[serde(default)]
    pub position: i32,
    /// Channel topic.
    pub topic: Option<String>,
}

/// Discord API DM recipient structure.
#[derive(Debug, Deserialize)]
pub struct DmRecipient {
    /// User ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Global display name.
    #[serde(default)]
    pub global_name: Option<String>,
}

/// Discord API DM channel response structure.
#[derive(Debug, Deserialize)]
pub struct DmChannelResponse {
    /// Channel ID.
    pub id: String,
    /// Channel type (DM = 1, Group DM = 3).
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub kind: u8,
    /// List of recipients in this DM.
    #[serde(default)]
    pub recipients: Vec<DmRecipient>,
}

/// Discord API message author structure.
#[derive(Debug, Deserialize)]
pub struct MessageAuthorResponse {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

/// Discord API attachment structure.
#[derive(Debug, Deserialize)]
pub struct AttachmentResponse {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    pub url: String,
    pub content_type: Option<String>,
}

/// Discord API message reference structure.
#[derive(Debug, Deserialize)]
pub struct MessageReferenceResponse {
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
}

/// Discord API message response structure.
#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    #[allow(dead_code)]
    pub channel_id: String,
    pub author: MessageAuthorResponse,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub attachments: Vec<AttachmentResponse>,
    pub message_reference: Option<MessageReferenceResponse>,
    pub referenced_message: Option<Box<Self>>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mentions: Vec<MentionUserResponse>,
}

#[derive(Debug, Deserialize)]
pub struct MentionUserResponse {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct SendMessagePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReferencePayload>,
}

#[derive(Debug, serde::Serialize)]
pub struct MessageReferencePayload {
    pub message_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct EditMessagePayload {
    pub content: String,
}
