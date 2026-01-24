use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::domain::entities::{
    Channel, ChannelId, GuildFolder, GuildId, Message, MessageId, ReadState,
};

/// Commands that can be sent to the gateway.
#[derive(Debug, Clone)]
pub enum GatewayCommand {
    /// Subscribe to a guild channel to receive typing events.
    /// Required for user accounts to receive `TYPING_START`.
    SubscribeChannel {
        guild_id: String,
        channel_id: String,
    },
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum GatewayEventKind {
    Connected {
        session_id: String,
        resume_url: Option<String>,
    },
    Disconnected {
        reason: String,
        can_resume: bool,
    },
    Reconnecting {
        attempt: u32,
    },
    Resumed,
    HeartbeatAck {
        latency_ms: u64,
    },
    #[allow(clippy::large_enum_variant)]
    Dispatch(DispatchEvent),
    Error {
        message: String,
        recoverable: bool,
    },
}

impl GatewayEventKind {
    #[must_use]
    pub const fn is_connection_event(&self) -> bool {
        matches!(
            self,
            Self::Connected { .. } | Self::Disconnected { .. } | Self::Reconnecting { .. }
        )
    }

    #[must_use]
    pub const fn is_message_event(&self) -> bool {
        matches!(
            self,
            Self::Dispatch(
                DispatchEvent::MessageCreate { .. }
                    | DispatchEvent::MessageUpdate { .. }
                    | DispatchEvent::MessageDelete { .. }
            )
        )
    }
}

#[derive(Debug, Clone)]
pub enum DispatchEvent {
    Ready {
        session_id: String,
        resume_gateway_url: Option<String>,
        user_id: String,
        guilds: Vec<UnavailableGuild>,
        /// Channels for guilds received in READY (User accounts)
        initial_guild_channels: std::collections::HashMap<GuildId, Vec<Channel>>,
        read_states: Vec<ReadState>,
        guild_folders: Vec<GuildFolder>,
    },

    MessageCreate {
        message: Message,
    },
    MessageUpdate {
        message: Message,
    },
    MessageDelete {
        message_id: MessageId,
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    },
    MessageDeleteBulk {
        message_ids: Vec<MessageId>,
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    },

    MessageReactionAdd {
        user_id: String,
        channel_id: ChannelId,
        message_id: MessageId,
        guild_id: Option<GuildId>,
        emoji: ReactionEmoji,
    },
    MessageReactionRemove {
        user_id: String,
        channel_id: ChannelId,
        message_id: MessageId,
        guild_id: Option<GuildId>,
        emoji: ReactionEmoji,
    },
    MessageReactionRemoveAll {
        channel_id: ChannelId,
        message_id: MessageId,
        guild_id: Option<GuildId>,
    },

    TypingStart {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        user_id: String,
        username: Option<String>,
        timestamp: DateTime<Utc>,
    },

    PresenceUpdate {
        user_id: String,
        guild_id: Option<GuildId>,
        status: PresenceStatus,
        activities: Vec<Activity>,
    },

    ChannelCreate {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        name: String,
        kind: u8,
    },
    ChannelUpdate {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        name: String,
        kind: u8,
    },
    ChannelDelete {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    },

    GuildCreate {
        guild_id: GuildId,
        name: String,
        unavailable: bool,
        channels: Vec<Channel>,
    },
    GuildUpdate {
        guild_id: GuildId,
        name: String,
    },
    GuildDelete {
        guild_id: GuildId,
        unavailable: bool,
    },

    UserUpdate {
        user_id: String,
        username: String,
        discriminator: String,
        avatar: Option<String>,
    },

    UserSettingsUpdate {
        guild_folders: Vec<GuildFolder>,
    },

    VoiceStateUpdate {
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        user_id: String,
        session_id: String,
        deaf: bool,
        mute: bool,
        self_deaf: bool,
        self_mute: bool,
        self_video: bool,
        suppress: bool,
    },
    VoiceServerUpdate {
        token: String,
        guild_id: GuildId,
        endpoint: Option<String>,
    },

    Unknown {
        event_type: String,
    },
}

impl DispatchEvent {
    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        match self {
            Self::Ready { .. } => "READY",
            Self::MessageCreate { .. } => "MESSAGE_CREATE",
            Self::MessageUpdate { .. } => "MESSAGE_UPDATE",
            Self::MessageDelete { .. } => "MESSAGE_DELETE",
            Self::MessageDeleteBulk { .. } => "MESSAGE_DELETE_BULK",
            Self::MessageReactionAdd { .. } => "MESSAGE_REACTION_ADD",
            Self::MessageReactionRemove { .. } => "MESSAGE_REACTION_REMOVE",
            Self::MessageReactionRemoveAll { .. } => "MESSAGE_REACTION_REMOVE_ALL",
            Self::TypingStart { .. } => "TYPING_START",
            Self::PresenceUpdate { .. } => "PRESENCE_UPDATE",
            Self::ChannelCreate { .. } => "CHANNEL_CREATE",
            Self::ChannelUpdate { .. } => "CHANNEL_UPDATE",
            Self::ChannelDelete { .. } => "CHANNEL_DELETE",
            Self::GuildCreate { .. } => "GUILD_CREATE",
            Self::GuildUpdate { .. } => "GUILD_UPDATE",
            Self::GuildDelete { .. } => "GUILD_DELETE",
            Self::UserUpdate { .. } => "USER_UPDATE",
            Self::UserSettingsUpdate { .. } => "USER_SETTINGS_UPDATE",
            Self::VoiceStateUpdate { .. } => "VOICE_STATE_UPDATE",
            Self::VoiceServerUpdate { .. } => "VOICE_SERVER_UPDATE",
            Self::Unknown { .. } => "UNKNOWN",
        }
    }

    #[must_use]
    pub const fn channel_id(&self) -> Option<ChannelId> {
        match self {
            Self::MessageCreate { message } | Self::MessageUpdate { message } => {
                Some(message.channel_id())
            }
            Self::MessageDelete { channel_id, .. }
            | Self::MessageDeleteBulk { channel_id, .. }
            | Self::MessageReactionAdd { channel_id, .. }
            | Self::MessageReactionRemove { channel_id, .. }
            | Self::MessageReactionRemoveAll { channel_id, .. }
            | Self::TypingStart { channel_id, .. }
            | Self::ChannelCreate { channel_id, .. }
            | Self::ChannelUpdate { channel_id, .. }
            | Self::ChannelDelete { channel_id, .. }
            | Self::VoiceStateUpdate {
                channel_id: Some(channel_id),
                ..
            } => Some(*channel_id),
            _ => None,
        }
    }

    #[must_use]
    pub const fn guild_id(&self) -> Option<GuildId> {
        match self {
            Self::MessageDelete { guild_id, .. }
            | Self::MessageDeleteBulk { guild_id, .. }
            | Self::MessageReactionAdd { guild_id, .. }
            | Self::MessageReactionRemove { guild_id, .. }
            | Self::MessageReactionRemoveAll { guild_id, .. }
            | Self::TypingStart { guild_id, .. }
            | Self::PresenceUpdate { guild_id, .. }
            | Self::ChannelCreate { guild_id, .. }
            | Self::ChannelUpdate { guild_id, .. }
            | Self::ChannelDelete { guild_id, .. }
            | Self::VoiceStateUpdate { guild_id, .. } => *guild_id,
            Self::GuildCreate { guild_id, .. }
            | Self::GuildUpdate { guild_id, .. }
            | Self::GuildDelete { guild_id, .. }
            | Self::VoiceServerUpdate { guild_id, .. } => Some(*guild_id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableGuild {
    pub id: GuildId,
    pub unavailable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionEmoji {
    pub id: Option<String>,
    pub name: Option<String>,
    pub animated: bool,
}

impl ReactionEmoji {
    #[must_use]
    pub fn display(&self) -> String {
        self.name.clone().unwrap_or_else(|| "?".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresenceStatus {
    Online,
    Idle,
    DoNotDisturb,
    Invisible,
    #[default]
    Offline,
}

impl PresenceStatus {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "online" => Self::Online,
            "idle" => Self::Idle,
            "dnd" => Self::DoNotDisturb,
            "invisible" => Self::Invisible,
            _ => Self::Offline,
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::DoNotDisturb => "dnd",
            Self::Invisible => "invisible",
            Self::Offline => "offline",
        }
    }

    #[must_use]
    pub const fn is_online(&self) -> bool {
        !matches!(self, Self::Offline | Self::Invisible)
    }

    #[must_use]
    pub const fn display_indicator(&self) -> &'static str {
        match self {
            Self::Online => "●",
            Self::Idle => "◐",
            Self::DoNotDisturb => "⊘",
            Self::Invisible | Self::Offline => "○",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub name: String,
    pub kind: ActivityKind,
    pub details: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivityKind {
    #[default]
    Playing = 0,
    Streaming = 1,
    Listening = 2,
    Watching = 3,
    Custom = 4,
    Competing = 5,
}

impl ActivityKind {
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Streaming,
            2 => Self::Listening,
            3 => Self::Watching,
            4 => Self::Custom,
            5 => Self::Competing,
            _ => Self::Playing,
        }
    }

    #[must_use]
    pub const fn prefix(&self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Streaming => "Streaming",
            Self::Listening => "Listening to",
            Self::Watching => "Watching",
            Self::Custom => "",
            Self::Competing => "Competing in",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypingUser {
    pub user_id: String,
    pub username: String,
    pub channel_id: ChannelId,
    pub started_at: Instant,
}

impl TypingUser {
    #[must_use]
    pub fn new(user_id: String, username: String, channel_id: ChannelId) -> Self {
        Self {
            user_id,
            username,
            channel_id,
            started_at: Instant::now(),
        }
    }

    #[must_use]
    pub fn is_expired(&self, timeout: std::time::Duration) -> bool {
        self.started_at.elapsed() > timeout
    }

    pub fn refresh(&mut self) {
        self.started_at = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presence_status() {
        assert_eq!(PresenceStatus::parse("online"), PresenceStatus::Online);
        assert_eq!(PresenceStatus::parse("dnd"), PresenceStatus::DoNotDisturb);
        assert!(PresenceStatus::Online.is_online());
        assert!(!PresenceStatus::Offline.is_online());
    }

    #[test]
    fn test_activity_kind() {
        assert_eq!(ActivityKind::from_u8(0), ActivityKind::Playing);
        assert_eq!(ActivityKind::Listening.prefix(), "Listening to");
    }

    #[test]
    fn test_typing_user_expiration() {
        let user = TypingUser::new("123".into(), "test".into(), ChannelId(1));
        assert!(!user.is_expired(std::time::Duration::from_secs(10)));
    }

    #[test]
    fn test_dispatch_event_channel_id() {
        let event = DispatchEvent::TypingStart {
            channel_id: ChannelId(123),
            guild_id: None,
            user_id: "456".into(),
            username: Some("test".into()),
            timestamp: Utc::now(),
        };
        assert_eq!(event.channel_id(), Some(ChannelId(123)));
    }
}
