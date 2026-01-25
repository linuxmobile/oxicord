use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::constants::LARGE_THRESHOLD;
use crate::infrastructure::discord::identity::SuperProperties;

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    pub d: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
}

impl GatewayPayload {
    #[must_use]
    pub fn heartbeat(sequence: Option<u64>) -> Self {
        Self {
            op: 1,
            d: sequence.map_or(Value::Null, |s| Value::Number(s.into())),
            s: None,
            t: None,
        }
    }

    #[must_use]
    pub fn identify(token: &str, intents: u32, props: &SuperProperties) -> Self {
        let properties = IdentifyProperties {
            os: props.os.clone(),
            browser: props.browser.clone(),
            device: props.device.clone(),
            system_locale: props.system_locale.clone(),
            browser_user_agent: props.browser_user_agent.clone(),
            browser_version: props.browser_version.clone(),
            os_version: props.os_version.clone(),
            referrer: props.referrer.clone(),
            referring_domain: props.referring_domain.clone(),
            referrer_current: props.referrer_current.clone(),
            referring_domain_current: props.referring_domain_current.clone(),
            release_channel: props.release_channel.clone(),
            client_build_number: props.client_build_number,
            client_event_source: props.client_event_source.clone(),
            client_version: props.client_version.clone(),
            os_arch: props.os_arch.clone(),
            app_arch: props.app_arch.clone(),
            has_client_mods: props.has_client_mods,
            client_launch_id: props.client_launch_id.clone(),
            launch_signature: props.launch_signature.clone(),
            client_heartbeat_session_id: props.client_heartbeat_session_id.clone(),
            native_build_number: props.native_build_number,
            window_manager: props.window_manager.clone(),
            distro: props.distro.clone(),
        };

        let identify = IdentifyData {
            token: token.to_string(),
            properties,
            compress: true,
            large_threshold: LARGE_THRESHOLD,
            intents,
        };

        Self {
            op: 2,
            d: serde_json::to_value(identify).unwrap_or(Value::Null),
            s: None,
            t: None,
        }
    }

    #[must_use]
    pub fn resume(token: &str, session_id: &str, sequence: u64) -> Self {
        let resume = ResumeData {
            token: token.to_string(),
            session_id: session_id.to_string(),
            seq: sequence,
        };

        Self {
            op: 6,
            d: serde_json::to_value(resume).unwrap_or(Value::Null),
            s: None,
            t: None,
        }
    }

    /// Creates a `LazyRequest` (Opcode 14) payload to subscribe to a guild channel.
    /// This is required for user accounts to receive `TYPING_START` events.
    #[must_use]
    pub fn lazy_request(guild_id: &str, channel_id: &str) -> Self {
        use serde_json::json;

        let data = json!({
            "guild_id": guild_id,
            "typing": true,
            "activities": true,
            "threads": true,
            "channels": {
                channel_id: [[0, 99]]
            }
        });

        Self {
            op: 14,
            d: data,
            s: None,
            t: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct IdentifyData {
    token: String,
    properties: IdentifyProperties,
    compress: bool,
    large_threshold: u16,
    intents: u32,
}

#[derive(Debug, Serialize)]
struct IdentifyProperties {
    #[serde(rename = "$os")]
    os: String,
    #[serde(rename = "$browser")]
    browser: String,
    #[serde(rename = "$device")]
    device: String,
    #[serde(rename = "$system_locale")]
    system_locale: String,
    #[serde(rename = "$browser_user_agent")]
    browser_user_agent: String,
    #[serde(rename = "$browser_version")]
    browser_version: String,
    #[serde(rename = "$os_version")]
    os_version: String,
    #[serde(rename = "$referrer")]
    referrer: String,
    #[serde(rename = "$referring_domain")]
    referring_domain: String,
    #[serde(rename = "$referrer_current")]
    referrer_current: String,
    #[serde(rename = "$referring_domain_current")]
    referring_domain_current: String,
    #[serde(rename = "$release_channel")]
    release_channel: String,
    #[serde(rename = "$client_build_number")]
    client_build_number: u32,
    #[serde(
        rename = "$client_event_source",
        skip_serializing_if = "Option::is_none"
    )]
    client_event_source: Option<String>,
    #[serde(rename = "$client_version")]
    client_version: String,
    #[serde(rename = "$os_arch")]
    os_arch: String,
    #[serde(rename = "$app_arch")]
    app_arch: String,
    #[serde(rename = "$has_client_mods")]
    has_client_mods: bool,
    #[serde(rename = "$client_launch_id")]
    client_launch_id: String,
    #[serde(rename = "$launch_signature")]
    launch_signature: String,
    #[serde(rename = "$client_heartbeat_session_id")]
    client_heartbeat_session_id: String,
    #[serde(
        rename = "$native_build_number",
        skip_serializing_if = "Option::is_none"
    )]
    native_build_number: Option<u32>,
    #[serde(rename = "$window_manager", skip_serializing_if = "Option::is_none")]
    window_manager: Option<String>,
    #[serde(rename = "$distro", skip_serializing_if = "Option::is_none")]
    distro: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResumeData {
    token: String,
    session_id: String,
    seq: u64,
}

#[derive(Debug, Deserialize)]
pub struct GatewayMessage {
    pub op: u8,
    pub d: Option<Value>,
    pub s: Option<u64>,
    pub t: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HelloPayload {
    pub heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReadyPayload {
    pub session_id: String,
    pub resume_gateway_url: Option<String>,
    pub user: ReadyUser,
    #[serde(default)]
    pub guilds: Vec<ReadyGuild>,
    #[serde(default)]
    pub read_state: Vec<ReadStatePayload>,
    #[serde(default)]
    pub user_settings: Option<UserSettingsPayload>,
}

#[derive(Debug, Deserialize)]
pub struct UserSettingsPayload {
    #[serde(default)]
    pub guild_folders: Vec<GuildFolderPayload>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GuildFolderPayload {
    #[serde(default)]
    pub id: Option<u64>,
    pub name: Option<String>,
    pub color: Option<u64>,
    #[serde(default)]
    pub guild_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadStatePayload {
    pub id: String,
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub mention_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct ReadyUser {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadyGuild {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: Option<String>,
    #[serde(default)]
    pub unavailable: bool,
    #[serde(default)]
    pub channels: Vec<ChannelPayload>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MessagePayload {
    pub id: String,
    pub channel_id: String,
    pub author: AuthorPayload,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub attachments: Vec<AttachmentPayload>,
    pub message_reference: Option<MessageReferencePayload>,
    pub referenced_message: Option<Box<Self>>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mentions: Vec<MentionUserPayload>,
    pub member: Option<MemberPayload>,
}

#[derive(Debug, Deserialize)]
pub struct MemberPayload {
    pub color: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MentionUserPayload {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    pub member: Option<MemberPayload>,
}

#[derive(Debug, Deserialize)]
pub struct AuthorPayload {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub global_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttachmentPayload {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    pub url: String,
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageReferencePayload {
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeletePayload {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeleteBulkPayload {
    pub ids: Vec<String>,
    pub channel_id: String,
    pub guild_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TypingStartPayload {
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub user_id: String,
    pub timestamp: i64,
    pub member: Option<TypingMemberPayload>,
}

#[derive(Debug, Deserialize)]
pub struct TypingMemberPayload {
    pub user: Option<TypingUserPayload>,
    pub nick: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TypingUserPayload {
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub global_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PresenceUpdatePayload {
    pub user: PresenceUserPayload,
    pub guild_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub activities: Vec<ActivityPayload>,
}

#[derive(Debug, Deserialize)]
pub struct PresenceUserPayload {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ActivityPayload {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: u8,
    pub details: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReactionPayload {
    pub user_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub guild_id: Option<String>,
    pub emoji: EmojiPayload,
}

#[derive(Debug, Deserialize)]
pub struct EmojiPayload {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReactionRemoveAllPayload {
    #[serde(rename = "channel_id")]
    pub channel: String,
    #[serde(rename = "message_id")]
    pub message: String,
    #[serde(rename = "guild_id")]
    pub guild: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelPayload {
    pub id: String,
    pub guild_id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: u8,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    pub topic: Option<String>,
    pub last_message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GuildCreatePayload {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub unavailable: bool,
    #[serde(default)]
    pub channels: Vec<ChannelPayload>,
    #[serde(default)]
    pub threads: Vec<ChannelPayload>,
}

#[derive(Debug, Deserialize)]
pub struct GuildDeletePayload {
    pub id: String,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Debug, Deserialize)]
pub struct UserUpdatePayload {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[allow(clippy::struct_excessive_bools)]
pub struct VoiceStateUpdatePayload {
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub user_id: String,
    pub member: Option<MemberPayload>,
    pub session_id: String,
    pub deaf: bool,
    pub mute: bool,
    pub self_deaf: bool,
    pub self_mute: bool,
    pub self_video: bool,
    pub suppress: bool,
    pub request_to_speak_timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct VoiceServerUpdatePayload {
    pub token: String,
    pub guild_id: String,
    pub endpoint: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_payload() {
        let payload = GatewayPayload::heartbeat(Some(42));
        assert_eq!(payload.op, 1);
        assert_eq!(payload.d, Value::Number(42.into()));
    }

    #[test]
    fn test_heartbeat_null_sequence() {
        let payload = GatewayPayload::heartbeat(None);
        assert_eq!(payload.d, Value::Null);
    }

    #[test]
    fn test_identify_payload_structure() {
        let payload = GatewayPayload::identify("test_token", 513, &SuperProperties::default());
        assert_eq!(payload.op, 2);
        assert!(payload.d.is_object());

        let obj = payload.d.as_object().unwrap();
        assert!(obj.contains_key("token"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("intents"));
    }

    #[test]
    fn test_resume_payload() {
        let payload = GatewayPayload::resume("token", "session123", 100);
        assert_eq!(payload.op, 6);

        let obj = payload.d.as_object().unwrap();
        assert_eq!(obj.get("session_id").unwrap(), "session123");
        assert_eq!(obj.get("seq").unwrap(), 100);
    }
}
