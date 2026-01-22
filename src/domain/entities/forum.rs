use serde::{Deserialize, Serialize};

use super::{ChannelId, GuildId, Message};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumThread {
    pub id: ChannelId,
    pub guild_id: Option<GuildId>,
    pub parent_id: Option<ChannelId>,
    pub name: String,
    pub author_id: String,
    pub message_count: u32,
    pub member_count: u32,
    pub last_activity_at: Option<String>,
    pub applied_tags: Vec<u64>,
    pub starter_message: Option<Message>,
    pub last_message_id: Option<super::MessageId>,
    pub new: bool,
    pub reaction_count: u32,
}

impl ForumThread {
    pub fn new(
        id: impl Into<ChannelId>,
        name: impl Into<String>,
        author_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            guild_id: None,
            parent_id: None,
            name: name.into(),
            author_id: author_id.into(),
            message_count: 0,
            member_count: 0,
            last_activity_at: None,
            applied_tags: Vec::new(),
            starter_message: None,
            last_message_id: None,
            new: false,
            reaction_count: 0,
        }
    }
}
