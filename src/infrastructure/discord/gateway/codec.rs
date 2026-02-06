use chrono::{DateTime, Local, Utc};
use flate2::{Decompress, FlushDecompress, Status};

use super::constants::ZLIB_SUFFIX;
use super::error::{GatewayError, GatewayResult};
use super::events::{
    Activity, ActivityKind, DispatchEvent, PresenceStatus, ReactionEmoji, UnavailableGuild,
};
use super::payloads::{
    ActivityPayload, ChannelPayload, GatewayMessage, GuildCreatePayload, GuildDeletePayload,
    HelloPayload, MessageDeleteBulkPayload, MessageDeletePayload, MessagePayload,
    PresenceUpdatePayload, ReactionPayload, ReactionRemoveAllPayload, ReadyPayload,
    TypingStartPayload, UserUpdatePayload,
};

use crate::domain::entities::{
    Attachment, ChannelId, GuildId, Member, Message, MessageAuthor, MessageId, MessageKind,
    MessageReference, Permissions, Role, RoleId, User,
};

const INITIAL_BUFFER_SIZE: usize = 32 * 1024;
const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

pub struct GatewayCodec {
    inflater: Decompress,
    compressed_buffer: Vec<u8>,
    decompressed_buffer: Vec<u8>,
}

impl GatewayCodec {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inflater: Decompress::new(true),
            compressed_buffer: Vec::with_capacity(4096),
            decompressed_buffer: Vec::with_capacity(INITIAL_BUFFER_SIZE),
        }
    }

    pub fn decode_binary(&mut self, data: &[u8]) -> GatewayResult<Option<String>> {
        self.compressed_buffer.extend_from_slice(data);

        if !self.is_message_complete() {
            return Ok(None);
        }

        let result = self.decompress()?;
        self.compressed_buffer.clear();
        Ok(Some(result))
    }

    fn is_message_complete(&self) -> bool {
        self.compressed_buffer.len() >= 4
            && self.compressed_buffer[self.compressed_buffer.len() - 4..] == ZLIB_SUFFIX
    }

    fn decompress(&mut self) -> GatewayResult<String> {
        self.decompressed_buffer.clear();

        if self.decompressed_buffer.capacity() < INITIAL_BUFFER_SIZE {
            self.decompressed_buffer.reserve(INITIAL_BUFFER_SIZE);
        }

        let mut total_in = 0;
        let mut total_out = 0;

        loop {
            if self.decompressed_buffer.len() == self.decompressed_buffer.capacity() {
                let new_capacity = self
                    .decompressed_buffer
                    .capacity()
                    .saturating_mul(2)
                    .min(MAX_BUFFER_SIZE);

                if new_capacity == self.decompressed_buffer.capacity() {
                    return Err(GatewayError::compression(
                        "decompressed data exceeds maximum size".to_string(),
                    ));
                }

                self.decompressed_buffer.reserve(new_capacity);
            }

            let spare_capacity =
                self.decompressed_buffer.capacity() - self.decompressed_buffer.len();
            self.decompressed_buffer
                .resize(self.decompressed_buffer.len() + spare_capacity, 0);

            let in_before = self.inflater.total_in();
            let out_before = self.inflater.total_out();

            let status = self
                .inflater
                .decompress(
                    &self.compressed_buffer[total_in..],
                    &mut self.decompressed_buffer[total_out..],
                    FlushDecompress::Sync,
                )
                .map_err(|e| GatewayError::compression(e.to_string()))?;

            let consumed = usize::try_from(self.inflater.total_in() - in_before).unwrap_or(0);
            let produced = usize::try_from(self.inflater.total_out() - out_before).unwrap_or(0);

            total_in += consumed;
            total_out += produced;

            self.decompressed_buffer.truncate(total_out);

            if total_out == self.decompressed_buffer.capacity() {
                continue;
            }

            match status {
                Status::Ok | Status::BufError => {
                    if total_in >= self.compressed_buffer.len() {
                        break;
                    }
                }
                Status::StreamEnd => {
                    break;
                }
            }
        }

        String::from_utf8(self.decompressed_buffer[..total_out].to_vec())
            .map_err(|e| GatewayError::compression(format!("invalid UTF-8: {e}")))
    }

    pub fn reset(&mut self) {
        self.inflater.reset(true);
        self.compressed_buffer.clear();
        self.decompressed_buffer.clear();
    }
}

impl Default for GatewayCodec {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventParser;

impl EventParser {
    pub fn parse_message(json: &str) -> GatewayResult<GatewayMessage> {
        serde_json::from_str(json).map_err(|e| GatewayError::serialization(e.to_string()))
    }

    pub fn parse_hello(data: &serde_json::Value) -> GatewayResult<HelloPayload> {
        serde_json::from_value(data.clone())
            .map_err(|e| GatewayError::serialization(format!("Failed to parse Hello: {e}")))
    }

    pub fn parse_dispatch(
        event_type: &str,
        data: Option<serde_json::Value>,
    ) -> GatewayResult<DispatchEvent> {
        let data = data.ok_or_else(|| GatewayError::protocol("Missing dispatch data"))?;

        match event_type {
            "READY" => Self::parse_ready(data),
            "MESSAGE_CREATE" => Self::parse_message_create(data),
            "MESSAGE_UPDATE" => Self::parse_message_update(data),
            "MESSAGE_DELETE" => Self::parse_message_delete(data),
            "MESSAGE_DELETE_BULK" => Self::parse_message_delete_bulk(data),
            "MESSAGE_REACTION_ADD" => Self::parse_reaction_add(data),
            "MESSAGE_REACTION_REMOVE" => Self::parse_reaction_remove(data),
            "MESSAGE_REACTION_REMOVE_ALL" => Self::parse_reaction_remove_all(data),
            "TYPING_START" => Self::parse_typing_start(data),
            "PRESENCE_UPDATE" => Self::parse_presence_update(data),
            "CHANNEL_CREATE" => Self::parse_channel_create(data),
            "CHANNEL_UPDATE" => Self::parse_channel_update(data),
            "CHANNEL_DELETE" => Self::parse_channel_delete(data),
            "GUILD_CREATE" => Self::parse_guild_create(data),
            "GUILD_UPDATE" => Self::parse_guild_update(data),
            "GUILD_DELETE" => Self::parse_guild_delete(data),
            "USER_UPDATE" => Self::parse_user_update(data),
            "USER_SETTINGS_UPDATE" => Self::parse_user_settings_update(data),
            "VOICE_STATE_UPDATE" => Self::parse_voice_state_update(data),
            "VOICE_SERVER_UPDATE" => Self::parse_voice_server_update(data),
            "RELATIONSHIP_ADD" | "RELATIONSHIP_UPDATE" => Self::parse_relationship_add(data),
            "RELATIONSHIP_REMOVE" => Self::parse_relationship_remove(data),
            _ => {
                tracing::debug!(event_type = event_type, "Unhandled dispatch event type");
                Ok(DispatchEvent::Unknown {
                    event_type: event_type.to_string(),
                })
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_ready(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let ready: ReadyPayload = serde_json::from_value(data)
            .map_err(|e| GatewayError::serialization(format!("Failed to parse Ready: {e}")))?;

        let mut initial_guild_channels = std::collections::HashMap::new();
        let mut initial_guild_roles = std::collections::HashMap::new();
        let mut initial_guild_members = std::collections::HashMap::new();

        for g in &ready.guilds {
            if let Ok(guild_id) = g.id.parse::<u64>()
                && (!g.channels.is_empty() || !g.threads.is_empty())
            {
                let mut channels = Vec::new();
                for channel_payload in &g.channels {
                    if let Ok(id) = channel_payload.id.parse::<u64>() {
                        let kind = crate::domain::entities::ChannelKind::from(channel_payload.kind);
                        let name = channel_payload.name.clone().unwrap_or_default();

                        let mut channel =
                            crate::domain::entities::Channel::new(ChannelId(id), name, kind)
                                .with_guild(guild_id)
                                .with_position(channel_payload.position);

                        if let Some(parent_id) = &channel_payload.parent_id
                            && let Ok(pid) = parent_id.parse::<u64>()
                        {
                            channel = channel.with_parent(pid);
                        }

                        if let Some(topic) = &channel_payload.topic {
                            channel = channel.with_topic(topic.clone());
                        }

                        if let Some(last_message_id) = &channel_payload.last_message_id
                            && let Ok(lmid) = last_message_id.parse::<u64>()
                        {
                            channel = channel.with_last_message_id(Some(lmid.into()));
                        }

                        let overwrites = channel_payload
                            .permission_overwrites
                            .iter()
                            .map(|o| crate::domain::entities::PermissionOverwrite {
                                id: o.id.clone(),
                                overwrite_type: crate::domain::entities::OverwriteType::from(
                                    o.overwrite_type,
                                ),
                                allow: o.allow.clone(),
                                deny: o.deny.clone(),
                            })
                            .collect();
                        channel = channel.with_permission_overwrites(overwrites);

                        channels.push(channel);
                    }
                }

                for thread_payload in &g.threads {
                    if let Ok(id) = thread_payload.id.parse::<u64>() {
                        let kind = crate::domain::entities::ChannelKind::from(thread_payload.kind);
                        let name = thread_payload.name.clone().unwrap_or_default();

                        let mut thread =
                            crate::domain::entities::Channel::new(ChannelId(id), name, kind)
                                .with_guild(guild_id);

                        if let Some(parent_id) = &thread_payload.parent_id
                            && let Ok(pid) = parent_id.parse::<u64>()
                        {
                            thread = thread.with_parent(pid);
                        }

                        if let Some(last_message_id) = &thread_payload.last_message_id
                            && let Ok(lmid) = last_message_id.parse::<u64>()
                        {
                            thread = thread.with_last_message_id(Some(lmid.into()));
                        }

                        channels.push(thread);
                    }
                }

                initial_guild_channels.insert(GuildId(guild_id), channels);

                let roles: Vec<Role> = g
                    .roles
                    .iter()
                    .filter_map(|r| {
                        let id = r.id.parse::<u64>().ok()?;
                        let permissions = r.permissions.parse::<u64>().ok()?;
                        Some(Role {
                            id: RoleId(id),
                            name: r.name.clone(),
                            color: r.color,
                            hoist: r.hoist,
                            icon: r.icon.clone(),
                            unicode_emoji: r.unicode_emoji.clone(),
                            position: r.position,
                            permissions: Permissions::from_bits_truncate(permissions),
                            managed: r.managed,
                            mentionable: r.mentionable,
                        })
                    })
                    .collect();
                initial_guild_roles.insert(GuildId(guild_id), roles);

                let members: Vec<Member> = g
                    .members
                    .iter()
                    .map(|m| Member {
                        user: m.user.as_ref().map(|u| {
                            User::new(
                                u.id.clone(),
                                u.username.clone(),
                                u.discriminator.clone(),
                                u.avatar.clone(),
                                u.bot,
                                None,
                            )
                            .with_global_name(u.global_name.clone().unwrap_or_default())
                        }),
                        nick: m.nick.clone(),
                        avatar: m.avatar.clone(),
                        roles: m
                            .roles
                            .iter()
                            .filter_map(|r| r.parse::<u64>().ok().map(RoleId))
                            .collect(),
                        joined_at: m.joined_at.clone(),
                        premium_since: m.premium_since.clone(),
                        deaf: m.deaf,
                        mute: m.mute,
                        pending: m.pending,
                        permissions: m.permissions.clone(),
                        communication_disabled_until: m.communication_disabled_until.clone(),
                    })
                    .collect();
                initial_guild_members.insert(GuildId(guild_id), members);
            }
        }

        let guilds = ready
            .guilds
            .iter()
            .filter_map(|g| {
                g.id.parse::<u64>().ok().map(|id| UnavailableGuild {
                    id: GuildId(id),
                    unavailable: g.unavailable,
                })
            })
            .collect();

        let read_states = ready
            .read_state
            .into_iter()
            .filter_map(|rs| {
                let channel_id = rs.id.parse::<u64>().ok()?;
                let last_message_id = rs
                    .last_message_id
                    .and_then(|id| id.parse::<u64>().ok())
                    .map(Into::into);

                Some(
                    crate::domain::entities::ReadState::new(ChannelId(channel_id), last_message_id)
                        .with_mention_count(rs.mention_count),
                )
            })
            .collect();

        let guild_folders = ready
            .user_settings
            .map(|s| s.guild_folders)
            .unwrap_or_default()
            .into_iter()
            .map(|f| crate::domain::entities::GuildFolder {
                id: f.id,
                name: f.name,
                color: f.color,
                guild_ids: f
                    .guild_ids
                    .into_iter()
                    .filter_map(|id| id.parse::<u64>().ok().map(GuildId))
                    .collect(),
            })
            .collect();

        let relationships = ready
            .relationships
            .into_iter()
            .filter_map(|rel| {
                rel.id.parse::<u64>().ok().map(|id| {
                    crate::domain::entities::Relationship::new(
                        crate::domain::entities::UserId(id),
                        crate::domain::entities::RelationshipType::from(rel.relationship_type),
                    )
                })
            })
            .collect();

        Ok(DispatchEvent::Ready {
            session_id: ready.session_id,
            resume_gateway_url: ready.resume_gateway_url,
            user_id: ready.user.id,
            guilds,
            initial_guild_channels,
            initial_guild_roles,
            initial_guild_members,
            read_states,
            guild_folders,
            relationships,
        })
    }

    fn parse_message_create(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: MessagePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse MessageCreate: {e}"))
        })?;

        let message = Self::convert_message_payload(payload)?;
        Ok(DispatchEvent::MessageCreate { message })
    }

    fn parse_message_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: MessagePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse MessageUpdate: {e}"))
        })?;

        let message = Self::convert_message_payload(payload)?;
        Ok(DispatchEvent::MessageUpdate { message })
    }

    fn parse_message_delete(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: MessageDeletePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse MessageDelete: {e}"))
        })?;

        let message_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid message ID"))?;

        let channel_id = payload
            .channel_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::MessageDelete {
            message_id: MessageId(message_id),
            channel_id: ChannelId(channel_id),
            guild_id,
        })
    }

    fn parse_message_delete_bulk(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: MessageDeleteBulkPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse MessageDeleteBulk: {e}"))
        })?;

        let message_ids: Vec<MessageId> = payload
            .ids
            .iter()
            .filter_map(|id| id.parse::<u64>().ok())
            .map(MessageId)
            .collect();

        let channel_id = payload
            .channel_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::MessageDeleteBulk {
            message_ids,
            channel_id: ChannelId(channel_id),
            guild_id,
        })
    }

    fn parse_reaction_add(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ReactionPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ReactionAdd: {e}"))
        })?;

        let channel_id = payload
            .channel_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let message_id = payload
            .message_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid message ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::MessageReactionAdd {
            user_id: payload.user_id,
            channel_id: ChannelId(channel_id),
            message_id: MessageId(message_id),
            guild_id,
            emoji: ReactionEmoji {
                id: payload.emoji.id,
                name: payload.emoji.name,
                animated: payload.emoji.animated,
            },
        })
    }

    fn parse_reaction_remove(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ReactionPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ReactionRemove: {e}"))
        })?;

        let channel_id = payload
            .channel_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let message_id = payload
            .message_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid message ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::MessageReactionRemove {
            user_id: payload.user_id,
            channel_id: ChannelId(channel_id),
            message_id: MessageId(message_id),
            guild_id,
            emoji: ReactionEmoji {
                id: payload.emoji.id,
                name: payload.emoji.name,
                animated: payload.emoji.animated,
            },
        })
    }

    fn parse_reaction_remove_all(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ReactionRemoveAllPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ReactionRemoveAll: {e}"))
        })?;

        let channel_id = payload
            .channel
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let message_id = payload
            .message
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid message ID"))?;

        let guild_id = payload
            .guild
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::MessageReactionRemoveAll {
            channel_id: ChannelId(channel_id),
            message_id: MessageId(message_id),
            guild_id,
        })
    }

    fn parse_typing_start(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: TypingStartPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse TypingStart: {e}"))
        })?;

        let channel_id = payload
            .channel_id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        let username = payload.member.and_then(|m| {
            m.nick.or_else(|| {
                m.user.map(|u| {
                    if let Some(global) = u.global_name {
                        global
                    } else if u.discriminator == "0" {
                        u.username
                    } else {
                        format!("{}#{}", u.username, u.discriminator)
                    }
                })
            })
        });

        let timestamp = DateTime::from_timestamp(payload.timestamp, 0).unwrap_or_else(Utc::now);

        Ok(DispatchEvent::TypingStart {
            channel_id: ChannelId(channel_id),
            guild_id,
            user_id: payload.user_id,
            username,
            timestamp,
        })
    }

    fn parse_presence_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: PresenceUpdatePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse PresenceUpdate: {e}"))
        })?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        let activities: Vec<Activity> = payload
            .activities
            .into_iter()
            .map(Self::convert_activity)
            .collect();

        Ok(DispatchEvent::PresenceUpdate {
            user_id: payload.user.id,
            guild_id,
            status: PresenceStatus::parse(&payload.status),
            activities,
        })
    }

    fn parse_channel_create(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ChannelPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ChannelCreate: {e}"))
        })?;

        let channel_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::ChannelCreate {
            channel_id: ChannelId(channel_id),
            guild_id,
            name: payload.name.unwrap_or_default(),
            kind: payload.kind,
        })
    }

    fn parse_channel_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ChannelPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ChannelUpdate: {e}"))
        })?;

        let channel_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::ChannelUpdate {
            channel_id: ChannelId(channel_id),
            guild_id,
            name: payload.name.unwrap_or_default(),
            kind: payload.kind,
        })
    }

    fn parse_channel_delete(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: ChannelPayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse ChannelDelete: {e}"))
        })?;

        let channel_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);

        Ok(DispatchEvent::ChannelDelete {
            channel_id: ChannelId(channel_id),
            guild_id,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn parse_guild_create(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: GuildCreatePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse GuildCreate: {e}"))
        })?;

        let guild_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid guild ID"))?;

        let mut channels = Vec::new();
        for channel_payload in payload.channels {
            let id = channel_payload
                .id
                .parse::<u64>()
                .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

            let kind = crate::domain::entities::ChannelKind::from(channel_payload.kind);
            let name = channel_payload.name.unwrap_or_default();

            let mut channel = crate::domain::entities::Channel::new(id, name, kind)
                .with_guild(guild_id)
                .with_position(channel_payload.position);

            if let Some(parent_id) = channel_payload.parent_id
                && let Ok(pid) = parent_id.parse::<u64>()
            {
                channel = channel.with_parent(pid);
            }

            if let Some(topic) = channel_payload.topic {
                channel = channel.with_topic(topic);
            }

            if let Some(last_message_id) = channel_payload.last_message_id
                && let Ok(lmid) = last_message_id.parse::<u64>()
            {
                channel = channel.with_last_message_id(Some(lmid.into()));
            }

            let overwrites = channel_payload
                .permission_overwrites
                .iter()
                .map(|o| crate::domain::entities::PermissionOverwrite {
                    id: o.id.clone(),
                    overwrite_type: crate::domain::entities::OverwriteType::from(o.overwrite_type),
                    allow: o.allow.clone(),
                    deny: o.deny.clone(),
                })
                .collect();
            channel = channel.with_permission_overwrites(overwrites);

            channels.push(channel);
        }

        let mut threads = Vec::new();
        for thread_payload in payload.threads {
            let id = thread_payload
                .id
                .parse::<u64>()
                .map_err(|_| GatewayError::protocol("Invalid thread ID"))?;

            let kind = crate::domain::entities::ChannelKind::from(thread_payload.kind);
            let name = thread_payload.name.unwrap_or_default();

            let mut thread =
                crate::domain::entities::Channel::new(id, name, kind).with_guild(guild_id);

            if let Some(parent_id) = thread_payload.parent_id
                && let Ok(pid) = parent_id.parse::<u64>()
            {
                thread = thread.with_parent(pid);
            }

            if let Some(last_message_id) = thread_payload.last_message_id
                && let Ok(lmid) = last_message_id.parse::<u64>()
            {
                thread = thread.with_last_message_id(Some(lmid.into()));
            }

            threads.push(thread);
        }

        let roles = payload
            .roles
            .into_iter()
            .filter_map(|r| {
                let id = r.id.parse::<u64>().ok()?;
                let permissions = r.permissions.parse::<u64>().ok()?;
                Some(Role {
                    id: RoleId(id),
                    name: r.name,
                    color: r.color,
                    hoist: r.hoist,
                    icon: r.icon,
                    unicode_emoji: r.unicode_emoji,
                    position: r.position,
                    permissions: Permissions::from_bits_truncate(permissions),
                    managed: r.managed,
                    mentionable: r.mentionable,
                })
            })
            .collect();

        let members = payload
            .members
            .into_iter()
            .map(|m| Member {
                user: m.user.map(|u| {
                    User::new(u.id, u.username, u.discriminator, u.avatar, u.bot, None)
                        .with_global_name(u.global_name.unwrap_or_default())
                }),
                nick: m.nick,
                avatar: m.avatar,
                roles: m
                    .roles
                    .into_iter()
                    .filter_map(|r| r.parse::<u64>().ok().map(RoleId))
                    .collect(),
                joined_at: m.joined_at,
                premium_since: m.premium_since,
                deaf: m.deaf,
                mute: m.mute,
                pending: m.pending,
                permissions: m.permissions,
                communication_disabled_until: m.communication_disabled_until,
            })
            .collect();

        Ok(DispatchEvent::GuildCreate {
            guild_id: GuildId(guild_id),
            name: payload.name,
            unavailable: payload.unavailable,
            channels,
            threads,
            roles,
            members,
        })
    }

    fn parse_guild_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: GuildCreatePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse GuildUpdate: {e}"))
        })?;

        let guild_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid guild ID"))?;

        Ok(DispatchEvent::GuildUpdate {
            guild_id: GuildId(guild_id),
            name: payload.name,
        })
    }

    fn parse_guild_delete(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: GuildDeletePayload = serde_json::from_value(data).map_err(|e| {
            GatewayError::serialization(format!("Failed to parse GuildDelete: {e}"))
        })?;

        let guild_id = payload
            .id
            .parse::<u64>()
            .map_err(|_| GatewayError::protocol("Invalid guild ID"))?;

        Ok(DispatchEvent::GuildDelete {
            guild_id: GuildId(guild_id),
            unavailable: payload.unavailable,
        })
    }

    fn parse_user_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: UserUpdatePayload = serde_json::from_value(data)
            .map_err(|e| GatewayError::serialization(format!("Failed to parse UserUpdate: {e}")))?;

        Ok(DispatchEvent::UserUpdate {
            user_id: payload.id,
            username: payload.username,
            discriminator: payload.discriminator,
            avatar: payload.avatar,
        })
    }

    fn parse_user_settings_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: super::payloads::UserSettingsPayload =
            serde_json::from_value(data).map_err(|e| {
                GatewayError::serialization(format!("Failed to parse UserSettingsUpdate: {e}"))
            })?;

        let guild_folders = payload
            .guild_folders
            .into_iter()
            .map(|f| crate::domain::entities::GuildFolder {
                id: f.id,
                name: f.name,
                color: f.color,
                guild_ids: f
                    .guild_ids
                    .into_iter()
                    .filter_map(|id| id.parse::<u64>().ok().map(GuildId))
                    .collect(),
            })
            .collect();

        Ok(DispatchEvent::UserSettingsUpdate { guild_folders })
    }

    fn parse_voice_state_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: super::payloads::VoiceStateUpdatePayload = serde_json::from_value(data)
            .map_err(|e| {
                GatewayError::serialization(format!("Failed to parse VoiceStateUpdate: {e}"))
            })?;

        let guild_id = payload
            .guild_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId);
        let channel_id = payload
            .channel_id
            .and_then(|id| id.parse::<u64>().ok())
            .map(ChannelId);

        Ok(DispatchEvent::VoiceStateUpdate {
            guild_id,
            channel_id,
            user_id: payload.user_id,
            session_id: payload.session_id,
            deaf: payload.deaf,
            mute: payload.mute,
            self_deaf: payload.self_deaf,
            self_mute: payload.self_mute,
            self_video: payload.self_video,
            suppress: payload.suppress,
        })
    }

    fn parse_voice_server_update(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: super::payloads::VoiceServerUpdatePayload = serde_json::from_value(data)
            .map_err(|e| {
                GatewayError::serialization(format!("Failed to parse VoiceServerUpdate: {e}"))
            })?;

        let guild_id = payload
            .guild_id
            .parse::<u64>()
            .map(GuildId)
            .map_err(|_| GatewayError::protocol("Invalid guild ID"))?;

        Ok(DispatchEvent::VoiceServerUpdate {
            token: payload.token,
            guild_id,
            endpoint: payload.endpoint,
        })
    }

    fn parse_relationship_add(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        tracing::debug!(raw = %data, "Parsing RELATIONSHIP_ADD");
        let payload: super::payloads::RelationshipPayload =
            serde_json::from_value(data).map_err(|e| {
                GatewayError::serialization(format!("Failed to parse RelationshipAdd: {e}"))
            })?;

        let user_id = payload
            .id
            .parse::<u64>()
            .map(crate::domain::entities::UserId)
            .map_err(|_| GatewayError::protocol("Invalid user ID in relationship"))?;

        let relationship_type =
            crate::domain::entities::RelationshipType::from(payload.relationship_type);

        Ok(DispatchEvent::RelationshipAdd {
            user_id,
            relationship_type,
        })
    }

    fn parse_relationship_remove(data: serde_json::Value) -> GatewayResult<DispatchEvent> {
        let payload: super::payloads::RelationshipRemovePayload = serde_json::from_value(data)
            .map_err(|e| {
                GatewayError::serialization(format!("Failed to parse RelationshipRemove: {e}"))
            })?;

        let user_id = payload
            .id
            .parse::<u64>()
            .map(crate::domain::entities::UserId)
            .map_err(|_| GatewayError::protocol("Invalid user ID in relationship"))?;

        Ok(DispatchEvent::RelationshipRemove { user_id })
    }

    fn convert_message_payload(payload: MessagePayload) -> GatewayResult<Message> {
        let id: u64 = payload
            .id
            .parse()
            .map_err(|_| GatewayError::protocol("Invalid message ID"))?;

        let channel_id: u64 = payload
            .channel_id
            .parse()
            .map_err(|_| GatewayError::protocol("Invalid channel ID"))?;

        let timestamp: DateTime<Utc> = payload
            .timestamp
            .parse()
            .map_err(|_| GatewayError::protocol("Invalid timestamp"))?;

        let author = MessageAuthor {
            id: payload.author.id,
            username: payload.author.username,
            discriminator: payload.author.discriminator,
            avatar: payload.author.avatar,
            bot: payload.author.bot,
            global_name: payload.author.global_name,
        };

        let mut message = Message::new(
            MessageId(id),
            ChannelId(channel_id),
            author,
            payload.content,
            timestamp.with_timezone(&Local),
            MessageKind::from(payload.kind),
        )
        .with_pinned(payload.pinned);

        if !payload.attachments.is_empty() {
            let attachments: Vec<Attachment> = payload
                .attachments
                .into_iter()
                .map(|a| {
                    let mut att = Attachment::new(a.id, a.filename, a.size, a.url);
                    if let Some(ct) = a.content_type {
                        att = att.with_content_type(ct);
                    }
                    att
                })
                .collect();
            message = message.with_attachments(attachments);
        }

        if let Some(edited) = payload.edited_timestamp
            && let Ok(edited_ts) = edited.parse::<DateTime<Utc>>()
        {
            message = message.with_edited_timestamp(edited_ts.with_timezone(&Local));
        }

        if let Some(reference) = payload.message_reference {
            let ref_msg_id = reference.message_id.and_then(|id| id.parse::<u64>().ok());
            let ref_channel_id = reference.channel_id.and_then(|id| id.parse::<u64>().ok());

            message = message.with_reference(MessageReference::new(
                ref_msg_id.map(Into::into),
                ref_channel_id.map(Into::into),
                None,
            ));
        }

        if let Some(referenced) = payload.referenced_message
            && let Ok(ref_message) = Self::convert_message_payload(*referenced)
        {
            message = message.with_referenced_message(Some(ref_message));
        }

        if !payload.mentions.is_empty() {
            let mentions: Vec<User> = payload
                .mentions
                .into_iter()
                .map(|m| {
                    User::new(
                        m.id,
                        m.username,
                        m.discriminator,
                        m.avatar,
                        m.bot,
                        m.member.and_then(|mem| mem.color),
                    )
                })
                .collect();
            message = message.with_mentions(mentions);
        }

        Ok(message)
    }

    fn convert_activity(payload: ActivityPayload) -> Activity {
        Activity {
            name: payload.name,
            kind: ActivityKind::from_u8(payload.kind),
            details: payload.details,
            state: payload.state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compress;
    use flate2::Compression;
    use flate2::FlushCompress;

    #[test]
    fn test_codec_incomplete_message() {
        let mut codec = GatewayCodec::new();
        let result = codec.decode_binary(&[0x01, 0x02, 0x03]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_event_parser_unknown_event() {
        let data = serde_json::json!({});
        let result = EventParser::parse_dispatch("UNKNOWN_EVENT", Some(data)).unwrap();
        assert!(matches!(result, DispatchEvent::Unknown { .. }));
    }

    #[test]
    fn test_codec_reset() {
        let mut codec = GatewayCodec::new();
        let compressed = {
            let mut encoder = Compress::new(Compression::default(), true);
            let mut out = Vec::new();
            encoder
                .compress(b"test", &mut out, FlushCompress::Sync)
                .unwrap();
            out
        };
        codec.compressed_buffer.extend_from_slice(&compressed);
        codec.reset();
        assert!(codec.compressed_buffer.is_empty());
    }

    #[test]
    fn test_codec_large_message() {
        let mut codec = GatewayCodec::new();
        let large_string = "a".repeat(50 * 1024);

        let mut encoder = Compress::new(Compression::default(), true);
        let mut compressed = Vec::with_capacity(large_string.len());
        let mut out_buffer = vec![0u8; 4096];

        let mut total_in = 0;
        loop {
            let input = &large_string.as_bytes()[total_in..];
            let status = encoder
                .compress(
                    input,
                    &mut out_buffer,
                    if input.is_empty() {
                        FlushCompress::Sync
                    } else {
                        FlushCompress::None
                    },
                )
                .unwrap();

            let produced = encoder.total_out() as usize - compressed.len();
            compressed.extend_from_slice(&out_buffer[..produced]);

            let consumed = encoder.total_in() as usize - total_in;
            total_in += consumed;

            if status == Status::StreamEnd || (input.is_empty() && status == Status::Ok) {
                break;
            }
        }

        let suffix = &compressed[compressed.len() - 4..];
        assert_eq!(
            suffix, ZLIB_SUFFIX,
            "Compression did not end with ZLIB suffix"
        );

        let result = codec.decode_binary(&compressed).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), large_string);
    }

    #[test]
    fn test_parse_typing_start() {
        let data = serde_json::json!({
            "channel_id": "123456789",
            "guild_id": "987654321",
            "user_id": "111222333",
            "timestamp": 1_234_567_890,
            "member": {
                "user": {
                    "username": "TestUser",
                    "discriminator": "0"
                },
                "nick": null
            }
        });
        let result = EventParser::parse_dispatch("TYPING_START", Some(data)).unwrap();
        match result {
            DispatchEvent::TypingStart {
                channel_id,
                guild_id,
                user_id,
                username,
                ..
            } => {
                assert_eq!(channel_id, ChannelId(123_456_789));
                assert_eq!(guild_id, Some(GuildId(987_654_321)));
                assert_eq!(user_id, "111222333");
                assert_eq!(username, Some("TestUser".to_string()));
            }
            _ => panic!("Expected TypingStart event"),
        }
    }

    #[test]
    fn test_parse_typing_start_dm() {
        let data = serde_json::json!({
            "channel_id": "123456789",
            "user_id": "111222333",
            "timestamp": 1_234_567_890
        });
        let result = EventParser::parse_dispatch("TYPING_START", Some(data)).unwrap();
        match result {
            DispatchEvent::TypingStart {
                channel_id,
                guild_id,
                user_id,
                username,
                ..
            } => {
                assert_eq!(channel_id, ChannelId(123_456_789));
                assert!(guild_id.is_none());
                assert_eq!(user_id, "111222333");
                assert!(username.is_none());
            }
            _ => panic!("Expected TypingStart event"),
        }
    }

    #[test]
    fn test_parse_ready_with_integer_zero_fields() {
        let data = serde_json::json!({
            "v": 9,
            "user_settings": {},
            "user": {
                "verified": true,
                "username": "test",
                "mfa_enabled": false,
                "id": "12345",
                "flags": 0,
                "email": "test@example.com",
                "discriminator": "0000",
                "bot": false,
                "avatar": null
            },
            "session_id": "session123",
            "relationships": [],
            "read_state": [
                {
                    "mention_count": 0,
                    "last_message_id": 0,
                    "id": "123456"
                }
            ],
            "private_channels": [],
            "presences": [],
            "guilds": [],
            "guild_join_requests": [],
            "geo_ordered_rtc_regions": [],
            "friend_suggestion_count": 0,
            "experiments": [],
            "country_code": "US",
            "consents": {},
            "connected_accounts": [],
            "auth_session_id_hash": "hash",
            "application": {
                "id": "123",
                "flags": 0
            },
            "analytics_token": "token",
            "_trace": []
        });

        let result = EventParser::parse_ready(data);
        assert!(
            result.is_ok(),
            "Parsing should succeed even with integer 0 for last_message_id"
        );

        if let Ok(DispatchEvent::Ready { read_states, .. }) = result {
            assert_eq!(read_states.len(), 1);
            let rs = &read_states[0];
            assert!(rs.last_read_message_id.is_none());
        } else {
            panic!("Expected Ready event");
        }
    }
}
