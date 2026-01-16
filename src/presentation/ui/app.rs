//! Main application orchestrator.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEvent};
use futures_util::StreamExt;
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::application::dto::{LoginRequest, TokenSource};
use crate::application::services::markdown_service::MarkdownService;
use crate::application::use_cases::{LoginUseCase, ResolveTokenUseCase};
use crate::domain::entities::{AuthToken, ChannelId, GuildId, MessageId, User};
use crate::domain::errors::AuthError;
use crate::domain::ports::{
    AuthPort, DiscordDataPort, FetchMessagesOptions, SendMessageRequest, TokenStoragePort,
};
use crate::infrastructure::discord::{
    DispatchEvent, GatewayClient, GatewayClientConfig, GatewayEventKind, GatewayIntents,
    TypingIndicatorManager,
};
use crate::presentation::events::{EventHandler, EventResult};
use crate::presentation::ui::{ChatKeyResult, ChatScreen, ChatScreenState, LoginScreen};
use crate::presentation::widgets::ConnectionStatus;

const TYPING_CLEANUP_INTERVAL: Duration = Duration::from_secs(2);
const TYPING_THROTTLE_DURATION: Duration = Duration::from_secs(8);

#[derive(Debug)]
enum Action {
    HistoryLoaded(Vec<crate::domain::entities::Message>),
    LoadError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Login,
    Chat,
    Exiting,
}

enum CurrentScreen {
    Login(LoginScreen),
    Chat(Box<ChatScreenState>),
}

pub struct App {
    state: AppState,
    screen: CurrentScreen,
    login_use_case: LoginUseCase,
    resolve_token_use_case: ResolveTokenUseCase,
    discord_data: Arc<dyn DiscordDataPort>,
    pending_token: Option<(String, TokenSource)>,
    current_token: Option<AuthToken>,
    gateway_client: Option<GatewayClient>,
    gateway_rx: Option<mpsc::UnboundedReceiver<GatewayEventKind>>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    typing_manager: TypingIndicatorManager,
    last_typing_cleanup: Instant,
    last_typing_sent: Option<(ChannelId, Instant)>,
    markdown_service: Arc<MarkdownService>,
}

impl App {
    #[must_use]
    pub fn new(
        auth_port: Arc<dyn AuthPort>,
        discord_data: Arc<dyn DiscordDataPort>,
        storage_port: Arc<dyn TokenStoragePort>,
    ) -> Self {
        let login_use_case = LoginUseCase::new(auth_port, storage_port.clone());
        let resolve_token_use_case = ResolveTokenUseCase::new(storage_port);
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let markdown_service = Arc::new(MarkdownService::new());

        Self {
            state: AppState::Login,
            screen: CurrentScreen::Login(LoginScreen::new()),
            login_use_case,
            resolve_token_use_case,
            discord_data,
            pending_token: None,
            current_token: None,
            gateway_client: None,
            gateway_rx: None,
            action_tx,
            action_rx,
            typing_manager: TypingIndicatorManager::new(),
            last_typing_cleanup: Instant::now(),
            last_typing_sent: None,
            markdown_service,
        }
    }

    /// # Errors
    /// Returns error if terminal or token resolution fails.
    pub async fn run(
        mut self,
        terminal: &mut DefaultTerminal,
        cli_token: Option<String>,
    ) -> color_eyre::Result<()> {
        if let Some(resolved) = self.resolve_token_use_case.execute(cli_token).await? {
            info!(source = %resolved.source, "Found existing token");
            self.pending_token = Some((resolved.token.as_str().to_string(), resolved.source));
        }

        if let Some((token, source)) = self.pending_token.take() {
            self.attempt_auto_login(token, source).await;
        }

        self.run_event_loop(terminal).await?;

        self.disconnect_gateway();
        info!("Application exiting normally");
        Ok(())
    }

    async fn run_event_loop(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        let mut terminal_events = EventStream::new();
        let mut typing_cleanup_interval = interval(TYPING_CLEANUP_INTERVAL);

        terminal.draw(|frame| self.render(frame))?;

        while self.state != AppState::Exiting {
            let gateway_future = match &mut self.gateway_rx {
                Some(rx) => futures_util::future::Either::Left(rx.recv()),
                None => futures_util::future::Either::Right(std::future::pending()),
            };
            let terminal_event = terminal_events.next();

            tokio::select! {
                biased;

                Some(event) = gateway_future => {
                    self.handle_gateway_event(event);
                    terminal.draw(|frame| self.render(frame))?;
                }

                Some(action) = self.action_rx.recv() => {
                    self.handle_action(action);
                    terminal.draw(|frame| self.render(frame))?;
                }

                Some(Ok(event)) = terminal_event => {
                    let result = self.handle_terminal_event(event).await;
                    match result {
                        EventResult::Exit => {
                            self.state = AppState::Exiting;
                        }
                        EventResult::OpenEditor => {
                            self.handle_open_editor(terminal)?;
                        }
                        _ => {}
                    }
                    terminal.draw(|frame| self.render(frame))?;
                }

                _ = typing_cleanup_interval.tick() => {
                    self.cleanup_typing_indicators();
                    terminal.draw(|frame| self.render(frame))?;
                }
            }
        }

        Ok(())
    }

    async fn handle_terminal_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Key(key) => self.handle_key(key).await,
            _ => EventResult::Continue,
        }
    }

    async fn attempt_auto_login(&mut self, token: String, source: TokenSource) {
        debug!("Attempting automatic login");

        if let CurrentScreen::Login(ref mut login_screen) = self.screen {
            login_screen.set_validating();
        }

        let request = LoginRequest::new(token.clone(), source);
        match self.login_use_case.execute(request).await {
            Ok(response) => {
                info!(user = %response.user.display_name(), "Auto-login successful");
                if let Some(auth_token) = AuthToken::new(&token) {
                    self.current_token = Some(auth_token);
                }
                self.transition_to_chat(response.user).await;
            }
            Err(e) => {
                error!(error = %e, "Auto-login failed");
                if let CurrentScreen::Login(ref mut login_screen) = self.screen {
                    login_screen.set_error(e.to_string());
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            CurrentScreen::Login(screen) => {
                frame.render_widget(&*screen, frame.area());
            }
            CurrentScreen::Chat(state) => {
                frame.render_stateful_widget(ChatScreen::new(), frame.area(), state);
            }
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) -> EventResult {
        if EventHandler::is_quit_event(&key) && self.state == AppState::Login {
            return EventResult::Exit;
        }

        let result = match &mut self.screen {
            CurrentScreen::Login(screen) => {
                if screen.handle_key(key) {
                    self.handle_login_submit().await;
                }
                return EventResult::Continue;
            }
            CurrentScreen::Chat(state) => state.handle_key(key),
        };

        match result {
            ChatKeyResult::Quit => return EventResult::Exit,
            ChatKeyResult::Logout => {
                self.transition_to_login();
            }
            ChatKeyResult::CopyToClipboard(text) => {
                debug!(text = %text, "Copy to clipboard requested");
            }
            ChatKeyResult::LoadGuildChannels(guild_id) => {
                self.load_guild_channels(guild_id).await;
            }
            ChatKeyResult::LoadChannelMessages(channel_id) => {
                self.load_channel_messages(channel_id).await;
            }
            ChatKeyResult::LoadDmMessages {
                channel_id,
                recipient_name,
            } => {
                debug!(channel_id = %channel_id, recipient = %recipient_name, "Loading DM messages");
                self.load_channel_messages(channel_id).await;
            }
            ChatKeyResult::LoadHistory {
                channel_id,
                before_message_id,
            } => {
                debug!(channel_id = %channel_id, before = %before_message_id, "Loading history");
                self.load_history(channel_id, before_message_id);
            }
            ChatKeyResult::ReplyToMessage {
                message_id,
                mention,
            } => {
                debug!(message_id = %message_id, mention = mention, "Reply to message requested");
                self.handle_reply_to_message(message_id);
            }
            ChatKeyResult::EditMessage(message_id) => {
                debug!(message_id = %message_id, "Edit message requested");
            }
            ChatKeyResult::DeleteMessage(message_id) => {
                debug!(message_id = %message_id, "Delete message requested");
            }
            ChatKeyResult::OpenAttachments(message_id) => {
                debug!(message_id = %message_id, "Open attachments requested");
            }
            ChatKeyResult::JumpToMessage(message_id) => {
                debug!(message_id = %message_id, "Jump to message requested");
            }
            ChatKeyResult::SendMessage { content, reply_to } => {
                self.handle_send_message(content, reply_to).await;
            }
            ChatKeyResult::StartTyping => {
                self.handle_start_typing().await;
            }
            ChatKeyResult::OpenEditor => {
                return EventResult::OpenEditor;
            }
            ChatKeyResult::Consumed => {}
        }

        EventResult::Continue
    }

    async fn handle_login_submit(&mut self) {
        let (token, persist) = if let CurrentScreen::Login(ref screen) = self.screen {
            match screen.token() {
                Some(t) => (t.to_string(), screen.should_persist()),
                None => return,
            }
        } else {
            return;
        };

        if let CurrentScreen::Login(ref mut screen) = self.screen {
            screen.set_validating();
        }

        let mut request = LoginRequest::new(token.clone(), TokenSource::UserInput);
        if !persist {
            request = request.without_persistence();
        }

        match self.login_use_case.execute(request).await {
            Ok(response) => {
                info!(
                    user = %response.user.display_name(),
                    persisted = response.token_persisted,
                    "Login successful"
                );
                if let Some(auth_token) = AuthToken::new(&token) {
                    self.current_token = Some(auth_token);
                }
                self.transition_to_chat(response.user).await;
            }
            Err(e) => {
                error!(error = %e, "Login failed");
                self.handle_login_error(&e);
            }
        }
    }

    async fn transition_to_chat(&mut self, user: User) {
        self.state = AppState::Chat;
        let mut chat_state = ChatScreenState::new(user, self.markdown_service.clone());

        let token_clone = self.current_token.clone();
        if let Some(ref token) = token_clone {
            self.load_discord_data(&mut chat_state, token).await;
        }

        self.screen = CurrentScreen::Chat(Box::new(chat_state));

        if let Some(ref token) = self.current_token.clone() {
            self.connect_gateway(token);
        }
    }

    fn connect_gateway(&mut self, token: &AuthToken) {
        let config = GatewayClientConfig::new()
            .with_intents(
                GatewayIntents::default_client()
                    .with_presence()
                    .with_reactions(),
            )
            .with_auto_reconnect(true)
            .with_max_reconnect_attempts(10);

        let mut client = GatewayClient::new(config);

        match client.connect(token.as_str()) {
            Ok(rx) => {
                info!("Gateway connection initiated");
                self.gateway_rx = Some(rx);
                self.gateway_client = Some(client);

                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_connection_status(ConnectionStatus::Connecting);
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to initiate gateway connection");
            }
        }
    }

    fn disconnect_gateway(&mut self) {
        if let Some(ref client) = self.gateway_client {
            client.disconnect();
        }
        self.gateway_client = None;
        self.gateway_rx = None;
    }

    fn handle_gateway_event(&mut self, event: GatewayEventKind) {
        match event {
            GatewayEventKind::Connected { session_id, .. } => {
                info!(session_id = %session_id, "Gateway connected");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_connection_status(ConnectionStatus::Connected);
                }
            }
            GatewayEventKind::Disconnected { reason, can_resume } => {
                warn!(reason = %reason, can_resume = can_resume, "Gateway disconnected");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_connection_status(ConnectionStatus::Disconnected);
                }
            }
            GatewayEventKind::Reconnecting { attempt } => {
                info!(attempt = attempt, "Gateway reconnecting");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_connection_status(ConnectionStatus::Connecting);
                }
            }
            GatewayEventKind::Resumed => {
                info!("Gateway session resumed");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_connection_status(ConnectionStatus::Connected);
                }
            }
            GatewayEventKind::HeartbeatAck { latency_ms } => {
                debug!(latency_ms = latency_ms, "Heartbeat acknowledged");
            }
            GatewayEventKind::Dispatch(dispatch) => {
                self.handle_dispatch_event(dispatch);
            }
            GatewayEventKind::Error {
                message,
                recoverable,
            } => {
                if recoverable {
                    warn!(error = %message, "Recoverable gateway error");
                } else {
                    error!(error = %message, "Fatal gateway error");
                }
            }
        }
    }

    fn handle_dispatch_event(&mut self, event: DispatchEvent) {
        match event {
            DispatchEvent::MessageCreate { message } => {
                self.handle_message_create(message);
            }
            DispatchEvent::MessageUpdate { message } => {
                self.handle_message_update(message);
            }
            DispatchEvent::MessageDelete { message_id, .. } => {
                debug!(message_id = %message_id, "Message deleted");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.remove_message(message_id);
                }
            }
            DispatchEvent::MessageDeleteBulk { message_ids, .. } => {
                debug!(count = message_ids.len(), "Bulk message delete");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    for message_id in message_ids {
                        state.remove_message(message_id);
                    }
                }
            }
            DispatchEvent::TypingStart {
                channel_id,
                user_id,
                username,
                ..
            } => {
                self.handle_typing_start(channel_id, user_id, username);
            }
            DispatchEvent::PresenceUpdate {
                user_id, status, ..
            } => {
                debug!(user_id = %user_id, status = ?status, "Presence updated");
            }
            DispatchEvent::MessageReactionAdd {
                message_id, emoji, ..
            } => {
                debug!(message_id = %message_id, emoji = %emoji.display(), "Reaction added");
            }
            DispatchEvent::MessageReactionRemove {
                message_id, emoji, ..
            } => {
                debug!(message_id = %message_id, emoji = %emoji.display(), "Reaction removed");
            }
            DispatchEvent::ChannelCreate {
                channel_id, name, ..
            }
            | DispatchEvent::ChannelUpdate {
                channel_id, name, ..
            } => {
                debug!(channel_id = %channel_id, name = %name, "Channel created/updated");
            }
            DispatchEvent::ChannelDelete { channel_id, .. } => {
                info!(channel_id = %channel_id, "Channel deleted");
            }
            DispatchEvent::GuildCreate {
                guild_id,
                name,
                unavailable,
            } => {
                if !unavailable {
                    info!(guild_id = %guild_id, name = %name, "Guild available");
                }
            }
            DispatchEvent::GuildUpdate { guild_id, name } => {
                debug!(guild_id = %guild_id, name = %name, "Guild updated");
            }
            DispatchEvent::GuildDelete {
                guild_id,
                unavailable,
            } => {
                if unavailable {
                    warn!(guild_id = %guild_id, "Guild became unavailable");
                } else {
                    info!(guild_id = %guild_id, "Left guild");
                }
            }
            DispatchEvent::UserUpdate {
                user_id, username, ..
            } => {
                debug!(user_id = %user_id, username = %username, "User updated");
            }
            DispatchEvent::Ready {
                user_id, guilds, ..
            } => {
                info!(user_id = %user_id, guild_count = guilds.len(), "Gateway ready");
            }
            _ => {}
        }
    }

    fn handle_message_create(&mut self, message: crate::domain::entities::Message) {
        let channel_id = message.channel_id();
        debug!(message_id = %message.id(), channel_id = %channel_id, "New message received");

        if let CurrentScreen::Chat(ref mut state) = self.screen {
            state.add_message(message);
        }

        self.typing_manager.remove_typing(channel_id, "");
        self.update_typing_indicator(channel_id);
    }

    fn handle_message_update(&mut self, message: crate::domain::entities::Message) {
        debug!(message_id = %message.id(), "Message updated");
        if let CurrentScreen::Chat(ref mut state) = self.screen {
            state.update_message(message);
        }
    }

    fn handle_typing_start(
        &mut self,
        channel_id: ChannelId,
        user_id: String,
        username: Option<String>,
    ) {
        let display_name = username.unwrap_or_else(|| user_id.clone());
        debug!(user = %display_name, channel_id = %channel_id, "User started typing");

        self.typing_manager
            .add_typing(channel_id, user_id, display_name);
        self.update_typing_indicator(channel_id);
    }

    fn update_typing_indicator(&mut self, channel_id: ChannelId) {
        if let CurrentScreen::Chat(ref mut state) = self.screen
            && state.message_pane_data().channel_id() == Some(channel_id)
        {
            let indicator = self.typing_manager.format_typing_indicator(channel_id);
            state.set_typing_indicator(indicator);
        }
    }

    fn cleanup_typing_indicators(&mut self) {
        self.last_typing_cleanup = Instant::now();
        self.typing_manager.cleanup_expired();

        if let CurrentScreen::Chat(ref mut state) = self.screen
            && let Some(channel_id) = state.message_pane_data().channel_id()
        {
            let indicator = self.typing_manager.format_typing_indicator(channel_id);
            state.set_typing_indicator(indicator);
        }
    }

    async fn load_discord_data(&self, state: &mut ChatScreenState, token: &AuthToken) {
        let guilds_future = self.discord_data.fetch_guilds(token);
        let dms_future = self.discord_data.fetch_dm_channels(token);

        let (guilds_result, dms_result) = tokio::join!(guilds_future, dms_future);

        if let Ok(dm_channels) = dms_result {
            let dm_users: Vec<(String, String)> = dm_channels
                .into_iter()
                .map(|dm| (dm.channel_id, dm.recipient_name))
                .collect();
            state.set_dm_users(dm_users);
            debug!(
                count = state.guilds_tree_data().dm_users().len(),
                "Loaded DM channels"
            );
        }

        match guilds_result {
            Ok(guilds) => {
                info!(count = guilds.len(), "Loaded guilds from Discord");
                state.set_guilds(guilds);
            }
            Err(e) => {
                warn!(error = %e, "Failed to load guilds from Discord");
            }
        }
    }

    async fn load_guild_channels(&mut self, guild_id: GuildId) {
        let channels = if let Some(ref token) = self.current_token {
            match self
                .discord_data
                .fetch_channels(token, guild_id.as_u64())
                .await
            {
                Ok(channels) => {
                    debug!(guild_id = %guild_id, count = channels.len(), "Loaded channels for guild");
                    Some(channels)
                }
                Err(e) => {
                    warn!(guild_id = %guild_id, error = %e, "Failed to load channels for guild");
                    None
                }
            }
        } else {
            None
        };

        if let (Some(channels), CurrentScreen::Chat(state)) = (channels, &mut self.screen) {
            state.set_channels(guild_id, channels);
        }
    }

    async fn load_channel_messages(&mut self, channel_id: ChannelId) {
        self.typing_manager.clear_channel(channel_id);

        let messages = if let Some(ref token) = self.current_token {
            let options = FetchMessagesOptions::default().with_limit(50);
            match self
                .discord_data
                .fetch_messages(token, channel_id.as_u64(), options)
                .await
            {
                Ok(messages) => {
                    debug!(channel_id = %channel_id, count = messages.len(), "Loaded messages for channel");
                    Some(messages)
                }
                Err(e) => {
                    warn!(channel_id = %channel_id, error = %e, "Failed to load messages for channel");
                    if let CurrentScreen::Chat(state) = &mut self.screen {
                        state.set_message_error(e.to_string());
                    }
                    None
                }
            }
        } else {
            None
        };

        if let (Some(messages), CurrentScreen::Chat(state)) = (messages, &mut self.screen) {
            state.set_messages(messages);
            state.set_typing_indicator(None);
        }
    }

    fn load_history(&mut self, channel_id: ChannelId, before_message_id: MessageId) {
        let Some(ref token) = self.current_token else {
            return;
        };

        let token = token.clone();
        let discord = self.discord_data.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            match discord
                .load_more_before_id(&token, channel_id.as_u64(), before_message_id.as_u64(), 50)
                .await
            {
                Ok(messages) => {
                    debug!(count = messages.len(), "Loaded historical messages");
                    let _ = tx.send(Action::HistoryLoaded(messages));
                }
                Err(e) => {
                    let _ = tx.send(Action::LoadError(e.to_string()));
                }
            }
        });
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::HistoryLoaded(messages) => {
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.prepend_messages(messages);
                }
            }
            Action::LoadError(e) => {
                warn!(error = %e, "Failed to load history");
            }
        }
    }

    fn transition_to_login(&mut self) {
        self.disconnect_gateway();
        self.state = AppState::Login;
        self.current_token = None;
        self.typing_manager = TypingIndicatorManager::new();
        self.screen = CurrentScreen::Login(LoginScreen::new());
    }

    fn handle_login_error(&mut self, error: &AuthError) {
        if let CurrentScreen::Login(ref mut screen) = self.screen {
            let message = match error {
                AuthError::InvalidTokenFormat { .. } => {
                    "Invalid token format. Please check your token.".to_string()
                }
                AuthError::TokenRejected { .. } => {
                    "Token rejected. It may be invalid or expired.".to_string()
                }
                AuthError::NetworkError { message } => {
                    format!("Network error: {message}")
                }
                AuthError::RateLimited { retry_after_ms } => {
                    format!("Rate limited. Try again in {}s", retry_after_ms / 1000)
                }
                _ => error.to_string(),
            };
            screen.set_error(message);
        }
    }

    fn handle_reply_to_message(&mut self, message_id: crate::domain::entities::MessageId) {
        if let CurrentScreen::Chat(ref mut state) = self.screen
            && let Some(author_name) = state.get_reply_author(message_id)
        {
            state.start_reply(message_id, author_name);
        }
    }

    async fn handle_send_message(
        &mut self,
        content: String,
        reply_to: Option<crate::domain::entities::MessageId>,
    ) {
        let channel_id = if let CurrentScreen::Chat(ref state) = self.screen {
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id)
        } else {
            None
        };

        let Some(channel_id) = channel_id else {
            warn!("Cannot send message: no channel selected");
            return;
        };

        let Some(ref token) = self.current_token else {
            warn!("Cannot send message: no token available");
            return;
        };

        let request = if let Some(reply_id) = reply_to {
            SendMessageRequest::new(channel_id, content).with_reply(reply_id)
        } else {
            SendMessageRequest::new(channel_id, content)
        };

        debug!(channel_id = %channel_id, has_reply = reply_to.is_some(), "Sending message");

        match self.discord_data.send_message(token, request).await {
            Ok(message) => {
                info!(message_id = %message.id(), "Message sent successfully");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.add_message(message);
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to send message");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_error(format!("Failed to send: {e}"));
                }
            }
        }

        self.last_typing_sent = None;
    }

    async fn handle_start_typing(&mut self) {
        let channel_id = if let CurrentScreen::Chat(ref state) = self.screen {
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id)
        } else {
            None
        };

        let Some(channel_id) = channel_id else {
            return;
        };

        let should_send = match self.last_typing_sent {
            Some((last_channel, last_time)) => {
                last_channel != channel_id || last_time.elapsed() >= TYPING_THROTTLE_DURATION
            }
            None => true,
        };

        if !should_send {
            return;
        }

        let Some(ref token) = self.current_token else {
            return;
        };

        if let Err(e) = self
            .discord_data
            .send_typing_indicator(token, channel_id)
            .await
        {
            debug!(error = %e, "Failed to send typing indicator");
        } else {
            self.last_typing_sent = Some((channel_id, Instant::now()));
        }
    }

    fn handle_open_editor(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        use std::io::Write;

        // Get current input content and reply state
        let (current_content, reply_info) = if let CurrentScreen::Chat(ref state) = self.screen {
            let content = state.message_input_value();
            let reply = state.message_input_reply_info();
            (content, reply)
        } else {
            return Ok(());
        };

        // Create temp file with .md extension
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("discordo_message_{}.md", std::process::id()));

        // Write current content to temp file
        {
            let mut file = std::fs::File::create(&temp_path)?;
            file.write_all(current_content.as_bytes())?;
        }

        // Get editor from environment
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| {
                // Try common editors in order of preference
                for editor in &["nvim", "vim", "nano", "vi"] {
                    if std::process::Command::new("which")
                        .arg(editor)
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                    {
                        return (*editor).to_string();
                    }
                }
                "vi".to_string()
            });

        debug!(editor = %editor, path = %temp_path.display(), "Opening external editor");

        // Suspend TUI - restore terminal to normal state
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;

        // Spawn editor and wait
        let status = std::process::Command::new(&editor).arg(&temp_path).status();

        // Resume TUI
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide
        )?;

        // Force terminal refresh
        terminal.clear()?;

        match status {
            Ok(exit_status) if exit_status.success() => {
                // Read file content back
                let new_content = std::fs::read_to_string(&temp_path).unwrap_or_default();

                // Update message input with new content
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_input_content(&new_content);

                    // Restore reply state if it existed
                    if let Some((message_id, author)) = reply_info {
                        state.start_reply(message_id, author);
                    }
                }

                info!("Editor closed successfully");
            }
            Ok(exit_status) => {
                warn!(
                    exit_code = ?exit_status.code(),
                    "Editor exited with non-zero status"
                );
            }
            Err(e) => {
                error!(error = %e, editor = %editor, "Failed to spawn editor");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_error(format!("Failed to open editor: {e}"));
                }
            }
        }

        // Clean up temp file
        if let Err(e) = std::fs::remove_file(&temp_path) {
            debug!(error = %e, "Failed to remove temp file");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::Guild;
    use crate::domain::ports::{
        DirectMessageChannel, FetchMessagesOptions, SendMessageRequest,
        mocks::{MockAuthPort, MockTokenStorage},
    };

    struct MockDiscordData;

    #[async_trait::async_trait]
    impl DiscordDataPort for MockDiscordData {
        async fn fetch_guilds(&self, _token: &AuthToken) -> Result<Vec<Guild>, AuthError> {
            Ok(vec![Guild::new(1_u64, "Test Guild")])
        }

        async fn fetch_channels(
            &self,
            _token: &AuthToken,
            _guild_id: u64,
        ) -> Result<Vec<crate::domain::entities::Channel>, AuthError> {
            Ok(vec![])
        }

        async fn fetch_dm_channels(
            &self,
            _token: &AuthToken,
        ) -> Result<Vec<DirectMessageChannel>, AuthError> {
            Ok(vec![])
        }

        async fn fetch_messages(
            &self,
            _token: &AuthToken,
            _channel_id: u64,
            _options: FetchMessagesOptions,
        ) -> Result<Vec<crate::domain::entities::Message>, AuthError> {
            Ok(vec![])
        }

        async fn load_more_before_id(
            &self,
            _token: &AuthToken,
            _channel_id: u64,
            _message_id: u64,
            _limit: u8,
        ) -> Result<Vec<crate::domain::entities::Message>, AuthError> {
            Ok(vec![])
        }

        async fn send_message(
            &self,
            _token: &AuthToken,
            _request: SendMessageRequest,
        ) -> Result<crate::domain::entities::Message, AuthError> {
            Err(AuthError::unexpected("mock not implemented"))
        }

        async fn send_typing_indicator(
            &self,
            _token: &AuthToken,
            _channel_id: ChannelId,
        ) -> Result<(), AuthError> {
            Ok(())
        }
    }

    #[test]
    fn test_app_creation() {
        let auth = Arc::new(MockAuthPort::new(true));
        let data = Arc::new(MockDiscordData);
        let storage = Arc::new(MockTokenStorage::new());
        let app = App::new(auth, data, storage);

        assert_eq!(app.state, AppState::Login);
    }
}
