use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::domain::entities::{AuthToken, ChannelId, GuildId, Message, MessageId};
use crate::domain::ports::{
    DirectMessageChannel, DiscordDataPort, EditMessageRequest, FetchMessagesOptions,
    SendMessageRequest,
};
use crate::infrastructure::image::ImageLoader;

#[derive(Debug)]
pub enum Action {
    HistoryLoaded(Vec<Message>),
    LoadError(String),
    DataLoaded {
        user: crate::domain::entities::User,
        guilds: Vec<crate::domain::entities::Guild>,
        dms: Vec<DirectMessageChannel>,
        read_states: std::collections::HashMap<ChannelId, crate::domain::entities::ReadState>,
    },
    GuildChannelsLoaded {
        guild_id: GuildId,
        channels: Vec<crate::domain::entities::Channel>,
    },
    GuildChannelsLoadError {
        guild_id: GuildId,
        error: String,
    },
    ChannelMessagesLoaded {
        channel_id: ChannelId,
        messages: Vec<Message>,
    },
    ChannelMessagesLoadError {
        channel_id: ChannelId,
        error: String,
    },
    ForumThreadsLoaded {
        channel_id: ChannelId,
        threads: Vec<crate::domain::entities::ForumThread>,
        offset: u32,
    },
    ForumThreadsLoadError {
        channel_id: ChannelId,
        error: String,
    },
    MessageSent(Message),
    MessageSendError(String),
    MessageEdited(Message),
    MessageEditError(String),
    MessageDeleted(MessageId),
    MessageDeleteError(String),
    TypingIndicatorSent(ChannelId),
    LoginSuccess {
        user: crate::domain::entities::User,
        token: String,
        source: crate::application::dto::TokenSource,
    },
    LoginFailure(crate::domain::errors::AuthError),
    /// Image loader has been initialized and is ready to use.
    ImageLoaderReady(Arc<ImageLoader>),
}

#[derive(Debug)]
pub enum BackendCommand {
    LoadGuildChannels {
        guild_id: GuildId,
        token: AuthToken,
    },
    LoadChannelMessages {
        channel_id: ChannelId,
        token: AuthToken,
    },
    LoadForumThreads {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        token: AuthToken,
        offset: u32,
    },
    LoadHistory {
        channel_id: ChannelId,
        before_message_id: MessageId,
        token: AuthToken,
    },
    SendMessage {
        token: AuthToken,
        request: SendMessageRequest,
    },
    EditMessage {
        token: AuthToken,
        request: EditMessageRequest,
    },
    DeleteMessage {
        token: AuthToken,
        channel_id: ChannelId,
        message_id: MessageId,
    },
    SendTypingIndicator {
        channel_id: ChannelId,
        token: AuthToken,
    },
    AcknowledgeMessage {
        channel_id: ChannelId,
        message_id: MessageId,
        token: AuthToken,
    },
    LoadInitialData {
        token: AuthToken,
        user: crate::domain::entities::User,
    },
}

pub struct Backend {
    discord_data: Arc<dyn DiscordDataPort>,
    command_rx: mpsc::UnboundedReceiver<BackendCommand>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl Backend {
    pub fn new(
        discord_data: Arc<dyn DiscordDataPort>,
        command_rx: mpsc::UnboundedReceiver<BackendCommand>,
        action_tx: mpsc::UnboundedSender<Action>,
    ) -> Self {
        Self {
            discord_data,
            command_rx,
            action_tx,
        }
    }

    pub async fn run(mut self) {
        info!("Backend worker started");
        while let Some(command) = self.command_rx.recv().await {
            self.handle_command(command).await;
        }
        info!("Backend worker stopped");
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_command(&self, command: BackendCommand) {
        match command {
            BackendCommand::LoadGuildChannels { guild_id, token } => {
                match self
                    .discord_data
                    .fetch_channels(&token, guild_id.as_u64())
                    .await
                {
                    Ok(channels) => {
                        debug!(guild_id = %guild_id, count = channels.len(), "Loaded channels for guild");
                        let _ = self
                            .action_tx
                            .send(Action::GuildChannelsLoaded { guild_id, channels });
                    }
                    Err(e) => {
                        warn!(guild_id = %guild_id, error = %e, "Failed to load channels for guild");
                        let _ = self.action_tx.send(Action::GuildChannelsLoadError {
                            guild_id,
                            error: e.to_string(),
                        });
                    }
                }
            }
            BackendCommand::LoadChannelMessages { channel_id, token } => {
                let options = FetchMessagesOptions::default().with_limit(50);
                match self
                    .discord_data
                    .fetch_messages(&token, channel_id.as_u64(), options)
                    .await
                {
                    Ok(messages) => {
                        debug!(channel_id = %channel_id, count = messages.len(), "Loaded messages for channel");
                        let _ = self.action_tx.send(Action::ChannelMessagesLoaded {
                            channel_id,
                            messages,
                        });
                    }
                    Err(e) => {
                        warn!(channel_id = %channel_id, error = %e, "Failed to load messages for channel");
                        let _ = self.action_tx.send(Action::ChannelMessagesLoadError {
                            channel_id,
                            error: e.to_string(),
                        });
                    }
                }
            }
            BackendCommand::LoadForumThreads { channel_id, guild_id, token, offset } => {
                match self.discord_data.fetch_forum_threads(&token, channel_id, guild_id, offset, Some(50)).await {
                    Ok(mut threads) => {
                        debug!(channel_id = %channel_id, count = threads.len(), "Loaded forum threads");
                        threads.reverse();
                        
                        let _ = self.action_tx.send(Action::ForumThreadsLoaded {
                            channel_id,
                            threads,
                            offset,
                        });
                    }
                    Err(e) => {
                        warn!(channel_id = %channel_id, error = %e, "Failed to load forum threads");
                        let _ = self.action_tx.send(Action::ForumThreadsLoadError {
                            channel_id,
                            error: e.to_string(),
                        });
                    }
                }
            }
            BackendCommand::LoadHistory {
                channel_id,
                before_message_id,
                token,
            } => {
                match self
                    .discord_data
                    .load_more_before_id(
                        &token,
                        channel_id.as_u64(),
                        before_message_id.as_u64(),
                        50,
                    )
                    .await
                {
                    Ok(messages) => {
                        debug!(count = messages.len(), "Loaded historical messages");
                        let _ = self.action_tx.send(Action::HistoryLoaded(messages));
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to load history");
                        let _ = self.action_tx.send(Action::LoadError(e.to_string()));
                    }
                }
            }
            BackendCommand::SendMessage { token, request } => {
                match self.discord_data.send_message(&token, request).await {
                    Ok(message) => {
                        info!(message_id = %message.id(), "Message sent successfully");
                        let _ = self.action_tx.send(Action::MessageSent(message));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to send message");
                        let _ = self.action_tx.send(Action::MessageSendError(e.to_string()));
                    }
                }
            }
            BackendCommand::EditMessage { token, request } => {
                match self.discord_data.edit_message(&token, request).await {
                    Ok(message) => {
                        info!(message_id = %message.id(), "Message edited successfully");
                        let _ = self.action_tx.send(Action::MessageEdited(message));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to edit message");
                        let _ = self.action_tx.send(Action::MessageEditError(e.to_string()));
                    }
                }
            }
            BackendCommand::DeleteMessage {
                token,
                channel_id,
                message_id,
            } => {
                match self
                    .discord_data
                    .delete_message(&token, channel_id, message_id)
                    .await
                {
                    Ok(()) => {
                        info!(message_id = %message_id, "Message deleted successfully");
                        let _ = self.action_tx.send(Action::MessageDeleted(message_id));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to delete message");
                        let _ = self.action_tx.send(Action::MessageDeleteError(e.to_string()));
                    }
                }
            }
            BackendCommand::SendTypingIndicator { channel_id, token } => {
                if let Err(e) = self
                    .discord_data
                    .send_typing_indicator(&token, channel_id)
                    .await
                {
                    debug!(error = %e, "Failed to send typing indicator");
                } else {
                    let _ = self.action_tx.send(Action::TypingIndicatorSent(channel_id));
                }
            }
            BackendCommand::AcknowledgeMessage {
                channel_id,
                message_id,
                token,
            } => {
                if let Err(e) = self
                    .discord_data
                    .acknowledge_message(&token, channel_id, message_id)
                    .await
                {
                    warn!(error = %e, "Failed to ack message");
                }
            }
            BackendCommand::LoadInitialData { token, user } => {
                let guilds_future = self.discord_data.fetch_guilds(&token);
                let dms_future = self.discord_data.fetch_dm_channels(&token);
                let read_states_future = self.discord_data.fetch_read_states(&token);

                let (guilds_result, dms_result, read_states_result) =
                    tokio::join!(guilds_future, dms_future, read_states_future);

                let guilds = match guilds_result {
                    Ok(g) => g,
                    Err(e) => {
                        error!(error = %e, "Failed to load initial guilds");
                        Vec::new()
                    }
                };

                let dms = match dms_result {
                    Ok(d) => d,
                    Err(e) => {
                        error!(error = %e, "Failed to load initial DMs");
                        Vec::new()
                    }
                };

                let read_states = match read_states_result {
                    Ok(rs) => rs.into_iter().map(|s| (s.channel_id, s)).collect(),
                    Err(e) => {
                        error!(error = %e, "Failed to load read states");
                        std::collections::HashMap::new()
                    }
                };

                let _ = self.action_tx.send(Action::DataLoaded {
                    user,
                    guilds,
                    dms,
                    read_states,
                });
            }
        }
    }
}
