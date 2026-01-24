//! Discord API HTTP client.

use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
use reqwest::{Client, Method, StatusCode, header};
use std::sync::Arc;
use tracing::{debug, warn};

use super::dto::{
    AttachmentResponse, ChannelResponse, DmChannelResponse, EditMessagePayload, EmbedDto,
    ErrorResponse, GuildResponse, MessageReferencePayload, MessageResponse, SendMessagePayload,
    UserResponse,
};
use super::identity::ClientIdentity;
use super::scraper;
use crate::domain::entities::{
    Attachment, AuthToken, Channel, ChannelId, ChannelKind, Embed, EmbedProvider, EmbedThumbnail,
    ForumThread, Guild, GuildId, Message, MessageAuthor, MessageId, ReadState,
    User,
};
use crate::domain::errors::AuthError;
use crate::domain::ports::{
    AuthPort, DirectMessageChannel, DiscordDataPort, EditMessageRequest, FetchMessagesOptions,
    SendMessageRequest,
};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const MAX_IDLE_CONNECTIONS: usize = 10;
const DEFAULT_MESSAGE_LIMIT: u8 = 50;

/// Discord API client for authentication and data fetching.
pub struct DiscordClient {
    client: Client,
    base_url: String,
    pub identity: Arc<ClientIdentity>,
}

impl DiscordClient {
    /// Creates a new client with the default Discord API base URL.
    ///
    /// # Errors
    /// Returns error if HTTP client creation fails.
    pub fn new() -> Result<Self, AuthError> {
        let identity = Arc::new(ClientIdentity::new());
        let id_clone = identity.clone();

        tokio::spawn(async move {
            if let Some(build) = scraper::fetch_latest_build_number().await {
                id_clone.update_build_number(build);
            }
        });

        Self::with_base_url(DISCORD_API_BASE, identity)
    }

    /// Creates a client with a custom base URL.
    ///
    /// # Errors
    /// Returns error if HTTP client creation fails.
    pub fn with_base_url(base_url: impl Into<String>, identity: Arc<ClientIdentity>) -> Result<Self, AuthError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(MAX_IDLE_CONNECTIONS)
            .build()
            .map_err(|e| AuthError::unexpected(format!("failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            identity,
        })
    }

    fn build_request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.client.request(method, url)
            .header(header::USER_AGENT, &self.identity.get_props().browser_user_agent)
            .header(header::ACCEPT_LANGUAGE, "en-US")
            .header("X-Discord-Locale", "en-US")
            .header(header::REFERER, "https://discord.com/channels/@me")
            .header("X-Super-Properties", self.identity.get_header_value())
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
                    .with_position(c.position)
                    .with_nsfw(c.nsfw);

                if let Some(last_message_id) = c.last_message_id
                    && let Ok(lmid) = last_message_id.parse::<u64>()
                {
                    channel = channel.with_last_message_id(Some(lmid.into()));
                }

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

                if let Some(bitrate) = c.bitrate {
                    channel = channel.with_bitrate(bitrate);
                }

                if let Some(user_limit) = c.user_limit {
                    channel = channel.with_user_limit(user_limit);
                }

                if let Some(rate_limit) = c.rate_limit_per_user {
                    channel = channel.with_rate_limit_per_user(rate_limit);
                }

                if let Some(flags) = c.flags {
                    channel = channel.with_flags(crate::domain::entities::ChannelFlags::from_bits_truncate(flags));
                }

                if let Some(rtc_region) = c.rtc_region {
                    channel = channel.with_rtc_region(rtc_region);
                }

                if let Some(video_mode) = c.video_quality_mode {
                    channel = channel.with_video_quality_mode(video_mode.into());
                }

                if let Some(auto_archive) = c.default_auto_archive_duration {
                    channel = channel.with_default_auto_archive_duration(auto_archive);
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

    fn parse_embed(embed: EmbedDto) -> Embed {
        Embed {
            title: embed.title,
            description: embed.description,
            url: embed.url,
            color: embed.color,
            timestamp: embed.timestamp,
            provider: embed.provider.map(|p| EmbedProvider {
                name: p.name,
                url: p.url,
            }),
            thumbnail: embed.thumbnail.map(|t| EmbedThumbnail {
                url: t.url,
                proxy_url: None, // DTO doesn't have proxy_url yet, effectively
                height: t.height,
                width: t.width,
            }),
            author: embed.author.map(|a| crate::domain::entities::EmbedAuthor {
                name: a.name.unwrap_or_default(),
                url: a.url,
                icon_url: a.icon_url,
                proxy_icon_url: None,
            }),
            footer: embed.footer.map(|f| crate::domain::entities::EmbedFooter {
                text: f.text,
                icon_url: f.icon_url,
                proxy_icon_url: None,
            }),
            image: embed.image.map(|i| crate::domain::entities::EmbedImage {
                url: i.url,
                proxy_url: None,
                height: i.height,
                width: i.width,
            }),
            video: embed.video.map(|v| crate::domain::entities::EmbedVideo {
                url: v.url,
                proxy_url: None,
                height: v.height,
                width: v.width,
            }),
            fields: embed
                .fields
                .into_iter()
                .map(|f| crate::domain::entities::EmbedField {
                    name: f.name,
                    value: f.value,
                    inline: f.inline,
                })
                .collect(),
        }
    }

    fn parse_message_response(response: MessageResponse, channel_id: u64) -> Option<Message> {
        let MessageResponse {
            id,
            author,
            content,
            timestamp,
            edited_timestamp,
            kind,
            attachments,
            embeds,
            message_reference,
            referenced_message,
            pinned,
            mentions,
            member: _,
            reactions,
            flags,
            ..
        } = response;

        let id: u64 = id.parse().ok()?;
        let message_author = MessageAuthor {
            id: author.id,
            username: author.username,
            discriminator: author.discriminator,
            avatar: author.avatar,
            bot: author.bot,
            global_name: author.global_name,
        };

        let timestamp: DateTime<Utc> = timestamp.parse().ok()?;

        let mut message = Message::new(
            id.into(),
            channel_id.into(),
            message_author,
            content,
            timestamp.into(),
            kind.into(),
        )
        .with_pinned(pinned);

        if let Some(r) = message_reference {
             let mr = crate::domain::entities::MessageReference {
                 message_id: r.message_id.and_then(|id| id.parse::<u64>().ok().map(Into::into)),
                 channel_id: r.channel_id.and_then(|id| id.parse::<u64>().ok().map(Into::into)),
                 guild_id: r.guild_id.and_then(|id| id.parse::<u64>().ok()),
             };
             message = message.with_reference(mr);
        }

        if let Some(ref_msg) = referenced_message {
             // referenced_message is Box<MessageResponse>
             let ref_cid = ref_msg.channel_id.parse().unwrap_or(channel_id);
             if let Some(parsed_ref) = Self::parse_message_response(*ref_msg, ref_cid) {
                 message = message.with_referenced_message(Some(parsed_ref));
             }
        }

        if !attachments.is_empty() {
            let attachments = attachments.into_iter().map(Self::parse_attachment).collect();
            message = message.with_attachments(attachments);
        }

        if !embeds.is_empty() {
            let embeds = embeds.into_iter().map(Self::parse_embed).collect();
            message = message.with_embeds(embeds);
        }

        if !mentions.is_empty() {
             let mentions = mentions.into_iter().map(|m| {
                 crate::domain::entities::User::new(
                    m.id,
                    m.username,
                    m.discriminator,
                    m.avatar,
                    m.bot,
                    m.member.and_then(|mb| mb.color),
                 )
            }).collect();
            message = message.with_mentions(mentions);
        }

        if !reactions.is_empty() {
             let reactions = reactions.into_iter().map(|r| {
                 crate::domain::entities::Reaction {
                     count: r.count,
                     me: r.me,
                     emoji: crate::domain::entities::ReactionEmoji {
                         id: r.emoji.id,
                         name: r.emoji.name,
                     }
                 }
            }).collect();
            message = message.with_reactions(reactions);
        }

        if let Some(f) = flags 
            && let Some(message_flags) = crate::domain::entities::MessageFlags::from_bits(f) {
                message = message.with_flags(message_flags);
        }

        if let Some(edited) = edited_timestamp
            && let Ok(edited_ts) = edited.parse::<DateTime<Utc>>()
        {
            message = message.with_edited_timestamp(edited_ts.with_timezone(&Local));
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
            .build_request(Method::GET, &url)
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
            None,
        ))
    }

    async fn health_check(&self) -> Result<(), AuthError> {
        let url = format!("{}/gateway", self.base_url);

        debug!("Performing Discord API health check");

        let response = self.build_request(Method::GET, &url).send().await.map_err(|e| {
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
            .build_request(Method::GET, &url)
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
            .build_request(Method::GET, &url)
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
            .build_request(Method::GET, &url)
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

    async fn fetch_read_states(&self, _token: &AuthToken) -> Result<Vec<ReadState>, AuthError> {
        Ok(Vec::new())
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
            .build_request(Method::GET, &url)
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

    async fn load_more_before_id(
        &self,
        token: &AuthToken,
        channel_id: u64,
        message_id: u64,
        limit: u8,
    ) -> Result<Vec<Message>, AuthError> {
        let options = FetchMessagesOptions::default()
            .with_limit(limit)
            .before_message(message_id);

        self.fetch_messages(token, channel_id, options).await
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
            attachment_count = request.attachments.len(),
            "Sending message to Discord API"
        );

        let payload = SendMessagePayload {
            content: if request.content.is_empty() {
                None
            } else {
                Some(request.content)
            },
            message_reference: request.reply_to.map(|id| MessageReferencePayload {
                message_id: id.as_u64().to_string(),
            }),
        };

        let mut request_builder = self
            .build_request(Method::POST, &url)
            .header(header::AUTHORIZATION, token.as_str());

        if request.attachments.is_empty() {
            request_builder = request_builder
                .header(header::CONTENT_TYPE, "application/json")
                .json(&payload);
        } else {
            use reqwest::multipart::{Form, Part};
            let mut form = Form::new();

            let json_payload = serde_json::to_string(&payload)
                .map_err(|e| AuthError::unexpected(format!("failed to serialize payload: {e}")))?;
            form = form.part(
                "payload_json",
                Part::text(json_payload)
                    .mime_str("application/json")
                    .map_err(|e| AuthError::unexpected(format!("failed to set mime type: {e}")))?,
            );

            for (index, path) in request.attachments.iter().enumerate() {
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let content = tokio::fs::read(path).await.map_err(|e| {
                    AuthError::unexpected(format!(
                        "failed to read attachment {}: {}",
                        path.display(),
                        e
                    ))
                })?;

                let part = Part::bytes(content).file_name(filename);
                form = form.part(format!("files[{index}]"), part);
            }

            request_builder = request_builder.multipart(form);
        }

        let response = request_builder.send().await.map_err(|e| {
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

    async fn edit_message(
        &self,
        token: &AuthToken,
        request: EditMessageRequest,
    ) -> Result<Message, AuthError> {
        let url = format!(
            "{}/channels/{}/messages/{}",
            self.base_url,
            request.channel_id.as_u64(),
            request.message_id.as_u64()
        );

        debug!(
            channel_id = %request.channel_id,
            message_id = %request.message_id,
            "Editing message in Discord API"
        );

        let payload = EditMessagePayload {
            content: request.content,
        };

        let response = self
            .build_request(Method::PATCH, &url)
            .header(header::AUTHORIZATION, token.as_str())
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to edit message");
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

        debug!(message_id = %message_response.id, "Message edited successfully");

        Self::parse_message_response(message_response, request.channel_id.as_u64())
            .ok_or_else(|| AuthError::unexpected("failed to parse edited message"))
    }

    async fn delete_message(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<(), AuthError> {
        let url = format!(
            "{}/channels/{}/messages/{}",
            self.base_url,
            channel_id.as_u64(),
            message_id.as_u64()
        );

        debug!(
            channel_id = %channel_id,
            message_id = %message_id,
            "Deleting message via Discord API"
        );

        let response = self
            .build_request(Method::DELETE, &url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to delete message");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            return Err(self.handle_error_response(status, response).await);
        }

        debug!(message_id = %message_id, "Message deleted successfully");

        Ok(())
    }

    async fn send_typing_indicator(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
    ) -> Result<(), AuthError> {
        let url = format!("{}/channels/{}/typing", self.base_url, channel_id.as_u64());

        debug!(channel_id = %channel_id, "Sending typing indicator");

        let response = self
            .build_request(Method::POST, &url)
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

    async fn acknowledge_message(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        message_id: crate::domain::entities::MessageId,
    ) -> Result<(), AuthError> {
        let url = format!(
            "{}/channels/{}/messages/{}/ack",
            self.base_url,
            channel_id.as_u64(),
            message_id.as_u64()
        );

        debug!(
            channel_id = %channel_id,
            message_id = %message_id,
            "Acknowledging message"
        );

        let payload = serde_json::json!({ "token": null });

        let response = self
            .build_request(Method::POST, &url)
            .header(header::AUTHORIZATION, token.as_str())
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to acknowledge message");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();

        if !status.is_success() && status != StatusCode::NO_CONTENT {
            warn!(status = %status, "Failed to ACK message in API");
        }

        Ok(())
    }

    async fn fetch_forum_threads(
        &self,
        token: &AuthToken,
        channel_id: ChannelId,
        _guild_id: Option<GuildId>,
        offset: u32,
        limit: Option<u8>,
    ) -> Result<Vec<ForumThread>, AuthError> {
        let effective_limit = limit.unwrap_or(25).min(25);

        let url = format!(
            "{}/channels/{}/threads/search?archived=false&sort_by=last_message_time&sort_order=desc&limit={}&tag_setting=match_some&offset={}",
            self.base_url,
            channel_id.as_u64(),
            effective_limit,
            offset
        );

        debug!("Fetching forum threads from URL: {}", url);

        let response = self
            .build_request(Method::GET, &url)
            .header(header::AUTHORIZATION, token.as_str())
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to fetch forum threads");
                AuthError::network(e.to_string())
            })?;

        let status = response.status();
        if !status.is_success() {
             let error_text = response.text().await.unwrap_or_default();
             warn!("Fetch forum threads failed: Status={} Body={}", status, error_text);
             return Err(AuthError::unexpected(format!("Fetch threads failed: {status} - {error_text}")));
        }

        let threads_response: super::dto::ThreadsResponse = serde_json::from_str(&response.text().await.unwrap_or_default()).map_err(|e| {
            warn!(error = %e, "Failed to parse threads response");
            AuthError::unexpected(format!("failed to parse threads: {e}"))
        })?;
        
        let threads = Self::process_threads_response(threads_response, None);
        Ok(threads)
    }
}

impl DiscordClient {
    fn process_threads_response(response: super::dto::ThreadsResponse, parent_id: Option<ChannelId>) -> Vec<ForumThread> {
        let threads_to_process: Vec<_> = response.threads
            .into_iter()
            .filter(|t| {
                if let Some(target_parent) = parent_id {
                    t.parent_id.as_deref().and_then(|p| p.parse::<u64>().ok()) == Some(target_parent.as_u64())
                } else {
                    true
                }
            })
            .collect();

        let mut starter_messages = std::collections::HashMap::new();
        if let Some(msgs) = response.first_messages {
            for msg_dto in msgs {
                if let Ok(thread_id) = msg_dto.channel_id.parse::<u64>()
                     && let Some(message) = Self::parse_message_response(msg_dto, thread_id) {
                         starter_messages.insert(ChannelId(thread_id), message);
                     }
            }
        }

        threads_to_process
            .into_iter()
            .filter_map(|thread_dto| {
                let thread_id = thread_dto.id.parse::<u64>().ok()?;
                
                let starter_message = starter_messages.remove(&ChannelId(thread_id));
                
                let reaction_count = starter_message.as_ref().map_or(0, |m| {
                    m.reactions().iter().map(|r| r.count).sum()
                });

                Some(ForumThread {
                    id: ChannelId(thread_id),
                    guild_id: thread_dto
                        .guild_id
                        .and_then(|id| id.parse::<u64>().ok())
                        .map(GuildId),
                    parent_id: thread_dto
                        .parent_id
                        .and_then(|id| id.parse::<u64>().ok())
                        .map(ChannelId),
                    name: thread_dto.name.unwrap_or_default(),
                    author_id: thread_dto.owner_id.unwrap_or_default(),
                    message_count: thread_dto.message_count.unwrap_or(0),
                    member_count: thread_dto.member_count.unwrap_or(0),
                    last_activity_at: thread_dto
                        .thread_metadata
                        .as_ref()
                        .map(|m| m.archive_timestamp.clone()),
                    applied_tags: thread_dto
                        .applied_tags
                        .iter()
                        .filter_map(|t| t.parse::<u64>().ok())
                        .collect(),
                    starter_message,
                    last_message_id: thread_dto
                        .last_message_id
                        .and_then(|id| id.parse::<u64>().ok())
                        .map(crate::domain::entities::MessageId::from),
                    new: false,
                    reaction_count,
                })
            })
            .collect()
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

    #[tokio::test]
    async fn test_client_creation() {
        let client = DiscordClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_guild_response_parsing() {
        let json = r#"[
            {"id": "1234567890123456789", "name": "Nix/NixOS (unofficial)", "icon": "abc123", "owner": true, "permissions": "36953089", "features": ["COMMUNITY"]},
            {"id": "2234567890123456789", "name": "r/Unixporn", "icon": null, "owner": false, "permissions": "36953089", "features": []},
            {"id": "3234567890123456789", "name": "RespriteApp", "icon": "def456", "owner": false, "permissions": "36953089", "features": ["ANIMATED_ICON"]},
            {"id": "4234567890123456789", "name": "Oxicord", "icon": null, "owner": true, "permissions": "36953089", "features": []},
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
        assert_eq!(guilds[3].name(), "Oxicord");
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

    #[test]
    fn test_message_response_parsing_with_reactions() {
        use super::MessageResponse;
        
        let json = r#"{
            "id": "123456789",
            "channel_id": "987654321",
            "author": {
                "id": "111222",
                "username": "User",
                "discriminator": "0000",
                "avatar": null,
                "bot": false
            },
            "content": "Hello",
            "timestamp": "2023-01-01T12:00:00Z",
            "type": 0,
            "reactions": [
                {
                    "count": 5,
                    "me": true,
                    "emoji": { "id": null, "name": "👍" }
                },
                {
                    "count": 2,
                    "me": false,
                    "emoji": { "id": "999", "name": "custom" }
                }
            ]
        }"#;
        
        let response: MessageResponse = serde_json::from_str(json).expect("Should parse message JSON");
        let message = DiscordClient::parse_message_response(response, 987654321).expect("Should convert to Message");
        
        assert_eq!(message.reactions().len(), 2);
        
        let r1 = &message.reactions()[0];
        assert_eq!(r1.count, 5);
        assert!(r1.me);
        assert_eq!(r1.emoji.name.as_deref(), Some("👍"));
        
        let r2 = &message.reactions()[1];
        assert_eq!(r2.count, 2);
        assert!(!r2.me);
        assert_eq!(r2.emoji.id.as_deref(), Some("999"));
        assert_eq!(r2.emoji.name.as_deref(), Some("custom"));
    }
}
