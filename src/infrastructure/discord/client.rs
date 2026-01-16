//! Discord API HTTP client.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode, header};
use tracing::{debug, warn};

use super::dto::{
    AttachmentResponse, ChannelResponse, DmChannelResponse, ErrorResponse, GuildResponse,
    MessageReferencePayload, MessageResponse, SendMessagePayload, UserResponse,
};
use crate::domain::entities::{
    Attachment, AuthToken, Channel, ChannelId, ChannelKind, Guild, Message, MessageAuthor,
    MessageKind, MessageReference, User,
};
use crate::domain::errors::AuthError;
use crate::domain::ports::{
    AuthPort, DirectMessageChannel, DiscordDataPort, FetchMessagesOptions, SendMessageRequest,
};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const MAX_IDLE_CONNECTIONS: usize = 10;
const DEFAULT_MESSAGE_LIMIT: u8 = 50;

/// Discord API client for authentication and data fetching.
pub struct DiscordClient {
    client: Client,
    base_url: String,
}

impl DiscordClient {
    /// Creates a new client with the default Discord API base URL.
    ///
    /// # Errors
    /// Returns error if HTTP client creation fails.
    pub fn new() -> Result<Self, AuthError> {
        Self::with_base_url(DISCORD_API_BASE)
    }

    /// Creates a client with a custom base URL.
    ///
    /// # Errors
    /// Returns error if HTTP client creation fails.
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, AuthError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(MAX_IDLE_CONNECTIONS)
            .build()
            .map_err(|e| AuthError::unexpected(format!("failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }

    async fn handle_error_response(
        &self,
        status: StatusCode,
        response: reqwest::Response,
    ) -> AuthError {
        let error_message = match response.json::<ErrorResponse>().await {
            Ok(error) => error.message,
            Err(_) => format!("HTTP {status}"),
        };

        match status {
            StatusCode::UNAUTHORIZED => AuthError::rejected("invalid or expired token"),
            StatusCode::FORBIDDEN => AuthError::rejected(format!("access denied: {error_message}")),
            StatusCode::TOO_MANY_REQUESTS => AuthError::RateLimited {
                retry_after_ms: 5000,
            },
            StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
                AuthError::network("Discord API is temporarily unavailable")
            }
            _ => AuthError::unexpected(format!("unexpected response: {status} - {error_message}")),
        }
    }

    fn parse_channels(channel_responses: Vec<ChannelResponse>, guild_id: u64) -> Vec<Channel> {
        channel_responses
            .into_iter()
            .filter_map(|c| {
                let id: u64 = c.id.parse().ok()?;
                let name = c.name.unwrap_or_default();
                let kind: ChannelKind = c.kind.into();

                let mut channel = Channel::new(id, name, kind)
                    .with_guild(guild_id)
                    .with_position(c.position);

                if let Some(parent_id) = c.parent_id
                    && let Ok(pid) = parent_id.parse::<u64>()
                {
                    channel = channel.with_parent(pid);
                }

                if let Some(topic) = c.topic
                    && !topic.is_empty()
                {
                    channel = channel.with_topic(topic);
                }

                Some(channel)
            })
            .collect()
    }

    fn parse_attachment(attachment: AttachmentResponse) -> Attachment {
        let mut result = Attachment::new(
            attachment.id,
            attachment.filename,
            attachment.size,
            attachment.url,
        );
        if let Some(content_type) = attachment.content_type {
            result = result.with_content_type(content_type);
        }
        result
    }

    fn parse_message_response(response: MessageResponse, channel_id: u64) -> Option<Message> {
        let id: u64 = response.id.parse().ok()?;
        let author = MessageAuthor::new(
            response.author.id,
            response.author.username,
            response.author.discriminator,
            response.author.avatar,
            response.author.bot,
        );

        let timestamp: DateTime<Utc> = response.timestamp.parse().ok()?;

        let mut message = Message::new(id, channel_id, author, response.content, timestamp)
            .with_kind(MessageKind::from(response.kind))
            .with_pinned(response.pinned);

        if !response.attachments.is_empty() {
            let attachments = response
                .attachments
                .into_iter()
                .map(Self::parse_attachment)
                .collect();
            message = message.with_attachments(attachments);
        }

        if let Some(edited) = response.edited_timestamp
            && let Ok(edited_ts) = edited.parse::<DateTime<Utc>>()
        {
            message = message.with_edited_timestamp(edited_ts);
        }

        if let Some(reference) = response.message_reference {
            let ref_msg_id = reference.message_id.and_then(|id| id.parse::<u64>().ok());
            let ref_channel_id = reference.channel_id.and_then(|id| id.parse::<u64>().ok());
            message = message.with_reference(MessageReference::new(
                ref_msg_id.map(Into::into),
                ref_channel_id.map(Into::into),
            ));
        }

        if let Some(referenced) = response.referenced_message
            && let Some(ref_message) = Self::parse_message_response(*referenced, channel_id)
        {
            message = message.with_referenced(ref_message);
        }

        Some(message)
    }
}

#[async_trait]
impl AuthPort for DiscordClient {
    async fn validate_token(&self, token: &AuthToken) -> Result<User, AuthError> {
        let url = format!("{}/users/@me", self.base_url);

        debug!("Validating token against Discord API");

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to connect to Discord API");
                if e.is_timeout() {
                    AuthError::network("request timed out")
                } else if e.is_connect() {
                    AuthError::network("failed to connect to Discord")
                } else {
                    AuthError::network(e.to_string())
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let user_response: UserResponse = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse user response");
            AuthError::unexpected(format!("failed to parse response: {e}"))
        })?;

        debug!(
            user_id = %user_response.id,
            username = %user_response.username,
            "Token validated successfully"
        );

        Ok(User::new(
            user_response.id,
            user_response.username,
            user_response.discriminator,
            user_response.avatar,
            user_response.bot,
        ))
    }

    async fn health_check(&self) -> Result<(), AuthError> {
        let url = format!("{}/gateway", self.base_url);

        debug!("Performing Discord API health check");

        let response = self.client.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                AuthError::network("request timed out")
            } else if e.is_connect() {
                AuthError::network("failed to connect to Discord")
            } else {
                AuthError::network(e.to_string())
            }
        })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(AuthError::network(format!(
                "Discord API returned {}",
                response.status()
            )))
        }
    }
}

#[async_trait]
impl DiscordDataPort for DiscordClient {
    async fn fetch_guilds(&self, token: &AuthToken) -> Result<Vec<Guild>, AuthError> {
        let url = format!("{}/users/@me/guilds", self.base_url);

        debug!("Fetching user guilds from Discord API");

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to fetch guilds");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let guild_responses: Vec<GuildResponse> = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse guilds response");
            AuthError::unexpected(format!("failed to parse guilds: {e}"))
        })?;

        debug!(
            raw_count = guild_responses.len(),
            "Fetched raw guild responses from Discord API"
        );

        let guilds: Vec<Guild> = guild_responses
            .into_iter()
            .filter_map(|g| {
                match g.id.parse::<u64>() {
                    Ok(id) => {
                        let mut guild = Guild::new(id, g.name);
                        if let Some(icon) = g.icon {
                            guild = guild.with_icon(icon);
                        }
                        Some(guild)
                    }
                    Err(e) => {
                        warn!(guild_id = %g.id, guild_name = %g.name, error = %e, "Failed to parse guild ID, skipping guild");
                        None
                    }
                }
            })
            .collect();

        debug!(parsed_count = guilds.len(), "Successfully parsed guilds");

        Ok(guilds)
    }

    async fn fetch_channels(
        &self,
        token: &AuthToken,
        guild_id: u64,
    ) -> Result<Vec<Channel>, AuthError> {
        let url = format!("{}/guilds/{}/channels", self.base_url, guild_id);

        debug!(guild_id = guild_id, "Fetching channels from Discord API");

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to fetch channels");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let channel_responses: Vec<ChannelResponse> = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse channels response");
            AuthError::unexpected(format!("failed to parse channels: {e}"))
        })?;

        debug!(
            count = channel_responses.len(),
            guild_id = guild_id,
            "Fetched channels successfully"
        );

        Ok(Self::parse_channels(channel_responses, guild_id))
    }

    async fn fetch_dm_channels(
        &self,
        token: &AuthToken,
    ) -> Result<Vec<DirectMessageChannel>, AuthError> {
        let url = format!("{}/users/@me/channels", self.base_url);

        debug!("Fetching DM channels from Discord API");

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to fetch DM channels");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let dm_responses: Vec<DmChannelResponse> = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse DM channels response");
            AuthError::unexpected(format!("failed to parse DM channels: {e}"))
        })?;

        debug!(
            count = dm_responses.len(),
            "Fetched DM channels successfully"
        );

        let dm_channels = dm_responses
            .into_iter()
            .filter_map(|dm| {
                let recipient = dm.recipients.first()?;
                let display_name = recipient
                    .global_name
                    .clone()
                    .unwrap_or_else(|| recipient.username.clone());
                Some(DirectMessageChannel {
                    channel_id: dm.id,
                    recipient_id: recipient.id.clone(),
                    recipient_name: display_name,
                })
            })
            .collect();

        Ok(dm_channels)
    }

    async fn fetch_messages(
        &self,
        token: &AuthToken,
        channel_id: u64,
        options: FetchMessagesOptions,
    ) -> Result<Vec<Message>, AuthError> {
        let mut url = format!("{}/channels/{}/messages", self.base_url, channel_id);
        let mut query_parts = Vec::new();

        let limit = options.limit.unwrap_or(DEFAULT_MESSAGE_LIMIT);
        query_parts.push(format!("limit={}", limit.min(100)));

        if let Some(before) = options.before {
            query_parts.push(format!("before={before}"));
        }
        if let Some(after) = options.after {
            query_parts.push(format!("after={after}"));
        }
        if let Some(around) = options.around {
            query_parts.push(format!("around={around}"));
        }

        if !query_parts.is_empty() {
            url = format!("{}?{}", url, query_parts.join("&"));
        }

        debug!(
            channel_id = channel_id,
            "Fetching messages from Discord API"
        );

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to fetch messages");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let message_responses: Vec<MessageResponse> = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse messages response");
            AuthError::unexpected(format!("failed to parse messages: {e}"))
        })?;

        debug!(
            count = message_responses.len(),
            channel_id = channel_id,
            "Fetched messages successfully"
        );

        let mut messages: Vec<Message> = message_responses
            .into_iter()
            .filter_map(|m| Self::parse_message_response(m, channel_id))
            .collect();

        messages.reverse();

        Ok(messages)
    }

    async fn send_message(
        &self,
        token: &AuthToken,
        request: SendMessageRequest,
    ) -> Result<Message, AuthError> {
        let url = format!(
            "{}/channels/{}/messages",
            self.base_url,
            request.channel_id.as_u64()
        );

        debug!(
            channel_id = %request.channel_id,
            has_reply = request.reply_to.is_some(),
            "Sending message to Discord API"
        );

        let payload = SendMessagePayload {
            content: request.content,
            message_reference: request.reply_to.map(|id| MessageReferencePayload {
                message_id: id.as_u64().to_string(),
            }),
        };

        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to send message");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() {
            return Err(self.handle_error_response(status, response).await);
        }

        let message_response: MessageResponse = response.json().await.map_err(|e| {
            warn!(error = %e, "Failed to parse message response");
            AuthError::unexpected(format!("failed to parse message response: {e}"))
        })?;

        debug!(message_id = %message_response.id, "Message sent successfully");

        Self::parse_message_response(message_response, request.channel_id.as_u64())
            .ok_or_else(|| AuthError::unexpected("failed to parse sent message"))
    }

    async fn send_typing_indicator(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
    ) -> Result<(), AuthError> {
        let url = format!("{}/channels/{}/typing", self.base_url, channel_id.as_u64());

        debug!(channel_id = %channel_id, "Sending typing indicator");

        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to send typing indicator");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(self.handle_error_response(status, response).await);
        }

        Ok(())
    }
}

impl Default for DiscordClient {
    fn default() -> Self {
        Self::new().expect("failed to create default Discord client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::discord::dto::GuildResponse;

    #[test]
    fn test_client_creation() {
        let client = DiscordClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_guild_response_parsing() {
        let json = r#"[
            {"id": "1234567890123456789", "name": "Nix/NixOS (unofficial)", "icon": "abc123", "owner": true, "permissions": "36953089", "features": ["COMMUNITY"]},
            {"id": "2234567890123456789", "name": "r/Unixporn", "icon": null, "owner": false, "permissions": "36953089", "features": []},
            {"id": "3234567890123456789", "name": "RespriteApp", "icon": "def456", "owner": false, "permissions": "36953089", "features": ["ANIMATED_ICON"]},
            {"id": "4234567890123456789", "name": "Discordo", "icon": null, "owner": true, "permissions": "36953089", "features": []},
            {"id": "5234567890123456789", "name": "Noctalia", "icon": "ghi789", "owner": false, "permissions": "36953089", "features": []},
            {"id": "6234567890123456789", "name": "L I N U X's TEST SERVER", "icon": null, "owner": true, "permissions": "36953089", "features": []},
            {"id": "7234567890123456789", "name": "OpenCode", "icon": "jkl012", "owner": false, "permissions": "36953089", "features": []},
            {"id": "8234567890123456789", "name": "OpenCode Antigravity Auth", "icon": null, "owner": false, "permissions": "36953089", "features": []}
        ]"#;

        let responses: Vec<GuildResponse> =
            serde_json::from_str(json).expect("Should parse guild JSON");
        assert_eq!(responses.len(), 8, "All 8 guilds should be parsed");

        let guilds: Vec<Guild> = responses
            .into_iter()
            .filter_map(|g| {
                let id: u64 = g.id.parse().ok()?;
                let mut guild = Guild::new(id, g.name);
                if let Some(icon) = g.icon {
                    guild = guild.with_icon(icon);
                }
                Some(guild)
            })
            .collect();

        assert_eq!(guilds.len(), 8, "All 8 guilds should be converted");
        assert_eq!(guilds[0].name(), "Nix/NixOS (unofficial)");
        assert_eq!(guilds[2].name(), "RespriteApp");
        assert_eq!(guilds[3].name(), "Discordo");
        assert_eq!(guilds[4].name(), "Noctalia");
    }

    #[test]
    fn test_guild_response_with_extra_fields() {
        let json = r#"[
            {
                "id": "1234567890123456789",
                "name": "Test Guild",
                "icon": null,
                "banner": "banner_hash",
                "owner": true,
                "permissions": "36953089",
                "features": ["COMMUNITY", "NEWS"],
                "approximate_member_count": 1000,
                "approximate_presence_count": 500,
                "some_future_field": "unknown"
            }
        ]"#;

        let responses: Vec<GuildResponse> =
            serde_json::from_str(json).expect("Should parse guild JSON with extra fields");
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].name, "Test Guild");
    }
}
