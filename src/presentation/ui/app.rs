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
use crate::domain::ConnectionStatus;
use crate::domain::entities::{AuthToken, ChannelId, GuildId, MessageId, UserCache};
use crate::domain::errors::AuthError;
use crate::domain::ports::{
    AuthPort, DiscordDataPort, EditMessageRequest, SendMessageRequest, TokenStoragePort,
};
use crate::infrastructure::discord::{
    DispatchEvent, GatewayClient, GatewayClientConfig, GatewayCommand, GatewayEventKind,
    GatewayIntents, TypingIndicatorManager, identity::ClientIdentity,
};
use crate::infrastructure::{ClipboardService, StateStore};
use crate::infrastructure::image::{ImageLoadedEvent, ImageLoader};
use crate::presentation::events::{EventHandler, EventResult};
use crate::presentation::theme::Theme;
use crate::presentation::ui::{
    ChatKeyResult, ChatScreen, ChatScreenState, LoginAction, LoginScreen, SplashScreen,
    backend::{Action, Backend, BackendCommand},
};

const TYPING_CLEANUP_INTERVAL: Duration = Duration::from_secs(2);
const TYPING_THROTTLE_DURATION: Duration = Duration::from_secs(8);
const ANIMATION_TICK_RATE: Duration = Duration::from_millis(33);
const IMAGE_CHECK_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Login,
    Initializing,
    Chat,
    Exiting,
}

enum CurrentScreen {
    Login(LoginScreen),
    Splash(SplashScreen),
    Chat(Box<ChatScreenState>),
}

pub struct App {
    state: AppState,
    screen: CurrentScreen,
    login_use_case: LoginUseCase,
    resolve_token_use_case: ResolveTokenUseCase,
    command_tx: mpsc::UnboundedSender<BackendCommand>,
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
    user_cache: UserCache,
    current_user_id: Option<String>,
    pending_chat_state: Option<Box<ChatScreenState>>,
    pending_read_states: Option<Vec<crate::domain::entities::ReadState>>,
    gateway_ready: bool,
    connection_status: ConnectionStatus,
    should_render: bool,
    /// Image loader for async image loading.
    image_loader: Option<Arc<ImageLoader>>,
    /// Receiver for image load completion events.
    image_load_rx: Option<mpsc::UnboundedReceiver<ImageLoadedEvent>>,
    /// Last time we checked for images to load.
    last_image_check: Instant,
    disable_user_colors: bool,
    theme: Theme,
    identity: Arc<ClientIdentity>,
    state_store: StateStore,
    state_save_tx: mpsc::UnboundedSender<(Option<GuildId>, Option<ChannelId>)>,
    clipboard_service: ClipboardService,
    notification: Option<(String, std::time::Instant)>,
}

impl App {
    /// # Panics
    ///
    /// Panics if the application state persistence cannot be initialized.
    #[must_use]
    pub fn new(
        auth_port: Arc<dyn AuthPort>,
        discord_data: Arc<dyn DiscordDataPort>,
        storage_port: Arc<dyn TokenStoragePort>,
        disable_user_colors: bool,
        theme: Theme,
        identity: Arc<ClientIdentity>,
    ) -> Self {
        let login_use_case = LoginUseCase::new(auth_port, storage_port.clone());
        let resolve_token_use_case = ResolveTokenUseCase::new(storage_port);
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let markdown_service = Arc::new(MarkdownService::new());

        let backend = Backend::new(discord_data, command_rx, action_tx.clone());
        tokio::spawn(backend.run());

        let state_store = StateStore::new();
        let (state_save_tx, mut state_save_rx) =
            mpsc::unbounded_channel::<(Option<GuildId>, Option<ChannelId>)>();
        let store = state_store.clone();

        tokio::spawn(async move {
            const DEBOUNCE_DURATION: Duration = Duration::from_secs(1);
            let mut pending_state: Option<(Option<GuildId>, Option<ChannelId>)> = None;
            let mut timer = Box::pin(tokio::time::sleep(Duration::MAX));

            loop {
                tokio::select! {
                    Some(state) = state_save_rx.recv() => {
                        pending_state = Some(state);
                        timer = Box::pin(tokio::time::sleep(DEBOUNCE_DURATION));
                    }
                    () = &mut timer, if pending_state.is_some() => {
                        if let Some((guild_id, channel_id)) = pending_state.take() {
                            let gid = guild_id.map(|g| g.as_u64().to_string());
                            let cid = channel_id.map(|c| c.as_u64().to_string());
                            if let Err(e) = store.save(gid, cid).await {
                                tracing::warn!("Failed to save state: {e}");
                            }
                        }
                        timer = Box::pin(tokio::time::sleep(Duration::MAX));
                    }
                    else => break,
                }
            }
        });

        Self {
            state: AppState::Login,
            screen: CurrentScreen::Login(LoginScreen::new().with_theme(theme)),
            login_use_case,
            resolve_token_use_case,
            command_tx,
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
            user_cache: UserCache::new(),
            current_user_id: None,
            pending_chat_state: None,
            pending_read_states: None,
            gateway_ready: false,
            connection_status: ConnectionStatus::Disconnected,
            should_render: true,
            image_loader: None,
            image_load_rx: None,
            last_image_check: Instant::now(),
            disable_user_colors,
            theme,
            identity,
            state_store,
            state_save_tx,
            clipboard_service: ClipboardService::new(),
            notification: None,
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
        let mut animation_interval = interval(ANIMATION_TICK_RATE);

        terminal.draw(|frame| self.render(frame))?;

        while self.state != AppState::Exiting {
            self.should_render = false;

            let gateway_future = match &mut self.gateway_rx {
                Some(rx) => futures_util::future::Either::Left(rx.recv()),
                None => futures_util::future::Either::Right(std::future::pending()),
            };
            let terminal_event = terminal_events.next();

            let image_load_future = match &mut self.image_load_rx {
                Some(rx) => futures_util::future::Either::Left(rx.recv()),
                None => futures_util::future::Either::Right(std::future::pending()),
            };

            tokio::select! {
                biased;

                Some(event) = gateway_future => {
                    self.handle_gateway_event(event);
                    self.should_render = true;
                }

                Some(action) = self.action_rx.recv() => {
                    self.handle_action(action);
                    self.should_render = true;
                }

                Some(event) = image_load_future => {
                    self.handle_image_loaded(event);
                    self.should_render = true;
                }

                _ = animation_interval.tick() => {
                     if let CurrentScreen::Splash(splash) = &mut self.screen {
                        splash.tick(ANIMATION_TICK_RATE);

                        if splash.state.animation_complete && self.pending_chat_state.is_some() {
                             self.state = AppState::Chat;
                             self.screen = CurrentScreen::Chat(self.pending_chat_state.take().unwrap());
                             self.should_render = true;
                        } else if !splash.state.intro_finished || (splash.state.data_ready && !splash.state.animation_complete) {
                             self.should_render = true;
                        }
                    } else if let CurrentScreen::Chat(state) = &mut self.screen {
                        state.tick(ANIMATION_TICK_RATE);
                        if !state.has_entered() {
                            self.should_render = true;
                        }
                    }

                    if self.last_image_check.elapsed() > IMAGE_CHECK_INTERVAL {
                        self.trigger_image_loads();
                        self.last_image_check = Instant::now();
                    }
                }

                Some(Ok(event)) = terminal_event => {
                    let result = self.handle_terminal_event(&event);
                    match result {
                        EventResult::Exit => {
                            self.state = AppState::Exiting;
                        }
                        EventResult::OpenEditor {
                            initial_content,
                            message_id,
                        } => {
                            self.handle_open_editor(terminal, &initial_content, message_id)?;
                            self.should_render = true;
                        }
                        _ => {
                            self.should_render = true;
                        }
                    }
                }

                _ = typing_cleanup_interval.tick() => {
                    self.cleanup_typing_indicators();
                    self.should_render = true;
                }
            }

            if self.should_render {
                terminal.draw(|frame| self.render(frame))?;
            }
        }

        Ok(())
    }

    fn handle_terminal_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::Key(key) => self.handle_key(*key),
            _ => EventResult::Continue,
        }
    }

    fn save_state(&self, guild_id: Option<GuildId>, channel_id: Option<ChannelId>) {
        let _ = self.state_save_tx.send((guild_id, channel_id));
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
                self.start_app_loading(response.user);
            }
            Err(e) => {
                error!(error = %e, "Auto-login failed");
                if let CurrentScreen::Login(ref mut login_screen) = self.screen {
                    login_screen.set_error(e.to_string());
                }
            }
        }
    }

    pub fn show_notification(&mut self, message: String) {
        self.notification = Some((message, std::time::Instant::now()));
    }

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            CurrentScreen::Login(screen) => {
                frame.render_widget(&*screen, frame.area());
            }
            CurrentScreen::Splash(screen) => {
                frame.render_widget(screen, frame.area());
            }
            CurrentScreen::Chat(state) => {
                let width = frame.area().width;
                state.update_visible_image_protocols(width);
                frame.render_stateful_widget(ChatScreen::new(), frame.area(), state);
            }
        }

        if let Some((message, time)) = &self.notification 
            && time.elapsed() < std::time::Duration::from_secs(3) {
            use ratatui::widgets::{Block, Borders, Paragraph};
            use ratatui::layout::Rect;
            use ratatui::style::{Style, Color, Modifier};

            let area = frame.area();
            let max_width = area.width.saturating_sub(2); // Keep some margin
            let width = u16::try_from(message.len())
                .unwrap_or(u16::MAX)
                .saturating_add(4)
                .min(max_width);
            let height = 3;
            let x = area.width.saturating_sub(width).saturating_sub(1);
            let y = 1;

            let rect = Rect::new(x, y, width, height);
            let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green));
            let para = Paragraph::new(message.as_str())
                .block(block)
                .style(Style::default().add_modifier(Modifier::BOLD));
            
            frame.render_widget(ratatui::widgets::Clear, rect);
            frame.render_widget(para, rect);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_key(&mut self, key: KeyEvent) -> EventResult {
        if EventHandler::is_quit_event(&key) && self.state == AppState::Login {
            return EventResult::Exit;
        }

        let result = match &mut self.screen {
            CurrentScreen::Login(screen) => {
                match screen.handle_key(key) {
                    LoginAction::Submit => self.handle_login_submit(),
                    LoginAction::DeleteToken => self.handle_delete_token(),
                    LoginAction::None => {}
                }
                return EventResult::Continue;
            }
            CurrentScreen::Splash(_) => return EventResult::Continue,
            CurrentScreen::Chat(state) => state.handle_key(key),
        };

        self.process_chat_key_result(result)
    }

    #[allow(clippy::too_many_lines)]
    fn process_chat_key_result(&mut self, result: ChatKeyResult) -> EventResult {
        match result {
            ChatKeyResult::Quit => return EventResult::Exit,
            ChatKeyResult::Logout => {
                self.transition_to_login();
            }
            ChatKeyResult::CopyToClipboard(text) => {
                debug!(text = %text, "Copy to clipboard requested");
                self.clipboard_service.set_text(text);
                self.show_notification("Copied to clipboard".to_string());
            }
            ChatKeyResult::LoadGuildChannels(guild_id) => {
                self.load_guild_channels(guild_id);
            }
            ChatKeyResult::LoadChannelMessages {
                channel_id,
                guild_id,
            } => {
                self.save_state(guild_id, Some(channel_id));
                if let Some(guild_id) = guild_id {
                    self.subscribe_to_channel(guild_id, channel_id);
                }
                self.load_channel_messages(channel_id);
            }
            ChatKeyResult::LoadForumThreads {
                channel_id,
                guild_id,
                offset,
            } => {
                self.save_state(guild_id, Some(channel_id));
                if let Some(guild_id) = guild_id {
                    self.subscribe_to_channel(guild_id, channel_id);
                }
                self.load_forum_threads(channel_id, guild_id, offset);
            }
            ChatKeyResult::LoadDmMessages {
                channel_id,
                recipient_name,
            } => {
                self.save_state(None, Some(channel_id));
                debug!(channel_id = %channel_id, recipient = %recipient_name, "Loading DM messages");
                self.load_channel_messages(channel_id);
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
                self.handle_reply_to_message(message_id, mention);
            }
            ChatKeyResult::SubmitEdit {
                message_id,
                content,
            } => {
                self.handle_edit_message(message_id, content);
            }
            ChatKeyResult::EditMessage(message_id) => {
                debug!(message_id = %message_id, "Edit message requested");
            }
            ChatKeyResult::DeleteMessage(message_id) => {
                debug!(message_id = %message_id, "Delete message requested");
                self.handle_delete_message(message_id);
            }
            ChatKeyResult::OpenAttachments(message_id) => {
                debug!(message_id = %message_id, "Open attachments requested");
            }
            ChatKeyResult::JumpToMessage(message_id) => {
                debug!(message_id = %message_id, "Jump to message requested");
                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.jump_to_message(message_id);
                }
            }
            ChatKeyResult::SendMessage {
                content,
                reply_to,
                attachments,
            } => {
                self.handle_send_message(content, reply_to, attachments);
            }
            ChatKeyResult::StartTyping => {
                self.handle_start_typing();
            }
            ChatKeyResult::OpenEditor {
                initial_content,
                message_id,
            } => {
                return EventResult::OpenEditor {
                    initial_content,
                    message_id,
                };
            }
            ChatKeyResult::Paste => {
                let clipboard = self.clipboard_service.clone();
                let tx = self.action_tx.clone();
                
                tokio::task::spawn_blocking(move || {
                    if let Some(image) = clipboard.get_image() {
                        let temp_dir = std::env::temp_dir();
                        let filename = format!("paste_{}.png", uuid::Uuid::new_v4());
                        let path = temp_dir.join(filename);
                        
                        if let Some(img) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                            u32::try_from(image.width).unwrap_or_default(),
                            u32::try_from(image.height).unwrap_or_default(),
                            image.bytes.into_owned()
                        ) {
                            if img.save(&path).is_ok() {
                                let _ = tx.send(Action::PasteImageLoaded(path));
                                return;
                            }
                        }
                    } 
                    
                    if let Some(text) = clipboard.get_text() {
                        let _ = tx.send(Action::PasteTextLoaded(text));
                    }
                });
            }
            ChatKeyResult::ToggleHelp | ChatKeyResult::Consumed | ChatKeyResult::Ignored => {}
        }

        EventResult::Continue
    }

    fn handle_login_submit(&mut self) {
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

        let use_case = self.login_use_case.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            match use_case.execute(request).await {
                Ok(response) => {
                    let _ = tx.send(Action::LoginSuccess {
                        user: response.user,
                        token,
                        source: TokenSource::UserInput,
                    });
                }
                Err(e) => {
                    let _ = tx.send(Action::LoginFailure(e));
                }
            }
        });
    }

    fn handle_delete_token(&self) {
        let use_case = self.login_use_case.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            match use_case.delete_token().await {
                Ok(()) => {
                    let _ = tx.send(Action::LoginFailure(AuthError::unexpected(
                        "Token deleted from secure storage",
                    )));
                }
                Err(e) => {
                    let _ = tx.send(Action::LoginFailure(e));
                }
            }
        });
    }

    fn start_app_loading(&mut self, user: crate::domain::entities::User) {
        self.state = AppState::Initializing;
        self.screen = CurrentScreen::Splash(SplashScreen::new());
        self.gateway_ready = false;

        let (img_tx, img_rx) = mpsc::unbounded_channel();
        let action_tx = self.action_tx.clone();
        tokio::spawn(async move {
            match ImageLoader::with_defaults(img_tx).await {
                Ok(loader) => {
                    let _ = action_tx.send(Action::ImageLoaderReady(Arc::new(loader)));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to initialize image loader");
                }
            }
        });
        self.image_load_rx = Some(img_rx);

        let Some(ref token) = self.current_token else {
            return;
        };
        let token = token.clone();

        self.connect_gateway(&token);

        let state_store = self.state_store.clone();
        let command_tx = self.command_tx.clone();

        tokio::spawn(async move {
            let state: crate::infrastructure::state_store::AppState =
                state_store.load().await.unwrap_or_default();
            debug!(
                guild_id = ?state.last_guild_id,
                channel_id = ?state.last_channel_id,
                "Loaded persisted state"
            );

            let _ = command_tx.send(BackendCommand::LoadInitialData {
                token,
                user,
                initial_guild_id: state
                    .last_guild_id
                    .and_then(|id| id.parse::<u64>().ok())
                    .map(GuildId),
                initial_channel_id: state
                    .last_channel_id
                    .and_then(|id| id.parse::<u64>().ok())
                    .map(ChannelId),
            });
        });
    }

    fn set_connection_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
        if let CurrentScreen::Chat(ref mut state) = self.screen {
            state.set_connection_status(status);
        }
        if let Some(ref mut state) = self.pending_chat_state {
            state.set_connection_status(status);
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

        let mut client = GatewayClient::new(config, self.identity.clone());

        match client.connect(token.as_str()) {
            Ok(rx) => {
                info!("Gateway connection initiated");
                self.gateway_rx = Some(rx);
                self.gateway_client = Some(client);
                self.set_connection_status(ConnectionStatus::Connecting);
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
                self.set_connection_status(ConnectionStatus::Connected);
            }
            GatewayEventKind::Disconnected { reason, can_resume } => {
                warn!(reason = %reason, can_resume = can_resume, "Gateway disconnected");
                self.set_connection_status(ConnectionStatus::Disconnected);
            }
            GatewayEventKind::Reconnecting { attempt } => {
                info!(attempt = attempt, "Gateway reconnecting");
                self.set_connection_status(ConnectionStatus::Reconnecting);
            }
            GatewayEventKind::Resumed => {
                info!("Gateway session resumed");
                self.set_connection_status(ConnectionStatus::Connected);
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

    #[allow(clippy::too_many_lines)]
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
                channels,
            } => {
                if !unavailable {
                    info!(guild_id = %guild_id, name = %name, channel_count = channels.len(), "Guild available");
                    if let CurrentScreen::Chat(ref mut state) = self.screen {
                        state.set_channels(guild_id, channels);
                    }
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
                self.user_cache.update_username(&user_id, &username);
            }
            DispatchEvent::Ready {
                user_id,
                guilds,
                read_states,
                ..
            } => {
                info!(user_id = %user_id, guild_count = guilds.len(), "Gateway ready");
                self.gateway_ready = true;

                let read_states_map: std::collections::HashMap<_, _> = read_states
                    .iter()
                    .map(|rs| (rs.channel_id, rs.clone()))
                    .collect();

                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_read_states(read_states_map);
                } else {
                    if let Some(ref mut state) = self.pending_chat_state {
                        state.set_read_states(read_states_map);
                    }

                    self.pending_read_states = Some(read_states);

                    if self.pending_chat_state.is_some()
                        && let CurrentScreen::Splash(splash) = &mut self.screen
                    {
                        splash.set_data_ready();
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_message_create(&mut self, message: crate::domain::entities::Message) {
        let channel_id = message.channel_id();
        let user_id = message.author().id().to_string();
        debug!(message_id = %message.id(), channel_id = %channel_id, "New message received");

        self.cache_users_from_message(&message);

        if let CurrentScreen::Chat(ref mut state) = self.screen {
            state.on_message_received(&message);
            state.add_message(message);
        }

        self.typing_manager.remove_typing(channel_id, &user_id);
        self.update_typing_indicator(channel_id);
    }

    fn cache_users_from_message(&self, message: &crate::domain::entities::Message) {
        let author = message.author();
        self.user_cache
            .insert(crate::domain::entities::CachedUser::new(
                author.id(),
                author.username(),
                author.discriminator(),
                author.avatar().map(String::from),
                author.is_bot(),
            ));

        for mention in message.mentions() {
            self.user_cache.insert_from_user(mention);
        }
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
        if self.current_user_id.as_deref() == Some(user_id.as_str()) {
            return;
        }

        if let Some(ref name) = username {
            self.user_cache.insert_basic(&user_id, name);
        }

        let display_name = username
            .or_else(|| self.user_cache.get_display_name(&user_id))
            .or_else(|| {
                if let CurrentScreen::Chat(ref state) = self.screen {
                    state
                        .message_pane_data()
                        .get_author_name(&user_id)
                        .map(String::from)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| user_id.clone());

        debug!(user = %display_name, channel_id = %channel_id, "User started typing");

        self.typing_manager
            .add_typing(channel_id, user_id, display_name);
        self.update_typing_indicator(channel_id);
    }

    fn update_typing_indicator(&mut self, channel_id: ChannelId) {
        if let CurrentScreen::Chat(ref mut state) = self.screen {
            let current_channel = state.message_pane_data().channel_id();
            debug!(
                typing_channel = %channel_id,
                current_channel = ?current_channel,
                "Updating typing indicator"
            );

            if current_channel == Some(channel_id) {
                let indicator = self.typing_manager.format_typing_indicator(channel_id);
                debug!(indicator = ?indicator, "Setting typing indicator");
                state.set_typing_indicator(indicator);
            } else {
                debug!("Channel mismatch, not updating indicator");
            }
        } else {
            debug!("Not in chat screen, skipping typing indicator update");
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

    fn load_guild_channels(&mut self, guild_id: GuildId) {
        if let Some(ref token) = self.current_token {
            let _ = self.command_tx.send(BackendCommand::LoadGuildChannels {
                guild_id,
                token: token.clone(),
            });
        }
    }

    fn load_channel_messages(&mut self, channel_id: ChannelId) {
        self.typing_manager.clear_channel(channel_id);

        if let Some(ref token) = self.current_token {
            let _ = self.command_tx.send(BackendCommand::LoadChannelMessages {
                channel_id,
                token: token.clone(),
            });
        }
    }

    fn load_forum_threads(
        &mut self,
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        offset: u32,
    ) {
        self.typing_manager.clear_channel(channel_id);

        if let Some(ref token) = self.current_token {
            let _ = self.command_tx.send(BackendCommand::LoadForumThreads {
                channel_id,
                guild_id,
                token: token.clone(),
                offset,
            });
        }
    }

    /// Send a subscription to the gateway to receive typing events for a channel.
    /// This is required for user accounts to receive `TYPING_START` events.
    fn subscribe_to_channel(&mut self, guild_id: GuildId, channel_id: ChannelId) {
        if let Some(ref gateway_client) = self.gateway_client {
            debug!(
                guild_id = %guild_id,
                channel_id = %channel_id,
                "Subscribing to channel for typing events"
            );
            gateway_client.send_command(GatewayCommand::SubscribeChannel {
                guild_id: guild_id.as_u64().to_string(),
                channel_id: channel_id.as_u64().to_string(),
            });
        }
    }

    fn load_history(&mut self, channel_id: ChannelId, before_message_id: MessageId) {
        let Some(ref token) = self.current_token else {
            return;
        };

        let _ = self.command_tx.send(BackendCommand::LoadHistory {
            channel_id,
            before_message_id,
            token: token.clone(),
        });
    }

    #[allow(clippy::too_many_lines)]
    fn handle_action(&mut self, action: Action) {
        match action {
            Action::HistoryLoaded(messages) => {
                for message in &messages {
                    self.cache_users_from_message(message);
                }

                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.prepend_messages(messages);
                }
            }
            Action::LoadError(e) => {
                warn!(error = %e, "Failed to load history");
            }
            Action::DataLoaded {
                user,
                guilds,
                dms,
                read_states,
                initial_guild_id,
                initial_channel_id,
                initial_channels,
                initial_messages,
            } => {
                info!("Data loaded, preparing chat state");
                self.current_user_id = Some(user.id().to_string());
                let mut chat_state = ChatScreenState::new(
                    user,
                    self.markdown_service.clone(),
                    self.user_cache.clone(),
                    self.disable_user_colors,
                    self.theme,
                );

                chat_state.set_connection_status(self.connection_status);

                for dm in &dms {
                    self.user_cache
                        .insert_basic(&dm.recipient_id, &dm.recipient_name);
                }

                let dm_users: Vec<(String, String)> = dms
                    .into_iter()
                    .map(|dm| (dm.channel_id, dm.recipient_name))
                    .collect();
                chat_state.set_dm_users(dm_users);
                chat_state.set_guilds(guilds);

                let mut final_read_states = read_states;
                if let Some(pending) = self.pending_read_states.take() {
                    info!(count = pending.len(), "Applying pending read states");
                    for state in pending {
                        final_read_states.insert(state.channel_id, state);
                    }
                }
                chat_state.set_read_states(final_read_states);

                let restore_result = chat_state.restore_state(
                    initial_guild_id,
                    initial_channel_id,
                    initial_channels,
                    initial_messages,
                );

                self.pending_chat_state = Some(Box::new(chat_state));

                if let Some(result) = restore_result {
                    self.process_chat_key_result(result);
                }

                if self.gateway_ready
                    && let CurrentScreen::Splash(splash) = &mut self.screen
                {
                    splash.set_data_ready();
                }
            }
            Action::LoginSuccess {
                user,
                token,
                source,
            } => {
                info!(
                    user = %user.display_name(),
                    source = %source,
                    "Login successful"
                );
                if let Some(auth_token) = AuthToken::new(&token) {
                    self.current_token = Some(auth_token);
                }
                self.start_app_loading(user);
            }
            Action::LoginFailure(error) => {
                error!(error = %error, "Login failed");
                self.handle_login_error(&error);
            }
            Action::GuildChannelsLoaded { guild_id, channels } => {
                debug!(guild_id = %guild_id, count = channels.len(), "Loaded channels for guild");
                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.set_channels(guild_id, channels);
                }
            }
            Action::GuildChannelsLoadError { guild_id, error } => {
                warn!(guild_id = %guild_id, error = %error, "Failed to load channels for guild");
            }
            Action::ChannelMessagesLoaded {
                channel_id,
                messages,
            } => {
                if let CurrentScreen::Chat(state) = &mut self.screen
                    && state.message_pane_data().channel_id() == Some(channel_id)
                {
                    state.set_messages(messages);
                    state.set_typing_indicator(None);

                    if let Some(last_msg) = state
                        .message_pane_data()
                        .messages()
                        .back()
                        .map(|m| &m.message)
                    {
                        let message_id = last_msg.id();
                        if let Some(ref token) = self.current_token {
                            let _ = self.command_tx.send(BackendCommand::AcknowledgeMessage {
                                channel_id,
                                message_id,
                                token: token.clone(),
                            });
                        }
                        state.mark_channel_read(channel_id, message_id);
                    }
                }
            }
            Action::ChannelMessagesLoadError { channel_id, error } => {
                warn!(channel_id = %channel_id, error = %error, "Failed to load messages for channel");
                if let CurrentScreen::Chat(state) = &mut self.screen
                    && state.message_pane_data().channel_id() == Some(channel_id)
                {
                    state.set_message_error(error);
                    state.focus_guilds_tree();
                }
            }
            Action::ForumThreadsLoaded {
                channel_id,
                threads,
                offset,
            } => {
                if let CurrentScreen::Chat(state) = &mut self.screen
                    && state.message_pane_data().channel_id() == Some(channel_id)
                {
                    state.set_forum_threads(threads, offset);
                }
            }
            Action::ForumThreadsLoadError { channel_id, error } => {
                warn!(channel_id = %channel_id, error = %error, "Failed to load forum threads");
                if let CurrentScreen::Chat(state) = &mut self.screen
                    && state.message_pane_data().channel_id() == Some(channel_id)
                {
                    state.set_message_error(error);
                }
            }
            Action::MessageSent(message) => {
                info!(message_id = %message.id(), "Message sent successfully");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.add_message(message);
                }
            }
            Action::MessageSendError(error) => {
                error!(error = %error, "Failed to send message");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_error(format!("Failed to send: {error}"));
                }
            }
            Action::MessageEdited(message) => {
                info!(message_id = %message.id(), "Message edited successfully");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.update_message(message);
                }
            }
            Action::MessageEditError(error) => {
                error!(error = %error, "Failed to edit message");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_error(format!("Failed to edit: {error}"));
                }
            }
            Action::MessageDeleted(message_id) => {
                debug!(message_id = %message_id, "Message delete confirmed by backend");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.remove_message(message_id);
                }
            }
            Action::MessageDeleteError(error) => {
                error!(error = %error, "Failed to delete message");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_message_error(format!("Failed to delete: {error}"));
                }
            }
            Action::TypingIndicatorSent(_) => {}
            Action::ImageLoaderReady(loader) => {
                self.image_loader = Some(loader);
            }
            Action::PasteImageLoaded(path) => {
                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.add_attachment(path);
                    self.show_notification("Image pasted as attachment".to_string());
                }
            }
            Action::PasteTextLoaded(text) => {
                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.insert_text(&text);
                }
            }
        }
    }

    fn transition_to_login(&mut self) {
        self.disconnect_gateway();
        self.state = AppState::Login;
        self.current_token = None;
        self.current_user_id = None;
        self.pending_chat_state = None;
        self.typing_manager = TypingIndicatorManager::new();
        self.user_cache.clear();
        self.screen = CurrentScreen::Login(LoginScreen::new().with_theme(self.theme));
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

    fn handle_reply_to_message(
        &mut self,
        message_id: crate::domain::entities::MessageId,
        mention: bool,
    ) {
        if let CurrentScreen::Chat(ref mut state) = self.screen
            && let Some(author_name) = state.get_reply_author(message_id)
        {
            state.start_reply(message_id, author_name, mention);
        }
    }

    fn handle_send_message(
        &mut self,
        content: String,
        reply_to: Option<crate::domain::entities::MessageId>,
        attachments: Vec<std::path::PathBuf>,
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

        let mut request = if let Some(reply_id) = reply_to {
            SendMessageRequest::new(channel_id, content).with_reply(reply_id)
        } else {
            SendMessageRequest::new(channel_id, content)
        };
        request = request.with_attachments(attachments);

        debug!(channel_id = %channel_id, has_reply = reply_to.is_some(), "Sending message");

        let _ = self.command_tx.send(BackendCommand::SendMessage {
            token: token.clone(),
            request,
        });

        self.last_typing_sent = None;
    }

    fn handle_edit_message(
        &mut self,
        message_id: crate::domain::entities::MessageId,
        content: String,
    ) {
        let channel_id = if let CurrentScreen::Chat(ref state) = self.screen {
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id)
        } else {
            None
        };

        let Some(channel_id) = channel_id else {
            warn!("Cannot edit message: no channel selected");
            return;
        };

        let Some(ref token) = self.current_token else {
            warn!("Cannot edit message: no token available");
            return;
        };

        let request = EditMessageRequest::new(channel_id, message_id, content);

        debug!(channel_id = %channel_id, message_id = %message_id, "Editing message");

        let _ = self.command_tx.send(BackendCommand::EditMessage {
            token: token.clone(),
            request,
        });
    }

    fn handle_delete_message(&mut self, message_id: crate::domain::entities::MessageId) {
        let channel_id = if let CurrentScreen::Chat(ref state) = self.screen {
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id)
        } else {
            None
        };

        let Some(channel_id) = channel_id else {
            warn!("Cannot delete message: no channel selected");
            return;
        };

        let Some(ref token) = self.current_token else {
            warn!("Cannot delete message: no token available");
            return;
        };

        debug!(channel_id = %channel_id, message_id = %message_id, "Deleting message");

        let _ = self.command_tx.send(BackendCommand::DeleteMessage {
            token: token.clone(),
            channel_id,
            message_id,
        });
    }

    fn handle_start_typing(&mut self) {
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

        let _ = self.command_tx.send(BackendCommand::SendTypingIndicator {
            channel_id,
            token: token.clone(),
        });

        self.last_typing_sent = Some((channel_id, Instant::now()));
    }

    /// Handle an image load completion event.
    fn handle_image_loaded(&mut self, event: ImageLoadedEvent) {
        if let CurrentScreen::Chat(ref mut state) = self.screen {
            match event.result {
                Ok(loaded) => {
                    debug!(id = %loaded.id, source = ?loaded.source, "Image loaded successfully");
                    state.on_image_loaded(&loaded.id, &loaded.image);
                }
                Err(e) => {
                    warn!(id = %event.id, error = %e, "Failed to load image");
                    state.mark_image_failed(&event.id, &e);
                }
            }
        }
    }

    /// Trigger loading of images in the visible viewport.
    fn trigger_image_loads(&mut self) {
        let CurrentScreen::Chat(ref mut state) = self.screen else {
            return;
        };

        let Some(ref loader) = self.image_loader else {
            return;
        };

        let needed = state.collect_needed_image_loads();
        for (id, url) in needed {
            state.mark_image_downloading(&id);
            loader.load_async(id, url);
        }
    }

    fn handle_open_editor(
        &mut self,
        terminal: &mut DefaultTerminal,
        initial_content: &str,
        target_message_id: Option<MessageId>,
    ) -> color_eyre::Result<()> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new()?;
        write!(temp_file, "{initial_content}")?;
        let temp_path = temp_file.path().to_owned();

        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| {
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

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;

        let status = std::process::Command::new(&editor).arg(&temp_path).status();

        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide
        )?;

        terminal.clear()?;

        match status {
            Ok(exit_status) if exit_status.success() => {
                let new_content = match std::fs::read_to_string(&temp_path) {
                    Ok(content) => content,
                    Err(e) => {
                        error!(error = %e, "Failed to read from temp file after edit");
                        if let CurrentScreen::Chat(ref mut state) = self.screen {
                            state.set_message_error(format!("Failed to read editor output: {e}"));
                        }
                        if let Err(e) = std::fs::remove_file(&temp_path) {
                            debug!(error = %e, "Failed to remove temp file");
                        }
                        return Ok(());
                    }
                };

                let new_content = if new_content.ends_with('\n') && !initial_content.ends_with('\n')
                {
                    new_content[..new_content.len() - 1].to_string()
                } else {
                    new_content
                };

                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    if let Some(id) = target_message_id {
                        state.message_input_parts_mut().start_edit(id, &new_content);
                        state.focus_message_input();
                    } else {
                        state.message_input_parts_mut().set_content(&new_content);
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
        async fn delete_message(
            &self,
            _token: &AuthToken,
            _channel_id: ChannelId,
            _message_id: MessageId,
        ) -> Result<(), AuthError> {
            Ok(())
        }

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

        async fn fetch_read_states(
            &self,
            _token: &AuthToken,
        ) -> Result<Vec<crate::domain::entities::ReadState>, AuthError> {
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

        async fn edit_message(
            &self,
            _token: &AuthToken,
            _request: EditMessageRequest,
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

        async fn acknowledge_message(
            &self,
            _token: &AuthToken,
            _channel_id: ChannelId,
            _message_id: MessageId,
        ) -> Result<(), AuthError> {
            Ok(())
        }

        async fn fetch_forum_threads(
            &self,
            _token: &AuthToken,
            _channel_id: ChannelId,
            _guild_id: Option<GuildId>,
            _offset: u32,
            _limit: Option<u8>,
        ) -> Result<Vec<crate::domain::entities::ForumThread>, AuthError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_app_creation() {
        let auth = Arc::new(MockAuthPort::new(true));
        let data = Arc::new(MockDiscordData);
        let storage = Arc::new(MockTokenStorage::new());
        let theme = Theme::new("Orange");
        let identity = Arc::new(ClientIdentity::new());
        let app = App::new(auth, data, storage, false, theme, identity);

        assert_eq!(app.state, AppState::Login);
    }
}
