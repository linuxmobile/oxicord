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
    /// User relationships (friends, blocked, pending).
    #[serde(default)]
    pub relationships: Vec<RelationshipPayload>,
}

#[derive(Debug, Deserialize)]
pub struct UserSettingsPayload {
    #[serde(default)]
    pub guild_folders: Vec<GuildFolderPayload>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GuildFolderPayload {
    #[serde(default, deserialize_with = "deserialize_option_string_or_int")]
    pub id: Option<String>,
    pub name: Option<String>,
    pub color: Option<u64>,
    #[serde(default)]
    pub guild_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadStatePayload {
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_option_string_or_int")]
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
    #[serde(default)]
    pub threads: Vec<ChannelPayload>,
    #[serde(default)]
    pub roles: Vec<RolePayload>,
    #[serde(default)]
    pub members: Vec<GuildMemberPayload>,
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
    #[serde(default, deserialize_with = "deserialize_option_string_or_int")]
    pub guild_id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default, deserialize_with = "deserialize_option_string_or_int")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    pub topic: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string_or_int")]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwritePayload>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionOverwritePayload {
    pub id: String,
    #[serde(rename = "type")]
    pub overwrite_type: u8,
    pub allow: String,
    pub deny: String,
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
    #[serde(default)]
    pub roles: Vec<RolePayload>,
    #[serde(default)]
    pub members: Vec<GuildMemberPayload>,
}

#[derive(Debug, Deserialize)]
pub struct RolePayload {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub hoist: bool,
    pub icon: Option<String>,
    pub unicode_emoji: Option<String>,
    pub position: i32,
    pub permissions: String,
    pub managed: bool,
    pub mentionable: bool,
}

#[derive(Debug, Deserialize)]
pub struct GuildMemberPayload {
    pub user: Option<UserPayload>,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub roles: Vec<String>,
    pub joined_at: String,
    pub premium_since: Option<String>,
    #[serde(default)]
    pub deaf: bool,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub pending: bool,
    pub permissions: Option<String>,
    pub communication_disabled_until: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserPayload {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
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

/// Relationship payload from READY event or `RELATIONSHIP_ADD`.
#[derive(Debug, Deserialize)]
pub struct RelationshipPayload {
    /// User ID this relationship is with.
    pub id: String,
    /// Relationship type (1 = friend, 2 = blocked, 3 = pending incoming, 4 = pending outgoing).
    #[serde(rename = "type", default)]
    pub relationship_type: u8,
}

/// Relationship remove payload.
#[derive(Debug, Deserialize)]
pub struct RelationshipRemovePayload {
    pub id: String,
}

fn deserialize_option_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptionStringOrIntVisitor;

    impl<'de> serde::de::Visitor<'de> for OptionStringOrIntVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, integer, or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct StringOrIntVisitor;

            impl serde::de::Visitor<'_> for StringOrIntVisitor {
                type Value = Option<String>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a string or integer")
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(Some(v.to_string()))
                }

                fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(Some(v))
                }

                fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    if v == 0 {
                        Ok(None)
                    } else {
                        Ok(Some(v.to_string()))
                    }
                }

                fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    if v == 0 {
                        Ok(None)
                    } else {
                        Ok(Some(v.to_string()))
                    }
                }
            }

            deserializer.deserialize_any(StringOrIntVisitor)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_option(OptionStringOrIntVisitor)
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

    #[test]
    fn test_guild_folder_payload_with_negative_string_id() {
        let json = r#"{
            "id": "-6757263541388715000",
            "guild_ids": [],
            "name": null,
            "color": null
        }"#;

        let folder: Result<GuildFolderPayload, _> = serde_json::from_str(json);
        match folder {
            Ok(f) => {
                assert_eq!(f.id, Some("-6757263541388715000".to_string()));
            }
            Err(e) => panic!("Should succeed parsing negative string ID: {e}"),
        }
    }
}
