use std::time::Duration;

pub const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json&compress=zlib-stream";
pub const ZLIB_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

pub const HEARTBEAT_JITTER_PERCENT: f64 = 0.05;
pub const HEARTBEAT_TIMEOUT_MULTIPLIER: f64 = 1.5;

pub const RECONNECT_DELAY_BASE: Duration = Duration::from_secs(1);
pub const RECONNECT_DELAY_MAX: Duration = Duration::from_secs(60);
pub const RECONNECT_JITTER_MAX: Duration = Duration::from_millis(500);
pub const MAX_RECONNECT_ATTEMPTS: u32 = 10;

pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
pub const IDENTIFY_TIMEOUT: Duration = Duration::from_secs(10);

pub const TYPING_INDICATOR_TIMEOUT: Duration = Duration::from_secs(10);

pub const CLIENT_PROPERTIES_OS: &str = "Linux";
pub const CLIENT_PROPERTIES_BROWSER: &str = "Discord Client";
pub const CLIENT_PROPERTIES_DEVICE: &str = "";

pub const LARGE_THRESHOLD: u16 = 250;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatewayOpcode {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    PresenceUpdate = 3,
    VoiceStateUpdate = 4,
    Resume = 6,
    Reconnect = 7,
    RequestGuildMembers = 8,
    InvalidSession = 9,
    Hello = 10,
    HeartbeatAck = 11,
    /// Opcode 14: Subscribe to guild events (typing, presence, etc.)
    LazyRequest = 14,
}

impl GatewayOpcode {
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Dispatch),
            1 => Some(Self::Heartbeat),
            2 => Some(Self::Identify),
            3 => Some(Self::PresenceUpdate),
            4 => Some(Self::VoiceStateUpdate),
            6 => Some(Self::Resume),
            7 => Some(Self::Reconnect),
            8 => Some(Self::RequestGuildMembers),
            9 => Some(Self::InvalidSession),
            10 => Some(Self::Hello),
            11 => Some(Self::HeartbeatAck),
            14 => Some(Self::LazyRequest),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<GatewayOpcode> for u8 {
    fn from(opcode: GatewayOpcode) -> Self {
        opcode.as_u8()
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatewayIntent {
    Guilds = 1 << 0,
    GuildMembers = 1 << 1,
    GuildModeration = 1 << 2,
    GuildExpressions = 1 << 3,
    GuildIntegrations = 1 << 4,
    GuildWebhooks = 1 << 5,
    GuildInvites = 1 << 6,
    GuildVoiceStates = 1 << 7,
    GuildPresences = 1 << 8,
    GuildMessages = 1 << 9,
    GuildMessageReactions = 1 << 10,
    GuildMessageTyping = 1 << 11,
    DirectMessages = 1 << 12,
    DirectMessageReactions = 1 << 13,
    DirectMessageTyping = 1 << 14,
    MessageContent = 1 << 15,
    GuildScheduledEvents = 1 << 16,
    AutoModerationConfiguration = 1 << 20,
    AutoModerationExecution = 1 << 21,
    GuildMessagePolls = 1 << 24,
    DirectMessagePolls = 1 << 25,
}

impl GatewayIntent {
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

impl From<GatewayIntent> for u32 {
    fn from(intent: GatewayIntent) -> Self {
        intent.as_u32()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GatewayIntents(u32);

impl GatewayIntents {
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn with(mut self, intent: GatewayIntent) -> Self {
        self.0 |= intent.as_u32();
        self
    }

    #[must_use]
    pub const fn has(self, intent: GatewayIntent) -> bool {
        (self.0 & intent.as_u32()) != 0
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn default_client() -> Self {
        Self::new()
            .with(GatewayIntent::Guilds)
            .with(GatewayIntent::GuildMessages)
            .with(GatewayIntent::GuildMessageTyping)
            .with(GatewayIntent::DirectMessages)
            .with(GatewayIntent::DirectMessageTyping)
            .with(GatewayIntent::MessageContent)
    }

    #[must_use]
    pub const fn with_presence(self) -> Self {
        self.with(GatewayIntent::GuildPresences)
    }

    #[must_use]
    pub const fn with_reactions(self) -> Self {
        self.with(GatewayIntent::GuildMessageReactions)
            .with(GatewayIntent::DirectMessageReactions)
    }
}

impl From<GatewayIntents> for u32 {
    fn from(intents: GatewayIntents) -> Self {
        intents.as_u32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_roundtrip() {
        for opcode in [
            GatewayOpcode::Dispatch,
            GatewayOpcode::Heartbeat,
            GatewayOpcode::Identify,
            GatewayOpcode::Hello,
            GatewayOpcode::HeartbeatAck,
        ] {
            let value = opcode.as_u8();
            assert_eq!(GatewayOpcode::from_u8(value), Some(opcode));
        }
    }

    #[test]
    fn test_intents_builder() {
        let intents = GatewayIntents::default_client();
        assert!(intents.has(GatewayIntent::Guilds));
        assert!(intents.has(GatewayIntent::GuildMessages));
        assert!(!intents.has(GatewayIntent::GuildPresences));

        let with_presence = intents.with_presence();
        assert!(with_presence.has(GatewayIntent::GuildPresences));
    }

    #[test]
    fn test_intents_value() {
        let intents = GatewayIntents::new()
            .with(GatewayIntent::Guilds)
            .with(GatewayIntent::GuildMessages);

        let expected = (1 << 0) | (1 << 9);
        assert_eq!(intents.as_u32(), expected);
    }
}
