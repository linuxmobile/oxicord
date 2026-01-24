//! Discord API response DTOs.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub global_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GuildResponse {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub icon: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub owner: bool,
    #[serde(default)]
    #[allow(dead_code)]
    pub permissions: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub features: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub splash: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PermissionOverwriteDto {
    pub id: String,
    #[serde(rename = "type")]
    pub overwrite_type: u8,
    #[serde(default)]
    pub allow: String,
    #[serde(default)]
    pub deny: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChannelResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    #[allow(dead_code)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub owner_id: Option<String>,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub topic: Option<String>,
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub message_count: Option<u32>,
    #[serde(default)]
    pub member_count: Option<u32>,
    #[serde(default)]
    pub applied_tags: Vec<String>,
    pub thread_metadata: Option<ThreadMetadataDto>,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub user_limit: Option<u32>,
    #[serde(default)]
    pub rate_limit_per_user: Option<u16>,
    #[serde(default)]
    pub flags: Option<u64>,
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwriteDto>,
    #[serde(default)]
    pub rtc_region: Option<String>,
    #[serde(default)]
    pub video_quality_mode: Option<u8>,
    #[serde(default)]
    pub default_auto_archive_duration: Option<u16>,
    #[serde(default)]
    pub last_pin_timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ThreadMetadataDto {
    pub archived: bool,
    pub auto_archive_duration: i32,
    pub archive_timestamp: String,
    pub locked: bool,
    #[serde(default)]
    pub invitable: Option<bool>,
    pub create_timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReactionDto {
    pub count: u32,
    pub me: bool,
    pub emoji: ReactionEmojiDto,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReactionEmojiDto {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DmRecipient {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DmChannelResponse {
    pub id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub kind: u8,
    #[serde(default)]
    pub recipients: Vec<DmRecipient>,
    pub last_message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageAuthorResponse {
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
#[allow(dead_code)]
pub struct AttachmentResponse {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    pub url: String,
    pub content_type: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub spoiler: bool,
}

#[derive(Debug, Deserialize)]
pub struct EmbedAuthorDto {
    pub name: Option<String>,
    pub url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedFooterDto {
    pub text: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedFieldDto {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Deserialize)]
pub struct EmbedImageDto {
    pub url: String,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedProviderDto {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedThumbnailDto {
    pub url: String,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedVideoDto {
    pub url: Option<String>,
    pub height: Option<u64>,
    pub width: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct EmbedDto {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub timestamp: Option<String>,
    pub provider: Option<EmbedProviderDto>,
    pub thumbnail: Option<EmbedThumbnailDto>,
    pub author: Option<EmbedAuthorDto>,
    pub footer: Option<EmbedFooterDto>,
    pub image: Option<EmbedImageDto>,
    pub video: Option<EmbedVideoDto>,
    #[serde(default)]
    pub fields: Vec<EmbedFieldDto>,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct MessageReferenceResponse {
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
    #[serde(default)]
    pub embeds: Vec<EmbedDto>,
    pub message_reference: Option<MessageReferenceResponse>,
    pub referenced_message: Option<Box<Self>>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mentions: Vec<MentionUserResponse>,
    #[serde(default)]
    #[allow(dead_code)]
    pub reactions: Vec<ReactionDto>,
    pub member: Option<MemberResponse>,
    #[serde(default)]
    pub flags: Option<u64>,
    #[serde(default)]
    pub tts: bool,
}

#[derive(Debug, Deserialize)]
pub struct MemberResponse {
    pub color: Option<u32>,
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
    pub member: Option<MemberResponse>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ThreadsResponse {
    pub threads: Vec<ChannelResponse>,
    #[serde(default)]
    pub members: Vec<ThreadMemberResponse>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub first_messages: Option<Vec<MessageResponse>>,
    #[serde(default)]
    pub total_results: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ThreadMemberResponse {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub join_timestamp: String,
    pub flags: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_decoding_with_special_chars() {
        let json = r#"{
            "id": "123456789",
            "type": 0,
            "name": "︓·⠄₊⊹",
            "topic": "Aesthetic topic ︓·⠄₊⊹",
            "guild_id": "987654321",
            "position": 1,
            "nsfw": false
        }"#;

        let decoded: ChannelResponse =
            serde_json::from_str(json).expect("Should decode special chars");
        assert_eq!(decoded.name.as_deref(), Some("︓·⠄₊⊹"));
        assert_eq!(decoded.topic.as_deref(), Some("Aesthetic topic ︓·⠄₊⊹"));
    }

    #[test]
    fn test_channel_decoding_missing_fields() {
        let json = r#"{
            "id": "123",
            "type": 0
         }"#;
        let decoded: ChannelResponse =
            serde_json::from_str(json).expect("Should handle missing optional fields");
        assert_eq!(decoded.name, None);
        assert_eq!(decoded.topic, None);
    }

    #[test]
    fn test_large_user_limit_decoding() {
        let json = r#"
            {
                "id": "1407144417625772032",
                "type": 13,
                "last_message_id": "1421571427806478356",
                "flags": 0,
                "guild_id": "1405283270484037762",
                "name": "karaoke event stage!",
                "parent_id": "1405283272103035073",
                "rate_limit_per_user": 0,
                "bitrate": 64000,
                "user_limit": 10000,
                "rtc_region": null,
                "topic": null,
                "position": 0,
                "permission_overwrites": [
                    {
                        "id": "323773153379876864",
                        "type": 1,
                        "allow": "4503599648342032",
                        "deny": "0"
                    }
                ],
                "nsfw": false
            }
        "#;

        let decoded: ChannelResponse =
            serde_json::from_str(json).expect("Should decode large user_limit");
        assert_eq!(decoded.user_limit, Some(10000));
        assert_eq!(decoded.bitrate, Some(64000));
        assert_eq!(decoded.permission_overwrites[0].allow, "4503599648342032");
    }
}
