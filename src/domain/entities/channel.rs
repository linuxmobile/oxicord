//! Discord channel entity.

use serde::{Deserialize, Serialize};

use super::{GuildId, MessageId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelId(#[serde(with = "crate::domain::serde_utils::string_to_u64")] pub u64);

impl ChannelId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for ChannelId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<&str> for ChannelId {
    fn from(value: &str) -> Self {
        Self(value.parse().unwrap_or(0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChannelKind {
    #[default]
    Text = 0,
    Dm = 1,
    Voice = 2,
    GroupDm = 3,
    Category = 4,
    Announcement = 5,
    Store = 6,
    Lfg = 7,
    LfgGroupDm = 8,
    ThreadAlpha = 9,
    AnnouncementThread = 10,
    PublicThread = 11,
    PrivateThread = 12,
    StageVoice = 13,
    Directory = 14,
    Forum = 15,
    Media = 16,
    Lobby = 17,
    EphemeralDm = 18,
}

impl ChannelKind {
    #[must_use]
    pub const fn is_text_based(self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::Dm
                | Self::GroupDm
                | Self::Announcement
                | Self::AnnouncementThread
                | Self::PublicThread
                | Self::PrivateThread
        )
    }

    #[must_use]
    pub const fn is_category(self) -> bool {
        matches!(self, Self::Category)
    }

    #[must_use]
    pub const fn is_voice(self) -> bool {
        matches!(self, Self::Voice | Self::StageVoice | Self::Lobby)
    }

    #[must_use]
    pub const fn is_thread(self) -> bool {
        matches!(
            self,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread | Self::ThreadAlpha
        )
    }

    #[must_use]
    pub const fn is_dm(self) -> bool {
        matches!(
            self,
            Self::Dm | Self::GroupDm | Self::LfgGroupDm | Self::EphemeralDm
        )
    }

    #[must_use]
    pub const fn is_deprecated(self) -> bool {
        matches!(self, Self::Store | Self::ThreadAlpha)
    }

    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Voice | Self::StageVoice | Self::Lobby => "󰕾 ",
            Self::Dm | Self::GroupDm | Self::LfgGroupDm | Self::EphemeralDm => "@ ",
            Self::Category => "",
            Self::Forum | Self::Media => "󰭹 ",
            _ => "#",
        }
    }
}

impl From<u8> for ChannelKind {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Dm,
            2 => Self::Voice,
            3 => Self::GroupDm,
            4 => Self::Category,
            5 => Self::Announcement,
            6 => Self::Store,
            7 => Self::Lfg,
            8 => Self::LfgGroupDm,
            9 => Self::ThreadAlpha,
            10 => Self::AnnouncementThread,
            11 => Self::PublicThread,
            12 => Self::PrivateThread,
            13 => Self::StageVoice,
            14 => Self::Directory,
            15 => Self::Forum,
            16 => Self::Media,
            17 => Self::Lobby,
            18 => Self::EphemeralDm,
            _ => Self::Text,
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct ChannelFlags: u64 {
        const GUILD_FEED_REMOVED = 1 << 0;
        const PINNED = 1 << 1;
        const ACTIVE_CHANNELS_REMOVED = 1 << 2;
        const REQUIRE_TAG = 1 << 4;
        const IS_SPAM = 1 << 5;
        const IS_GUILD_RESOURCE_CHANNEL = 1 << 7;
        const CLYDE_AI = 1 << 8;
        const IS_SCHEDULED_FOR_DELETION = 1 << 9;
        const IS_MEDIA_CHANNEL = 1 << 10;
        const SUMMARIES_DISABLED = 1 << 11;
        const APPLICATION_SHELF_CONSENT = 1 << 12;
        const IS_ROLE_SUBSCRIPTION_TEMPLATE_PREVIEW_CHANNEL = 1 << 13;
        const IS_BROADCASTING = 1 << 14;
        const HIDE_MEDIA_DOWNLOAD_OPTIONS = 1 << 15;
        const IS_JOIN_REQUEST_INTERVIEW_CHANNEL = 1 << 16;
        const OBFUSCATED = 1 << 17;
        const IS_MODERATOR_REPORT_CHANNEL = 1 << 19;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum VideoQualityMode {
    #[default]
    Auto = 1,
    Full = 2,
}

impl From<u8> for VideoQualityMode {
    fn from(value: u8) -> Self {
        match value {
            2 => Self::Full,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum OverwriteType {
    Role = 0,
    Member = 1,
}

impl From<u8> for OverwriteType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Member,
            _ => Self::Role,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOverwrite {
    pub id: String,
    pub overwrite_type: OverwriteType,
    pub allow: String,
    pub deny: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadMetadata {
    pub archived: bool,
    pub auto_archive_duration: u16,
    pub archive_timestamp: String,
    pub locked: bool,
    #[serde(default)]
    pub invitable: Option<bool>,
    #[serde(default)]
    pub create_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    id: ChannelId,
    guild_id: Option<GuildId>,
    name: String,
    kind: ChannelKind,
    parent_id: Option<ChannelId>,
    #[serde(default)]
    position: i32,
    topic: Option<String>,
    last_message_id: Option<MessageId>,
    has_unread: bool,
    #[serde(default)]
    mention_count: u32,
    #[serde(default)]
    nsfw: bool,
    #[serde(default)]
    bitrate: Option<u32>,
    #[serde(default)]
    user_limit: Option<u32>,
    #[serde(default)]
    rate_limit_per_user: Option<u16>,
    #[serde(default)]
    flags: ChannelFlags,
    #[serde(default)]
    rtc_region: Option<String>,
    #[serde(default)]
    video_quality_mode: Option<VideoQualityMode>,
    #[serde(default)]
    default_auto_archive_duration: Option<u16>,
    #[serde(default)]
    permission_overwrites: Vec<PermissionOverwrite>,
    #[serde(default)]
    thread_metadata: Option<ThreadMetadata>,
}

impl Channel {
    #[must_use]
    pub fn new(id: impl Into<ChannelId>, name: impl Into<String>, kind: ChannelKind) -> Self {
        Self {
            id: id.into(),
            guild_id: None,
            name: name.into(),
            kind,
            parent_id: None,
            position: 0,
            topic: None,
            last_message_id: None,
            has_unread: false,
            mention_count: 0,
            nsfw: false,
            bitrate: None,
            user_limit: None,
            rate_limit_per_user: None,
            flags: ChannelFlags::empty(),
            rtc_region: None,
            video_quality_mode: None,
            default_auto_archive_duration: None,
            permission_overwrites: Vec::new(),
            thread_metadata: None,
        }
    }

    #[must_use]
    pub fn with_guild(mut self, guild_id: impl Into<GuildId>) -> Self {
        self.guild_id = Some(guild_id.into());
        self
    }

    #[must_use]
    pub fn with_parent(mut self, parent_id: impl Into<ChannelId>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    #[must_use]
    pub const fn with_position(mut self, position: i32) -> Self {
        self.position = position;
        self
    }

    #[must_use]
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    #[must_use]
    pub fn with_last_message_id(mut self, last_message_id: Option<MessageId>) -> Self {
        self.last_message_id = last_message_id;
        self
    }

    #[must_use]
    pub const fn with_unread(mut self, has_unread: bool) -> Self {
        self.has_unread = has_unread;
        self
    }

    #[must_use]
    pub const fn with_nsfw(mut self, nsfw: bool) -> Self {
        self.nsfw = nsfw;
        self
    }

    #[must_use]
    pub const fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    #[must_use]
    pub const fn with_user_limit(mut self, user_limit: u32) -> Self {
        self.user_limit = Some(user_limit);
        self
    }

    #[must_use]
    pub const fn with_rate_limit_per_user(mut self, rate_limit: u16) -> Self {
        self.rate_limit_per_user = Some(rate_limit);
        self
    }

    #[must_use]
    pub const fn with_flags(mut self, flags: ChannelFlags) -> Self {
        self.flags = flags;
        self
    }

    #[must_use]
    pub fn with_rtc_region(mut self, region: impl Into<String>) -> Self {
        self.rtc_region = Some(region.into());
        self
    }

    #[must_use]
    pub const fn with_video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.video_quality_mode = Some(mode);
        self
    }

    #[must_use]
    pub const fn with_default_auto_archive_duration(mut self, duration: u16) -> Self {
        self.default_auto_archive_duration = Some(duration);
        self
    }

    #[must_use]
    pub fn with_permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.permission_overwrites = overwrites;
        self
    }

    #[must_use]
    pub fn with_thread_metadata(mut self, metadata: ThreadMetadata) -> Self {
        self.thread_metadata = Some(metadata);
        self
    }

    #[must_use]
    pub const fn id(&self) -> ChannelId {
        self.id
    }

    #[must_use]
    pub const fn guild_id(&self) -> Option<GuildId> {
        self.guild_id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> ChannelKind {
        self.kind
    }

    #[must_use]
    pub const fn parent_id(&self) -> Option<ChannelId> {
        self.parent_id
    }

    #[must_use]
    pub const fn position(&self) -> i32 {
        self.position
    }

    #[must_use]
    pub fn topic(&self) -> Option<&str> {
        self.topic.as_deref()
    }

    #[must_use]
    pub const fn last_message_id(&self) -> Option<MessageId> {
        self.last_message_id
    }

    pub fn set_last_message_id(&mut self, last_message_id: Option<MessageId>) {
        self.last_message_id = last_message_id;
    }

    #[must_use]
    pub const fn has_unread(&self) -> bool {
        self.has_unread
    }

    pub const fn set_unread(&mut self, has_unread: bool) {
        self.has_unread = has_unread;
    }

    #[must_use]
    pub const fn mention_count(&self) -> u32 {
        self.mention_count
    }

    pub fn set_mention_count(&mut self, count: u32) {
        self.mention_count = count;
    }

    pub fn increment_mention_count(&mut self) {
        self.mention_count += 1;
    }

    #[must_use]
    pub const fn nsfw(&self) -> bool {
        self.nsfw
    }

    #[must_use]
    pub const fn bitrate(&self) -> Option<u32> {
        self.bitrate
    }

    #[must_use]
    pub const fn user_limit(&self) -> Option<u32> {
        self.user_limit
    }

    #[must_use]
    pub const fn rate_limit_per_user(&self) -> Option<u16> {
        self.rate_limit_per_user
    }

    #[must_use]
    pub const fn flags(&self) -> ChannelFlags {
        self.flags
    }

    #[must_use]
    pub fn rtc_region(&self) -> Option<&str> {
        self.rtc_region.as_deref()
    }

    #[must_use]
    pub const fn video_quality_mode(&self) -> Option<VideoQualityMode> {
        self.video_quality_mode
    }

    #[must_use]
    pub const fn default_auto_archive_duration(&self) -> Option<u16> {
        self.default_auto_archive_duration
    }

    #[must_use]
    pub fn permission_overwrites(&self) -> &[PermissionOverwrite] {
        &self.permission_overwrites
    }

    #[must_use]
    pub const fn thread_metadata(&self) -> Option<&ThreadMetadata> {
        self.thread_metadata.as_ref()
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        format!("{}{}", self.kind.prefix(), self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new(123_u64, "general", ChannelKind::Text);

        assert_eq!(channel.id().as_u64(), 123);
        assert_eq!(channel.name(), "general");
        assert_eq!(channel.display_name(), "#general");
    }

    #[test]
    fn test_channel_with_parent() {
        let channel = Channel::new(123_u64, "chat", ChannelKind::Text)
            .with_guild(456_u64)
            .with_parent(789_u64);

        assert_eq!(channel.guild_id(), Some(GuildId(456)));
        assert_eq!(channel.parent_id(), Some(ChannelId(789)));
    }

    #[test]
    fn test_voice_channel_display() {
        let channel = Channel::new(123_u64, "Voice", ChannelKind::Voice);
        assert_eq!(channel.display_name(), "󰕾 Voice");
    }

    #[test]
    fn test_channel_kind_is_text_based() {
        assert!(ChannelKind::Text.is_text_based());
        assert!(ChannelKind::Dm.is_text_based());
        assert!(!ChannelKind::Voice.is_text_based());
        assert!(!ChannelKind::Category.is_text_based());
    }

    #[test]
    fn test_channel_kind_is_thread() {
        assert!(ChannelKind::PublicThread.is_thread());
        assert!(ChannelKind::PrivateThread.is_thread());
        assert!(ChannelKind::AnnouncementThread.is_thread());
        assert!(!ChannelKind::Text.is_thread());
    }

    #[test]
    fn test_channel_kind_is_voice() {
        assert!(ChannelKind::Voice.is_voice());
        assert!(ChannelKind::StageVoice.is_voice());
        assert!(ChannelKind::Lobby.is_voice());
        assert!(!ChannelKind::Text.is_voice());
    }

    #[test]
    fn test_channel_with_new_fields() {
        let channel = Channel::new(123_u64, "nsfw-chat", ChannelKind::Text)
            .with_nsfw(true)
            .with_rate_limit_per_user(60)
            .with_flags(ChannelFlags::PINNED);

        assert!(channel.nsfw());
        assert_eq!(channel.rate_limit_per_user(), Some(60));
        assert!(channel.flags().contains(ChannelFlags::PINNED));
    }

    #[test]
    fn test_voice_channel_properties() {
        let channel = Channel::new(123_u64, "Music", ChannelKind::Voice)
            .with_bitrate(64000)
            .with_user_limit(10)
            .with_rtc_region("us-west");

        assert_eq!(channel.bitrate(), Some(64000));
        assert_eq!(channel.user_limit(), Some(10));
        assert_eq!(channel.rtc_region(), Some("us-west"));
    }
}
