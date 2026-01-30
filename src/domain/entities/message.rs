use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::{ChannelId, GuildId, User};

/// Unique identifier for a Discord message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

impl MessageId {
    /// Returns the underlying u64 value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
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
    GuildIncidentAlertModeEnabled = 36,
    GuildIncidentAlertModeDisabled = 37,
    GuildIncidentReportRaid = 38,
    GuildIncidentReportFalseAlarm = 39,
    PurchaseNotification = 44,
    PollResult = 46,
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
            36 => Self::GuildIncidentAlertModeEnabled,
            37 => Self::GuildIncidentAlertModeDisabled,
            38 => Self::GuildIncidentReportRaid,
            39 => Self::GuildIncidentReportFalseAlarm,
            44 => Self::PurchaseNotification,
            46 => Self::PollResult,
            _ => Self::Default,
        }
    }
}

impl MessageKind {
    /// Returns true if this is a regular user message.
    #[must_use]
    pub const fn is_regular(self) -> bool {
        matches!(self, Self::Default | Self::Reply)
    }

    /// Returns true if this is a system message.
    #[must_use]
    pub const fn is_system(self) -> bool {
        !self.is_regular()
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct MessageFlags: u64 {
        const CROSSPOSTED = 1 << 0;
        const IS_CROSSPOST = 1 << 1;
        const SUPPRESS_EMBEDS = 1 << 2;
        const SOURCE_MESSAGE_DELETED = 1 << 3;
        const URGENT = 1 << 4;
        const HAS_THREAD = 1 << 5;
        const EPHEMERAL = 1 << 6;
        const LOADING = 1 << 7;
        const FAILED_TO_MENTION_SOME_ROLES_IN_THREAD = 1 << 8;
        const SUPPRESS_NOTIFICATIONS = 1 << 12;
        const IS_VOICE_MESSAGE = 1 << 13;
        const HAS_SNAPSHOT = 1 << 14;
        const IS_COMPONENTS_V2 = 1 << 15;
    }
}

/// Discord message embed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[allow(missing_docs)]
pub struct Embed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub timestamp: Option<String>,
    pub provider: Option<EmbedProvider>,
    pub thumbnail: Option<EmbedThumbnail>,
    pub author: Option<EmbedAuthor>,
    pub footer: Option<EmbedFooter>,
    pub image: Option<EmbedImage>,
    pub video: Option<EmbedVideo>,
    #[serde(default)]
    pub fields: Vec<EmbedField>,
}

#[allow(missing_docs)]
impl Embed {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedAuthor {
    pub name: String,
    pub url: Option<String>,
    pub icon_url: Option<String>,
    pub proxy_icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedFooter {
    pub text: String,
    pub icon_url: Option<String>,
    pub proxy_icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedImage {
    pub url: String,
    pub proxy_url: Option<String>,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedVideo {
    pub url: Option<String>,
    pub proxy_url: Option<String>,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedProvider {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedThumbnail {
    pub url: String,
    pub proxy_url: Option<String>,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

/// Discord message attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub url: String,
    pub content_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub spoiler: bool,
}

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
            width: None,
            height: None,
            spoiler: false,
        }
    }

    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    #[must_use]
    pub fn is_image(&self) -> bool {
        if let Some(ct) = &self.content_type {
            return ct.starts_with("image/");
        }
        let lower = self.filename.to_lowercase();
        std::path::Path::new(&lower).extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("jpg")
                || ext.eq_ignore_ascii_case("jpeg")
                || ext.eq_ignore_ascii_case("gif")
                || ext.eq_ignore_ascii_case("webp")
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageAuthor {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    pub bot: bool,
    pub global_name: Option<String>,
}

impl MessageAuthor {
    #[must_use]
    pub fn raw_display_name(&self) -> String {
        if let Some(ref global) = self.global_name {
            global.clone()
        } else if self.discriminator == "0" {
            self.username.clone()
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }

    // Add color logic here or remove dependency on it for strict DTO separation
    // For now we assume no color directly on author, it might come from member
    #[must_use]
    pub fn color(&self) -> Option<u32> {
        None
    }

    #[must_use]
    pub const fn is_bot(&self) -> bool {
        self.bot
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
}

/// Discord message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct Message {
    id: MessageId,
    channel_id: ChannelId,
    author: MessageAuthor,
    content: String,
    timestamp: DateTime<Local>,
    edited_timestamp: Option<DateTime<Local>>,
    kind: MessageKind,
    attachments: Vec<Attachment>,
    embeds: Vec<Embed>,
    pinned: bool,
    mentions: Vec<User>,
    reactions: Vec<Reaction>,
    flags: MessageFlags,
    #[allow(clippy::struct_field_names)]
    message_reference: Option<MessageReference>,
    referenced_message: Option<Box<Message>>,
    guild_id: Option<GuildId>,
}

impl Message {
    #[must_use]
    pub const fn id(&self) -> MessageId {
        self.id
    }

    #[must_use]
    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    #[must_use]
    pub const fn author(&self) -> &MessageAuthor {
        &self.author
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Local> {
        self.timestamp
    }

    #[must_use]
    pub const fn kind(&self) -> MessageKind {
        self.kind
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn embeds(&self) -> &[Embed] {
        &self.embeds
    }

    #[must_use]
    pub fn mentions(&self) -> &[User] {
        &self.mentions
    }

    #[must_use]
    pub fn reactions(&self) -> &[Reaction] {
        &self.reactions
    }

    #[must_use]
    pub const fn flags(&self) -> MessageFlags {
        self.flags
    }

    #[must_use]
    pub const fn message_reference(&self) -> Option<&MessageReference> {
        self.message_reference.as_ref()
    }

    #[must_use]
    pub fn referenced(&self) -> Option<&Message> {
        self.referenced_message.as_deref()
    }

    #[must_use]
    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%H:%M").to_string()
    }

    #[must_use]
    pub const fn is_edited(&self) -> bool {
        self.edited_timestamp.is_some()
    }

    #[must_use]
    pub fn is_reply(&self) -> bool {
        self.kind == MessageKind::Reply || self.message_reference.is_some()
    }

    #[must_use]
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    #[must_use]
    pub fn has_embeds(&self) -> bool {
        !self.embeds.is_empty()
    }

    #[must_use]
    pub const fn reference(&self) -> Option<&MessageReference> {
        self.message_reference.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageReference {
    pub message_id: Option<MessageId>,
    pub channel_id: Option<ChannelId>,
    pub guild_id: Option<GuildId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reaction {
    pub count: u32,
    pub me: bool,
    pub emoji: ReactionEmoji,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReactionEmoji {
    pub id: Option<String>,
    pub name: Option<String>,
}

impl MessageReference {
    #[must_use]
    pub const fn new(
        message_id: Option<MessageId>,
        channel_id: Option<ChannelId>,
        guild_id: Option<GuildId>,
    ) -> Self {
        Self {
            message_id,
            channel_id,
            guild_id,
        }
    }
}

#[allow(missing_docs)]
impl Message {
    #[must_use]
    pub fn new(
        id: MessageId,
        channel_id: ChannelId,
        author: MessageAuthor,
        content: String,
        timestamp: DateTime<Local>,
        kind: MessageKind,
    ) -> Self {
        Self {
            id,
            channel_id,
            author,
            content,
            timestamp,
            edited_timestamp: None,
            kind,
            attachments: Vec::new(),
            embeds: Vec::new(),
            pinned: false,
            mentions: Vec::new(),
            reactions: Vec::new(),
            flags: MessageFlags::empty(),
            message_reference: None,
            referenced_message: None,
            guild_id: None,
        }
    }

    #[must_use]
    pub const fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    #[must_use]
    pub fn with_guild_id(mut self, guild_id: Option<GuildId>) -> Self {
        self.guild_id = guild_id;
        self
    }

    #[must_use]
    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    #[must_use]
    pub fn with_edited_timestamp(mut self, timestamp: DateTime<Local>) -> Self {
        self.edited_timestamp = Some(timestamp);
        self
    }

    #[must_use]
    pub fn with_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    #[must_use]
    pub fn with_embeds(mut self, embeds: Vec<Embed>) -> Self {
        self.embeds = embeds;
        self
    }

    #[must_use]
    pub fn with_mentions(mut self, mentions: Vec<User>) -> Self {
        self.mentions = mentions;
        self
    }

    #[must_use]
    pub fn with_reactions(mut self, reactions: Vec<Reaction>) -> Self {
        self.reactions = reactions;
        self
    }

    #[must_use]
    pub fn with_reference(mut self, reference: MessageReference) -> Self {
        self.message_reference = Some(reference);
        self
    }

    #[must_use]
    pub fn with_referenced_message(mut self, message: Option<Message>) -> Self {
        self.referenced_message = message.map(Box::new);
        self
    }

    #[must_use]
    pub fn with_flags(mut self, flags: MessageFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Checks if the message can be edited by the given user.
    ///
    /// Currently, a user can only edit their own messages.
    #[must_use]
    pub fn can_be_edited_by(&self, user: &User) -> bool {
        self.author.id == user.id_str()
    }
}
