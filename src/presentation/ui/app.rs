//! Main application orchestrator.

use crate::domain::keybinding::Action as KeyAction;
use crate::presentation::commands::CommandRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use futures_util::StreamExt;
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use zeroize::Zeroize;

use crate::application::dto::{LoginRequest, TokenSource};
use crate::application::services::notification_manager::NotificationManager;
use crate::application::services::notification_service::NotificationService;
use crate::application::use_cases::{LoginUseCase, ResolveTokenUseCase};
use crate::domain::ConnectionStatus;
use crate::domain::entities::{
    AuthToken, Channel, ChannelId, GuildFolder, GuildId, MessageId, RelationshipState, UserCache,
};
use crate::domain::errors::AuthError;
use crate::domain::ports::{
    AuthPort, DiscordDataPort, EditMessageRequest, SendMessageRequest, TokenStoragePort,
};
use crate::infrastructure::config::app_config::QuickSwitcherSortMode;
use crate::infrastructure::discord::{
    DispatchEvent, GatewayClient, GatewayClientConfig, GatewayCommand, GatewayEventKind,
    GatewayIntents, TypingIndicatorManager, identity::ClientIdentity,
};
use crate::infrastructure::image::{ImageLoadedEvent, ImageLoader};
use crate::infrastructure::notifications::DesktopNotificationService;
use crate::infrastructure::{ClipboardService, StateStore};
use crate::presentation::events::EventResult;
use crate::presentation::services::markdown_renderer::MarkdownRenderer;
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
    Login(Box<LoginScreen>),
    Splash(SplashScreen),
    Chat(Box<ChatScreenState>),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub disable_user_colors: bool,
    pub group_guilds: bool,
    pub desktop_notifications: bool,
    pub use_display_name: bool,
    pub image_preview: bool,
    pub timestamp_format: String,
    pub show_typing: bool,
    pub internal_notifications: bool,
    pub notification_duration: u64,
    pub enable_animations: bool,
    pub editor: Option<String>,
    pub keybindings: HashMap<String, KeyAction>,
    pub theme: Theme,
    pub hide_blocked_completely: bool,
    pub quick_switcher_order: QuickSwitcherSortMode,
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    state: AppState,
    screen: CurrentScreen,
    login_use_case: LoginUseCase,
    resolve_token_use_case: ResolveTokenUseCase,
    command_tx: mpsc::UnboundedSender<BackendCommand>,
    pending_token: Option<(String, TokenSource)>,
    current_token: Option<AuthToken>,
    token_source: Option<TokenSource>,
    gateway_client: Option<GatewayClient>,
    gateway_rx: Option<mpsc::UnboundedReceiver<GatewayEventKind>>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
    typing_manager: TypingIndicatorManager,
    last_typing_cleanup: Instant,
    last_typing_sent: Option<(ChannelId, Instant)>,
    markdown_service: Arc<MarkdownRenderer>,
    user_cache: UserCache,
    current_user_id: Option<String>,
    pending_chat_state: Option<Box<ChatScreenState>>,
    pending_read_states: Option<Vec<crate::domain::entities::ReadState>>,
    pending_guild_folders: Option<Vec<GuildFolder>>,
    pending_channels: std::collections::HashMap<GuildId, Vec<Channel>>,
    pending_roles: std::collections::HashMap<GuildId, Vec<crate::domain::entities::Role>>,
    pending_members: std::collections::HashMap<GuildId, Vec<crate::domain::entities::Member>>,
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
    group_guilds: bool,
    use_display_name: bool,
    image_preview: bool,
    timestamp_format: String,
    show_typing: bool,
    internal_notifications: bool,
    enable_animations: bool,
    editor: Option<String>,
    command_registry: CommandRegistry,
    theme: Theme,
    identity: Arc<ClientIdentity>,
    state_store: StateStore,
    #[allow(clippy::type_complexity)]
    state_save_tx: mpsc::UnboundedSender<(
        Option<GuildId>,
        Option<ChannelId>,
        Vec<crate::domain::search::RecentItem>,
        QuickSwitcherSortMode,
    )>,
    clipboard_service: ClipboardService,
    notification_manager: NotificationManager,
    notification_service: NotificationService,
    last_desktop_notification: Option<Instant>,
    relationship_state: RelationshipState,
    hide_blocked_completely: bool,
    pub quick_switcher_order: QuickSwitcherSortMode,
}

impl App {
    /// # Panics
    ///
    /// Panics if the application state persistence cannot be initialized.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(
        auth_port: Arc<dyn AuthPort>,
        discord_data: Arc<dyn DiscordDataPort>,
        storage_port: Arc<dyn TokenStoragePort>,
        config: AppConfig,
        identity: Arc<ClientIdentity>,
    ) -> Self {
        let login_use_case = LoginUseCase::new(auth_port, storage_port.clone());
        let resolve_token_use_case = ResolveTokenUseCase::new(storage_port);
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let markdown_service = Arc::new(MarkdownRenderer::new());
        let notification_port = Arc::new(DesktopNotificationService::new(
            config.desktop_notifications,
        ));
        let notification_service = NotificationService::new(notification_port);

        let backend = Backend::new(discord_data, command_rx, action_tx.clone());
        tokio::spawn(backend.run());

        let state_store = StateStore::new();
        let (state_save_tx, mut state_save_rx) = mpsc::unbounded_channel::<(
            Option<GuildId>,
            Option<ChannelId>,
            Vec<crate::domain::search::RecentItem>,
            QuickSwitcherSortMode,
        )>();
        let store = state_store.clone();

        let mut command_registry = CommandRegistry::new();
        command_registry.apply_overrides(&config.keybindings);

        tokio::spawn(async move {
            const DEBOUNCE_DURATION: Duration = Duration::from_secs(1);
            type PendingState = (
                Option<GuildId>,
                Option<ChannelId>,
                Vec<crate::domain::search::RecentItem>,
                QuickSwitcherSortMode,
            );
            let mut pending_state: Option<PendingState> = None;
            let mut timer = Box::pin(tokio::time::sleep(Duration::MAX));

            loop {
                tokio::select! {
                    Some(state) = state_save_rx.recv() => {
                        pending_state = Some(state);
                        timer = Box::pin(tokio::time::sleep(DEBOUNCE_DURATION));
                    }
                    () = &mut timer, if pending_state.is_some() => {
                        if let Some((guild_id, channel_id, recents, sort_mode)) = pending_state.take() {
                            let gid = guild_id.map(|g| g.as_u64().to_string());
                            let cid = channel_id.map(|c| c.as_u64().to_string());
                            if let Err(e) = store.save(gid, cid, &recents, sort_mode).await {
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
            screen: CurrentScreen::Login(Box::new(LoginScreen::new().with_theme(config.theme))),
            login_use_case,
            resolve_token_use_case,
            command_tx,
            pending_token: None,
            current_token: None,
            token_source: None,
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
            pending_guild_folders: None,
            pending_channels: std::collections::HashMap::new(),
            pending_roles: std::collections::HashMap::new(),
            pending_members: std::collections::HashMap::new(),
            gateway_ready: false,
            connection_status: ConnectionStatus::Disconnected,
            should_render: true,
            image_loader: None,
            image_load_rx: None,
            last_image_check: Instant::now(),
            disable_user_colors: config.disable_user_colors,
            group_guilds: config.group_guilds,
            use_display_name: config.use_display_name,
            image_preview: config.image_preview,
            timestamp_format: config.timestamp_format,
            show_typing: config.show_typing,
            internal_notifications: config.internal_notifications,
            enable_animations: config.enable_animations,
            editor: config.editor,
            command_registry,
            theme: config.theme,
            identity,
            state_store,
            state_save_tx,
            clipboard_service: ClipboardService::new(),
            notification_manager: NotificationManager::new(Duration::from_secs(
                config.notification_duration,
            )),
            notification_service,
            last_desktop_notification: None,
            relationship_state: RelationshipState::new(),
            hide_blocked_completely: config.hide_blocked_completely,
            quick_switcher_order: config.quick_switcher_order,
        }
    }

    /// # Errors
    /// Returns error if terminal or token resolution fails.
    pub async fn run(
        mut self,
        terminal: &mut DefaultTerminal,
        external_token: Option<(String, TokenSource)>,
    ) -> color_eyre::Result<()> {
        if let Some(resolved) = self.resolve_token_use_case.execute(external_token).await? {
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

    async fn validate_auth_status(&self) -> bool {
        match self.token_source {
            Some(TokenSource::Keyring) => {
                matches!(self.resolve_token_use_case.execute(None).await, Ok(Some(_)))
            }
            Some(TokenSource::Environment | TokenSource::UserInput) => true,
            None => false,
        }
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
                             if self.validate_auth_status().await {
                                 self.state = AppState::Chat;
                                 self.screen = CurrentScreen::Chat(self.pending_chat_state.take().unwrap());
                                 self.should_render = true;
                             } else {
                                 warn!("Token verification failed after splash");
                                 self.transition_to_login();
                                 self.should_render = true;
                             }
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

                    if self.notification_manager.has_notifications() {
                        self.should_render = true;
                    }
                }

                Some(Ok(event)) = terminal_event => {
                    let is_release_event = matches!(
                        event,
                        crossterm::event::Event::Key(key)
                        if key.kind == crossterm::event::KeyEventKind::Release
                    );

                    if !is_release_event {
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
            Event::Paste(text) => self.handle_paste(text),
            _ => EventResult::Continue,
        }
    }

    fn handle_paste(&mut self, text: &str) -> EventResult {
        let clean_text = text.trim_end_matches(['\n', '\r']);

        match &mut self.screen {
            CurrentScreen::Login(screen) => {
                screen.paste_token(clean_text);
                EventResult::Continue
            }
            CurrentScreen::Chat(state) => {
                state.insert_text(clean_text);
                EventResult::Continue
            }
            CurrentScreen::Splash(_) => EventResult::Continue,
        }
    }

    fn save_state(&self, guild_id: Option<GuildId>, channel_id: Option<ChannelId>) {
        let (recents, sort_mode) = if let CurrentScreen::Chat(state) = &self.screen {
            (state.recents.clone(), state.quick_switcher_sort_mode())
        } else {
            (Vec::new(), QuickSwitcherSortMode::default())
        };
        let _ = self
            .state_save_tx
            .send((guild_id, channel_id, recents, sort_mode));
    }

    async fn attempt_auto_login(&mut self, token: String, source: TokenSource) {
        debug!("Attempting automatic login");

        if let CurrentScreen::Login(ref mut login_screen) = self.screen {
            login_screen.set_validating();
        }

        let mut request = LoginRequest::new(token.clone(), source);
        if source == TokenSource::Environment {
            request = request.without_persistence();
        }

        match self.login_use_case.execute(request).await {
            Ok(response) => {
                info!(user = %response.user.display_name(), "Auto-login successful");
                if let Some(auth_token) = AuthToken::new(&token) {
                    self.current_token = Some(auth_token);
                }
                self.token_source = Some(source);
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
        self.notification_manager.info("Info", message);
    }

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            CurrentScreen::Login(screen) => {
                frame.render_widget(&**screen, frame.area());
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

        self.notification_manager.tick();
        if let Some(notification) = self.notification_manager.current_notification() {
            use crate::presentation::ui::notification_popup::NotificationPopup;
            frame.render_widget(
                NotificationPopup::new(notification, &self.theme),
                frame.area(),
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_key(&mut self, key: KeyEvent) -> EventResult {
        if self.state == AppState::Login {
            let is_force_quit = matches!(
                key,
                KeyEvent {
                    code: KeyCode::Esc,
                    ..
                } | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: crossterm::event::KeyModifiers::CONTROL,
                    ..
                }
            );

            if is_force_quit {
                return EventResult::Exit;
            }
        } else if let KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: crossterm::event::KeyModifiers::CONTROL,
            ..
        } = key
        {
            return EventResult::Exit;
        }

        let result = match &mut self.screen {
            CurrentScreen::Login(screen) => {
                match screen.handle_key(key) {
                    LoginAction::Submit => self.handle_login_submit(),
                    LoginAction::DeleteToken => self.handle_delete_token(),
                    LoginAction::Paste => self.handle_login_paste(),
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
            ChatKeyResult::SecureLogout => {
                self.handle_delete_token();
                self.transition_to_login();
            }
            ChatKeyResult::CopyToClipboard(text) => {
                debug!(text = %text, "Copy to clipboard requested");
                self.clipboard_service.set_text(text);
                self.show_notification("Copied to clipboard".to_string());
            }
            ChatKeyResult::CopyImageToClipboard(image_id) => {
                debug!(image_id = %image_id, "Copy image to clipboard requested");
                if let CurrentScreen::Chat(state) = &self.screen {
                    let mut found = false;
                    for msg in state.message_pane_data().messages() {
                        if let Some(attachment) =
                            msg.image_attachments.iter().find(|a| a.id == image_id)
                        {
                            if let Some(image) = &attachment.image {
                                let image = image.clone();
                                let clipboard = self.clipboard_service.clone();
                                let tx = self.action_tx.clone();

                                tokio::task::spawn_blocking(move || {
                                    let rgba = image.to_rgba8();
                                    let width = rgba.width() as usize;
                                    let height = rgba.height() as usize;
                                    let bytes = std::borrow::Cow::Owned(rgba.into_raw());

                                    let image_data = arboard::ImageData {
                                        width,
                                        height,
                                        bytes,
                                    };

                                    clipboard.set_image(image_data);
                                    let _ = tx.send(Action::ShowNotification(
                                        "Copied image to clipboard".to_string(),
                                    ));
                                });
                                found = true;
                            } else {
                                self.show_notification("Image not loaded yet".to_string());
                                found = true;
                            }
                            break;
                        }
                    }
                    if !found {
                        self.show_notification("Image not found".to_string());
                    }
                }
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
                if let CurrentScreen::Chat(state) = &self.screen
                    && let Some(ui_msg) = state
                        .message_pane_data()
                        .messages()
                        .iter()
                        .find(|m| m.message.id() == message_id)
                {
                    let mut urls = std::collections::HashSet::new();
                    for img in &ui_msg.image_attachments {
                        urls.insert(img.url.clone());
                    }

                    if let Some(loader) = &self.image_loader {
                        for url in urls {
                            let loader = loader.clone();
                            let image_id = crate::domain::entities::ImageId::from_url(&url);
                            let url_clone = url.clone();

                            tokio::spawn(async move {
                                match loader.export_for_viewing(&image_id, &url_clone).await {
                                    Ok(path) => {
                                        tokio::task::spawn_blocking(move || {
                                            if let Err(e) = opener::open(&path) {
                                                tracing::error!("Failed to open image file: {}", e);
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to export image for viewing: {}",
                                            e
                                        );
                                    }
                                }
                            });
                        }
                    }
                }
            }
            ChatKeyResult::OpenLink(url) => {
                debug!(url = %url, "Open link requested");
                tokio::task::spawn_blocking(move || {
                    if let Err(e) = opener::open(&url) {
                        tracing::error!("Failed to open link: {}", e);
                    }
                });
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
                            image.bytes.into_owned(),
                        ) && img.save(&path).is_ok()
                        {
                            let _ = tx.send(Action::PasteImageLoaded(path));
                            return;
                        }
                    }

                    if let Some(text) = clipboard.get_text() {
                        let _ = tx.send(Action::PasteTextLoaded(text));
                    }
                });
            }
            ChatKeyResult::ToggleHelp | ChatKeyResult::Consumed | ChatKeyResult::Ignored => {}
            ChatKeyResult::ToggleDisplayName => {
                self.use_display_name = !self.use_display_name;
                info!(
                    enabled = self.use_display_name,
                    "Toggled display name preference"
                );
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_use_display_name(self.use_display_name);

                    if let Some(channel_id) = state.message_pane_data().channel_id() {
                        let indicator = self.typing_manager.format_typing_indicator(channel_id);
                        state.set_typing_indicator(indicator);
                    }
                } else if let Some(ref mut state) = self.pending_chat_state {
                    state.set_use_display_name(self.use_display_name);
                }

                let status = if self.use_display_name {
                    "enabled"
                } else {
                    "disabled"
                };
                self.show_notification(format!("Display names {status}"));
            }
            ChatKeyResult::JumpToChannel(channel_id) => {
                debug!(channel_id = %channel_id, "Jump to channel requested");
                self.save_state(None, Some(channel_id));
                self.load_channel_messages(channel_id);

                if let CurrentScreen::Chat(state) = &mut self.screen {
                    let name = state.get_channel(channel_id).map_or_else(
                        || format!("{channel_id}"),
                        crate::domain::entities::Channel::display_name,
                    );

                    state.message_pane_data_mut().set_channel(channel_id, name);
                    state.message_pane_parts_mut().1.on_channel_change();
                    state.message_input_parts_mut().set_has_channel(true);
                    state.message_input_parts_mut().clear();
                    state.focus_messages_list();
                }
            }
            ChatKeyResult::RequestChannelFetch(ids) => {
                if let Some(ref token) = self.current_token {
                    for id in ids {
                        let _ = self.command_tx.send(BackendCommand::FetchChannel {
                            channel_id: id,
                            token: token.clone(),
                        });
                    }
                }
            }
            ChatKeyResult::ShowNotification(message) => {
                self.show_notification(message);
            }
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

    fn handle_login_paste(&mut self) {
        let clipboard = self.clipboard_service.clone();
        let tx = self.action_tx.clone();

        tokio::task::spawn_blocking(move || {
            if let Some(text) = clipboard.get_text() {
                let _ = tx.send(Action::PasteTextLoaded(text));
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
                recents: state.recents,
                sort_mode: state.quick_switcher_order,
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
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.remove_channel(channel_id);
                } else if let Some(ref mut state) = self.pending_chat_state {
                    state.remove_channel(channel_id);
                }
            }
            DispatchEvent::GuildCreate {
                guild_id,
                name,
                unavailable,
                channels,
                mut threads,
                roles,
                members,
            } => {
                if !unavailable {
                    info!(guild_id = %guild_id, name = %name, channel_count = channels.len(), thread_count = threads.len(), "Guild available");
                    let mut all_channels = channels;
                    all_channels.append(&mut threads);

                    if let CurrentScreen::Chat(ref mut state) = self.screen {
                        state.set_guild_data(guild_id, roles, members);
                        state.set_channels(guild_id, all_channels);
                    } else if let Some(ref mut state) = self.pending_chat_state {
                        state.set_guild_data(guild_id, roles, members);
                        state.set_channels(guild_id, all_channels);
                    } else {
                        self.pending_roles.insert(guild_id, roles);
                        self.pending_members.insert(guild_id, members);
                        self.pending_channels.insert(guild_id, all_channels);
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
                    if let CurrentScreen::Chat(ref mut state) = self.screen {
                        state.remove_guild(guild_id);
                    } else if let Some(ref mut state) = self.pending_chat_state {
                        state.remove_guild(guild_id);
                    } else {
                        self.pending_roles.remove(&guild_id);
                        self.pending_members.remove(&guild_id);
                        self.pending_channels.remove(&guild_id);
                    }
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
                mut initial_guild_channels,
                mut initial_guild_roles,
                mut initial_guild_members,
                read_states,
                guild_folders,
                relationships,
                ..
            } => {
                info!(user_id = %user_id, guild_count = guilds.len(), read_states_count = read_states.len(), relationship_count = relationships.len(), "Gateway ready");
                self.gateway_ready = true;

                self.relationship_state
                    .initialize_from_relationships(&relationships);
                debug!(
                    blocked_count = self.relationship_state.blocked_count(),
                    "Initialized relationship state"
                );

                let read_states_map: std::collections::HashMap<_, _> = read_states
                    .iter()
                    .map(|rs| (rs.channel_id, rs.clone()))
                    .collect();

                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_read_states(read_states_map);
                    state.set_guild_folders(guild_folders);
                    for (guild_id, channels) in initial_guild_channels.drain() {
                        let roles = initial_guild_roles.remove(&guild_id).unwrap_or_default();
                        let members = initial_guild_members.remove(&guild_id).unwrap_or_default();
                        state.set_guild_data(guild_id, roles, members);
                        state.set_channels(guild_id, channels);
                    }
                } else {
                    if let Some(ref mut state) = self.pending_chat_state {
                        state.set_read_states(read_states_map);
                        state.set_guild_folders(guild_folders.clone());
                        for (guild_id, channels) in initial_guild_channels.drain() {
                            let roles = initial_guild_roles.remove(&guild_id).unwrap_or_default();
                            let members =
                                initial_guild_members.remove(&guild_id).unwrap_or_default();
                            state.set_guild_data(guild_id, roles, members);
                            state.set_channels(guild_id, channels);
                        }
                    } else {
                        for (guild_id, channels) in initial_guild_channels.drain() {
                            let roles = initial_guild_roles.remove(&guild_id).unwrap_or_default();
                            let members =
                                initial_guild_members.remove(&guild_id).unwrap_or_default();

                            self.pending_roles.insert(guild_id, roles);
                            self.pending_members.insert(guild_id, members);
                            self.pending_channels.insert(guild_id, channels);
                        }
                    }

                    self.pending_read_states = Some(read_states);
                    self.pending_guild_folders = Some(guild_folders);

                    if self.pending_chat_state.is_some()
                        && let CurrentScreen::Splash(splash) = &mut self.screen
                    {
                        splash.set_data_ready();
                    }
                }
            }
            DispatchEvent::UserSettingsUpdate { guild_folders } => {
                debug!("Received guild folders update");
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.set_guild_folders(guild_folders);
                } else if let Some(ref mut state) = self.pending_chat_state {
                    state.set_guild_folders(guild_folders);
                }
            }
            DispatchEvent::RelationshipAdd {
                user_id,
                relationship_type,
            } => {
                self.relationship_state
                    .update_relationship(user_id, relationship_type);
                self.should_render = true;
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.mark_messages_dirty();
                }
            }
            DispatchEvent::RelationshipRemove { user_id } => {
                self.relationship_state.unblock_user(user_id);
                self.should_render = true;
                if let CurrentScreen::Chat(ref mut state) = self.screen {
                    state.mark_messages_dirty();
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_message_create(&mut self, message: crate::domain::entities::Message) {
        let channel_id = message.channel_id();
        let user_id = message.author().id().to_string();
        debug!(message_id = %message.id(), channel_id = %channel_id, "New message received");

        self.cache_users_from_message(&message);

        let mut is_mentioned = false;
        if let Some(current_user_id) = &self.current_user_id
            && message
                .mentions()
                .iter()
                .any(|u| u.id().to_string() == *current_user_id)
        {
            is_mentioned = true;
        }

        let mut is_dm = false;
        if let CurrentScreen::Chat(ref state) = self.screen
            && state
                .guilds_tree_data()
                .dm_users()
                .iter()
                .any(|dm| dm.channel_id == channel_id.to_string())
        {
            is_dm = true;
        }

        if is_dm && Some(&user_id) != self.current_user_id.as_ref() {
            is_mentioned = true;
        }

        if is_mentioned {
            if let CurrentScreen::Chat(ref mut state) = self.screen {
                state.increment_mention_count(channel_id);
            } else if let Some(ref mut state) = self.pending_chat_state {
                state.increment_mention_count(channel_id);
            }
        }

        let is_focused = if let CurrentScreen::Chat(ref state) = self.screen {
            state.message_pane_data().channel_id() == Some(channel_id)
        } else {
            false
        };

        if is_mentioned && !is_focused {
            let content = message.content();
            let mut body = if content.is_empty() {
                if !message.attachments().is_empty() {
                    "[Attachment]".to_string()
                } else if !message.embeds().is_empty() {
                    "[Embed]".to_string()
                } else {
                    "Sent a message".to_string()
                }
            } else {
                content.chars().take(500).collect()
            };

            for mention in message.mentions() {
                let id_pattern = format!("<@{}>", mention.id().as_u64());
                let id_pattern_bang = format!("<@!{}>", mention.id().as_u64());
                let name = if self.use_display_name {
                    self.user_cache
                        .get_display_name(&mention.id_str())
                        .unwrap_or_else(|| mention.username().to_string())
                } else {
                    mention.username().to_string()
                };
                let replacement = format!("@{name}");
                body = body.replace(&id_pattern, &replacement);
                body = body.replace(&id_pattern_bang, &replacement);
            }

            if body.chars().count() > 200 {
                body = body.chars().take(200).collect();
                body.push_str("...");
            }

            if self.internal_notifications {
                let title = if self.use_display_name {
                    message.author().raw_display_name()
                } else {
                    message.author().username().to_string()
                };

                self.notification_manager.info(format!("@{title}"), &body);
            }

            let now = Instant::now();
            let should_notify = match self.last_desktop_notification {
                Some(last) => now.duration_since(last) > Duration::from_secs(2),
                None => true,
            };

            if should_notify {
                let title = format!("Oxicord - @{}", message.author().username());
                self.notification_service.send(&title, &body);
                self.last_desktop_notification = Some(now);
            }
        }

        if let CurrentScreen::Chat(ref mut state) = self.screen {
            state.on_message_received(&message);
            if let Some(result) = state.add_message(message) {
                let _ = self.process_chat_key_result(result);
            }
        } else if let Some(ref mut state) = self.pending_chat_state {
            state.on_message_received(&message);
            if let Some(result) = state.add_message(message) {
                let _ = result;
            }
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
                author.global_name.clone(),
                author.is_bot(),
            ));

        for mention in message.mentions() {
            self.user_cache.insert_from_user(mention);
        }
    }

    fn handle_message_update(&mut self, message: crate::domain::entities::Message) {
        debug!(message_id = %message.id(), "Message updated");
        if let CurrentScreen::Chat(ref mut state) = self.screen
            && let Some(result) = state.update_message(message)
        {
            let _ = self.process_chat_key_result(result);
        }
    }

    fn handle_typing_start(
        &mut self,
        channel_id: ChannelId,
        user_id: String,
        username: Option<String>,
    ) {
        if !self.show_typing || self.current_user_id.as_deref() == Some(user_id.as_str()) {
            return;
        }

        if let Some(ref name) = username {
            if self.user_cache.contains(&user_id) {
                self.user_cache.update_username(&user_id, name);
            } else {
                self.user_cache.insert_basic(&user_id, name);
            }
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
        if !self.show_typing {
            return;
        }

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
            Action::ChannelLoaded(channel) => {
                let id = channel.id().to_string();
                let icon = if channel.kind().is_thread() {
                    "󰙯 "
                } else if channel.kind() == crate::domain::entities::ChannelKind::Forum {
                    "󰠄 "
                } else {
                    "#"
                };
                let name = format!("{}{}", icon, channel.name());

                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.message_pane_data_mut().register_channel(id, name);
                    state.message_pane_data_mut().force_refresh_layout();
                }
            }
            Action::ChannelLoadError { channel_id, error } => {
                warn!(channel_id = %channel_id, error = %error, "Failed to load channel info");
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
                recents,
                sort_mode,
            } => {
                info!("Data loaded, preparing chat state");
                self.current_user_id = Some(user.id().to_string());
                let mut chat_state = ChatScreenState::new(
                    user,
                    self.markdown_service.clone(),
                    self.user_cache.clone(),
                    self.disable_user_colors,
                    self.use_display_name,
                    self.image_preview,
                    self.timestamp_format.clone(),
                    self.theme,
                    self.enable_animations,
                    self.command_registry.clone(),
                    self.relationship_state.clone(),
                    self.hide_blocked_completely,
                    sort_mode,
                    recents,
                );

                chat_state.set_connection_status(self.connection_status);

                for dm in &dms {
                    self.user_cache
                        .insert(crate::domain::entities::CachedUser::new(
                            &dm.recipient_id,
                            &dm.recipient_username,
                            &dm.recipient_discriminator,
                            None,
                            dm.recipient_global_name.clone(),
                            false,
                        ));
                }

                chat_state.set_dm_users(dms);
                chat_state.set_guilds(guilds);

                if let Some(folders) = self.pending_guild_folders.take() {
                    chat_state.set_guild_folders(folders);
                }
                chat_state.set_group_guilds(self.group_guilds);

                for (guild_id, channels) in self.pending_channels.drain() {
                    chat_state.set_channels(guild_id, channels);
                }

                let mut final_read_states = read_states;

                if let Some(pending) = self.pending_read_states.take() {
                    info!(
                        count = pending.len(),
                        "Applying pending read states from Gateway"
                    );
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
                self.token_source = Some(source);
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
                } else if let Some(ref mut state) = self.pending_chat_state {
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
                let _ = self.handle_terminal_event(&Event::Paste(text));
            }
            Action::ShowNotification(message) => {
                self.show_notification(message);
            }
            Action::SearchResults(results) => {
                if let CurrentScreen::Chat(state) = &mut self.screen {
                    state.set_quick_switcher_results(results);
                }
            }
        }
    }

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

    fn transition_to_login(&mut self) {
        self.state = AppState::Login;
        self.screen = CurrentScreen::Login(Box::new(LoginScreen::new().with_theme(self.theme)));
        self.current_token = None;
        self.gateway_ready = false;
        self.disconnect_gateway();

        self.token_source = None;
        self.current_user_id = None;
        self.pending_chat_state = None;
        self.typing_manager = TypingIndicatorManager::new();
        self.user_cache.clear();

        if let Some((mut token, _)) = self.pending_token.take() {
            token.zeroize();
        }
    }

    fn handle_reply_to_message(&mut self, message_id: MessageId, mention: bool) {
        if let CurrentScreen::Chat(state) = &mut self.screen
            && let Some(author) = state.get_reply_author(message_id)
        {
            state.start_reply(message_id, author, mention);
        }
    }

    fn handle_edit_message(&mut self, message_id: MessageId, content: String) {
        if let Some(ref token) = self.current_token
            && let CurrentScreen::Chat(state) = &self.screen
            && let Some(channel_id) = state.message_pane_data().channel_id()
        {
            let request = EditMessageRequest {
                channel_id,
                message_id,
                content,
            };
            let _ = self.command_tx.send(BackendCommand::EditMessage {
                token: token.clone(),
                request,
            });
        }
    }

    fn handle_delete_message(&mut self, message_id: MessageId) {
        if let Some(ref token) = self.current_token
            && let CurrentScreen::Chat(state) = &self.screen
            && let Some(channel_id) = state.message_pane_data().channel_id()
        {
            let _ = self.command_tx.send(BackendCommand::DeleteMessage {
                token: token.clone(),
                channel_id,
                message_id,
            });
        }
    }

    fn handle_send_message(
        &mut self,
        content: String,
        reply_to: Option<MessageId>,
        attachments: Vec<std::path::PathBuf>,
    ) {
        if let Some(ref token) = self.current_token
            && let CurrentScreen::Chat(state) = &self.screen
            && let Some(channel_id) = state.message_pane_data().channel_id()
        {
            let request = SendMessageRequest {
                channel_id,
                content,
                reply_to,
                attachments,
            };
            let _ = self.command_tx.send(BackendCommand::SendMessage {
                token: token.clone(),
                request,
            });
        }
    }

    fn handle_start_typing(&mut self) {
        let now = Instant::now();
        if let CurrentScreen::Chat(state) = &self.screen
            && let Some(channel_id) = state.message_pane_data().channel_id()
            && let Some(ref token) = self.current_token
        {
            if let Some((last_cid, last_time)) = self.last_typing_sent
                && last_cid == channel_id
                && now.duration_since(last_time) < TYPING_THROTTLE_DURATION
            {
                return;
            }

            self.last_typing_sent = Some((channel_id, now));
            let _ = self.command_tx.send(BackendCommand::SendTypingIndicator {
                channel_id,
                token: token.clone(),
            });
        }
    }

    fn handle_login_error(&mut self, error: &AuthError) {
        if let CurrentScreen::Login(screen) = &mut self.screen {
            screen.set_error(error.to_string());
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

        let editor = self
            .editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .or_else(|| std::env::var("VISUAL").ok())
            .unwrap_or_else(|| {
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

        let parts = crate::presentation::ui::utils::split_command(&editor)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid editor command syntax"))?;

        let mut parts_iter = parts.into_iter();
        let cmd = parts_iter.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Empty editor command")
        })?;

        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;

        let status = std::process::Command::new(cmd)
            .args(parts_iter)

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

        async fn fetch_channel(
            &self,
            _token: &AuthToken,
            _channel_id: ChannelId,
        ) -> Result<crate::domain::entities::Channel, AuthError> {
            Ok(crate::domain::entities::Channel::new(
                123,
                "mock-channel".to_string(),
                crate::domain::entities::ChannelKind::Text,
            ))
        }
    }

    #[tokio::test]
    async fn test_app_creation() {
        let auth = Arc::new(MockAuthPort::new(true));
        let data = Arc::new(MockDiscordData);
        let storage = Arc::new(MockTokenStorage::new());
        let theme = Theme::new("Orange", None, false);
        let identity = Arc::new(ClientIdentity::new());
        let config = AppConfig {
            disable_user_colors: false,
            group_guilds: false,
            desktop_notifications: false,
            use_display_name: true,
            image_preview: true,
            timestamp_format: "%H:%M".to_string(),
            show_typing: true,
            internal_notifications: true,
            enable_animations: true,
            editor: None,
            keybindings: std::collections::HashMap::new(),
            notification_duration: 5,
            theme,
            hide_blocked_completely: false,
            quick_switcher_order: QuickSwitcherSortMode::default(),
        };
        let app = App::new(auth, data, storage, config, identity);

        assert_eq!(app.state, AppState::Login);
    }
}
