use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{StatefulWidget, Widget},
};
use std::sync::Arc;
use std::time::Duration;
use tachyonfx::{Effect, Interpolation, fx};

use crate::application::services::autocomplete_service::AutocompleteService;
use crate::application::services::markdown_service::MarkdownService;
use crate::domain::ConnectionStatus;
use crate::domain::entities::{
    CachedUser, Channel, ChannelId, ChannelKind, Guild, GuildId, Message, MessageId, User,
    UserCache,
};
use crate::domain::keybinding::{Action, Keybind};
use crate::presentation::commands::{CommandRegistry, HasCommands};
use crate::presentation::theme::Theme;
use crate::presentation::widgets::{
    ConfirmationModal, FileExplorerAction, FileExplorerComponent, FocusContext, FooterBar,
    ForumState, GuildsTree, GuildsTreeAction, GuildsTreeData, GuildsTreeState, HeaderBar,
    ImageManager, MentionPopup, MessageInput, MessageInputAction, MessageInputMode,
    MessageInputState, MessagePane, MessagePaneAction, MessagePaneData, MessagePaneState, ViewMode,
};
use crate::{NAME, VERSION};

const GUILDS_TREE_WIDTH_PERCENT: u16 = 25;
const GUILDS_TREE_MIN_WIDTH: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFocus {
    GuildsTree,
    MessagesList,
    MessageInput,
    ConfirmationModal,
}

impl ChatFocus {
    const fn next(self, guilds_visible: bool) -> Self {
        if guilds_visible {
            match self {
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput => Self::GuildsTree,
                Self::GuildsTree | Self::ConfirmationModal => Self::MessagesList,
            }
        } else {
            match self {
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput | Self::GuildsTree | Self::ConfirmationModal => {
                    Self::MessagesList
                }
            }
        }
    }

    const fn previous(self, guilds_visible: bool) -> Self {
        if guilds_visible {
            match self {
                Self::GuildsTree => Self::MessageInput,
                Self::MessagesList => Self::GuildsTree,
                Self::MessageInput | Self::ConfirmationModal => Self::MessagesList,
            }
        } else {
            match self {
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput | Self::GuildsTree | Self::ConfirmationModal => {
                    Self::MessagesList
                }
            }
        }
    }

    #[must_use]
    pub const fn to_focus_context(self) -> FocusContext {
        match self {
            Self::GuildsTree => FocusContext::GuildsTree,
            Self::MessagesList => FocusContext::MessagesList,
            Self::MessageInput => FocusContext::MessageInput,
            Self::ConfirmationModal => FocusContext::ConfirmationModal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DmChannelInfo {
    channel_id: ChannelId,
    recipient_name: String,
}

impl DmChannelInfo {
    #[must_use]
    pub const fn new(channel_id: ChannelId, recipient_name: String) -> Self {
        Self {
            channel_id,
            recipient_name,
        }
    }

    #[must_use]
    pub const fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    #[must_use]
    pub fn recipient_name(&self) -> &str {
        &self.recipient_name
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct ChatScreenState {
    user: User,
    focus: ChatFocus,
    guilds_tree_visible: bool,
    guilds_tree_state: GuildsTreeState,
    guilds_tree_data: GuildsTreeData,
    message_pane_state: MessagePaneState,
    message_pane_data: MessagePaneData,
    message_input_state: MessageInputState<'static>,
    autocomplete_service: AutocompleteService,
    user_cache: UserCache,
    selected_guild: Option<GuildId>,
    selected_channel: Option<Channel>,
    dm_channels: std::collections::HashMap<String, DmChannelInfo>,
    read_states: std::collections::HashMap<ChannelId, crate::domain::entities::ReadState>,
    connection_status: ConnectionStatus,
    markdown_service: Arc<MarkdownService>,
    file_explorer: Option<FileExplorerComponent>,
    show_file_explorer: bool,
    show_help: bool,
    registry: CommandRegistry,
    entrance_effect: Effect,
    pending_duration: Duration,
    has_entered: bool,
    /// Image manager for rendering image attachments.
    image_manager: ImageManager,
    disable_user_colors: bool,
    theme: Theme,
    forum_states: std::collections::HashMap<ChannelId, crate::presentation::widgets::ForumState>,
    pending_deletion_id: Option<MessageId>,
}

impl ChatScreenState {
    #[must_use]
    pub fn new(
        user: User,
        markdown_service: Arc<MarkdownService>,
        user_cache: UserCache,
        disable_user_colors: bool,
        theme: Theme,
    ) -> Self {
        let mut guilds_tree_state = GuildsTreeState::new();
        guilds_tree_state.set_focused(true);

        Self {
            user,
            focus: ChatFocus::GuildsTree,
            guilds_tree_visible: true,
            guilds_tree_state,
            guilds_tree_data: GuildsTreeData::new(),
            message_pane_state: MessagePaneState::new(),
            message_pane_data: MessagePaneData::new(),
            message_input_state: MessageInputState::new(),
            autocomplete_service: AutocompleteService::new(),
            user_cache,
            selected_guild: None,
            selected_channel: None,
            dm_channels: std::collections::HashMap::new(),
            read_states: std::collections::HashMap::new(),
            connection_status: ConnectionStatus::Disconnected,
            markdown_service,
            file_explorer: None,
            show_file_explorer: false,
            show_help: false,
            registry: CommandRegistry::default(),
            entrance_effect: fx::coalesce((800, Interpolation::SineOut)),
            pending_duration: Duration::ZERO,
            has_entered: false,
            image_manager: ImageManager::new(),
            disable_user_colors,
            theme,
            forum_states: std::collections::HashMap::new(),
            pending_deletion_id: None,
        }
    }

    pub fn restore_state(
        &mut self,
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        channels: Option<Vec<Channel>>,
        messages: Option<Vec<Message>>,
    ) -> Option<ChatKeyResult> {
        if let Some(gid) = guild_id
            && let Some(chans) = channels
        {
            self.set_channels(gid, chans);
        }

        let mut restored = false;
        if let Some(cid) = channel_id
            && self.on_channel_selected(cid).is_some()
        {
            if let Some(msgs) = messages {
                self.set_messages(msgs);
                if let Some(last_msg) = self.message_pane_data.messages().back().map(|m| &m.message)
                {
                    self.mark_channel_read(cid, last_msg.id());
                }
            }
            restored = true;
        }

        if !restored && let Some(first_guild) = self.guilds_tree_data.guilds().first() {
            let guild_id = first_guild.id();
            return self.on_guild_selected(guild_id);
        }

        None
    }

    pub fn tick(&mut self, duration: Duration) {
        if !self.has_entered {
            self.pending_duration = self.pending_duration.saturating_add(duration);
        }
    }

    #[must_use]
    pub const fn has_entered(&self) -> bool {
        self.has_entered
    }

    #[must_use]
    pub const fn user(&self) -> &User {
        &self.user
    }

    #[must_use]
    pub const fn focus(&self) -> ChatFocus {
        self.focus
    }

    #[must_use]
    pub const fn is_guilds_tree_visible(&self) -> bool {
        self.guilds_tree_visible
    }

    #[must_use]
    pub const fn selected_channel(&self) -> Option<&Channel> {
        self.selected_channel.as_ref()
    }

    #[must_use]
    pub const fn selected_guild(&self) -> Option<GuildId> {
        self.selected_guild
    }

    #[must_use]
    pub const fn connection_status(&self) -> ConnectionStatus {
        self.connection_status
    }

    pub const fn set_connection_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
    }

    pub fn set_guilds(&mut self, guilds: Vec<Guild>) {
        self.guilds_tree_data.set_guilds(guilds);
    }

    pub fn set_channels(&mut self, guild_id: GuildId, channels: Vec<Channel>) {
        self.guilds_tree_data.set_channels(guild_id, channels);
        self.recalculate_all_unread();
    }

    pub fn set_dm_users(&mut self, users: Vec<(String, String)>) {
        self.dm_channels.clear();
        for (channel_id_str, recipient_name) in &users {
            if let Ok(channel_id) = channel_id_str.parse::<u64>() {
                let info = DmChannelInfo::new(ChannelId(channel_id), recipient_name.clone());
                self.dm_channels.insert(channel_id_str.clone(), info);
            }
        }
        self.guilds_tree_data.set_dm_users(users);
    }

    pub fn set_read_states(
        &mut self,
        read_states: std::collections::HashMap<ChannelId, crate::domain::entities::ReadState>,
    ) {
        self.read_states = read_states;
        self.recalculate_all_unread();
    }

    fn recalculate_all_unread(&mut self) {
        self.guilds_tree_data
            .update_unread_status(&self.read_states);
    }

    pub fn mark_channel_read(&mut self, channel_id: ChannelId, message_id: MessageId) {
        if let Some(read_state) = self.read_states.get_mut(&channel_id) {
            read_state.last_read_message_id = Some(message_id);
        } else {
            self.read_states.insert(
                channel_id,
                crate::domain::entities::ReadState::new(channel_id, Some(message_id)),
            );
        }
        self.recalculate_all_unread();
    }

    pub fn on_message_received(&mut self, message: &Message) {
        if let Some(channel) = self.guilds_tree_data.get_channel_mut(message.channel_id()) {
            channel.set_last_message_id(Some(message.id()));

            let is_own_message = message.author().id() == self.user.id_str();
            let is_active_channel = self
                .selected_channel
                .as_ref()
                .map(crate::domain::entities::Channel::id)
                == Some(message.channel_id());

            if is_own_message || is_active_channel {
                self.mark_channel_read(message.channel_id(), message.id());
            } else {
                self.recalculate_all_unread();
            }
        }
    }

    pub fn toggle_guilds_tree(&mut self) {
        self.guilds_tree_visible = !self.guilds_tree_visible;
        if !self.guilds_tree_visible && self.focus == ChatFocus::GuildsTree {
            self.focus_next();
        }
    }

    pub fn focus_guilds_tree(&mut self) {
        if self.guilds_tree_visible {
            self.set_focus(ChatFocus::GuildsTree);
        }
    }

    pub fn focus_messages_list(&mut self) {
        self.set_focus(ChatFocus::MessagesList);
    }

    pub fn focus_message_input(&mut self) {
        self.set_focus(ChatFocus::MessageInput);
    }

    pub fn focus_next(&mut self) {
        let new_focus = self.focus.next(self.guilds_tree_visible);
        self.set_focus(new_focus);
    }

    pub fn focus_previous(&mut self) {
        let new_focus = self.focus.previous(self.guilds_tree_visible);
        self.set_focus(new_focus);
    }

    fn set_focus(&mut self, focus: ChatFocus) {
        self.focus = focus;
        self.guilds_tree_state
            .set_focused(focus == ChatFocus::GuildsTree);
        self.message_pane_state
            .set_focused(focus == ChatFocus::MessagesList);
        self.message_input_state
            .set_focused(focus == ChatFocus::MessageInput);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if self.show_help {
            if let Some(action) = self.registry.find_action(key)
                && matches!(action, Action::ToggleHelp | Action::Quit | Action::Cancel)
            {
                self.toggle_help();
                return ChatKeyResult::Consumed;
            }
            return ChatKeyResult::Consumed;
        }

        if self.focus == ChatFocus::ConfirmationModal {
            match key.code {
                KeyCode::Enter => {
                    if let Some(id) = self.pending_deletion_id.take() {
                        self.set_focus(ChatFocus::MessagesList);
                        return ChatKeyResult::DeleteMessage(id);
                    }
                    self.set_focus(ChatFocus::MessagesList);
                    return ChatKeyResult::Consumed;
                }
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.pending_deletion_id = None;
                    self.set_focus(ChatFocus::MessagesList);
                    return ChatKeyResult::Consumed;
                }
                _ => return ChatKeyResult::Consumed,
            }
        }

        if self.show_file_explorer {
            if let Some(action) = self.registry.find_action(key)
                && (action == Action::ToggleFileExplorer || action == Action::Cancel)
            {
                self.toggle_file_explorer();
                return ChatKeyResult::Consumed;
            }
            return self.handle_file_explorer_key(key);
        }

        if self.focus == ChatFocus::MessageInput {
            let result = self.handle_message_input_key(key);
            if result != ChatKeyResult::Ignored {
                return result;
            }
        } else {
            match self.focus {
                ChatFocus::GuildsTree => {
                    let result = self.handle_guilds_tree_key(key);
                    if result != ChatKeyResult::Ignored {
                        return result;
                    }
                }
                ChatFocus::MessagesList => {
                    let result = self.handle_messages_list_key(key);
                    if result != ChatKeyResult::Ignored {
                        return result;
                    }
                }
                ChatFocus::ConfirmationModal => return ChatKeyResult::Consumed,
                ChatFocus::MessageInput => unreachable!(),
            }
        }

        if let Some(result) = self.handle_global_key(key) {
            return result;
        }

        ChatKeyResult::Consumed
    }

    fn handle_global_key(&mut self, key: KeyEvent) -> Option<ChatKeyResult> {
        match self.registry.find_action(key) {
            Some(Action::Quit) => Some(ChatKeyResult::Quit),
            Some(Action::Logout) => Some(ChatKeyResult::Logout),
            Some(Action::FocusGuilds) => {
                self.focus_guilds_tree();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::FocusMessages) => {
                self.focus_messages_list();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::FocusInput) => {
                if self.focus == ChatFocus::MessageInput {
                    return None;
                }
                self.focus_message_input();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::FocusPrevious) => {
                self.focus_previous();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::FocusNext | Action::NextTab) => {
                self.focus_next();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::ToggleGuildsTree) => {
                self.toggle_guilds_tree();
                Some(ChatKeyResult::Consumed)
            }
            Some(Action::ToggleHelp) => {
                if self.focus == ChatFocus::MessageInput && matches!(key.code, KeyCode::Char(_)) {
                    return None;
                }
                self.toggle_help();
                Some(ChatKeyResult::ToggleHelp)
            }
            _ => None,
        }
    }

    fn handle_guilds_tree_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        use crate::presentation::widgets::GuildsTreeStyle;

        let style = GuildsTreeStyle::from_theme(&self.theme);

        if let Some(action) =
            self.guilds_tree_state
                .handle_key(key, &self.guilds_tree_data, &self.registry, &style)
        {
            match action {
                GuildsTreeAction::SelectChannel(channel_id) => {
                    if let Some(result) = self.on_channel_selected(channel_id) {
                        return result;
                    }
                }
                GuildsTreeAction::SelectGuild(guild_id) => {
                    if let Some(result) = self.on_guild_selected(guild_id) {
                        return result;
                    }
                }
                GuildsTreeAction::SelectDirectMessage(dm_channel_id) => {
                    if let Some(result) = self.on_dm_selected(&dm_channel_id) {
                        return result;
                    }
                }
                GuildsTreeAction::YankId(id) => {
                    return ChatKeyResult::CopyToClipboard(id);
                }
                GuildsTreeAction::LoadGuildChannels(guild_id) => {
                    return ChatKeyResult::LoadGuildChannels(guild_id);
                }
            }
        }
        ChatKeyResult::Ignored
    }

    #[allow(clippy::too_many_lines)]
    fn handle_messages_list_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if let Some(action) =
            self.message_pane_state
                .handle_key(key, &self.message_pane_data, &self.registry)
        {
            match action {
                MessagePaneAction::ClearSelection | MessagePaneAction::SelectMessage(_) => {}
                MessagePaneAction::Reply {
                    message_id,
                    mention,
                } => {
                    return ChatKeyResult::ReplyToMessage {
                        message_id,
                        mention,
                    };
                }
                MessagePaneAction::Edit(message_id) => {
                    if let Some(message) = self
                        .message_pane_data
                        .messages()
                        .iter()
                        .find(|m| m.message.id() == message_id)
                    {
                        self.message_input_state
                            .start_edit(message_id, message.message.content());
                        self.focus_message_input();
                    }
                }
                MessagePaneAction::EditExternal(message_id) => {
                    if let Some(message) = self
                        .message_pane_data
                        .messages()
                        .iter()
                        .find(|m| m.message.id() == message_id)
                    {
                        return ChatKeyResult::OpenEditor {
                            initial_content: message.message.content().to_string(),
                            message_id: Some(message_id),
                        };
                    }
                }
                MessagePaneAction::Delete(message_id) => {
                    if let Some(message) = self
                        .message_pane_data
                        .messages()
                        .iter()
                        .find(|m| m.message.id() == message_id)
                        && message.message.author().id() == self.user.id_str()
                    {
                        self.pending_deletion_id = Some(message_id);
                        self.set_focus(ChatFocus::ConfirmationModal);
                        return ChatKeyResult::Consumed;
                    }
                }
                MessagePaneAction::YankContent(content) | MessagePaneAction::YankUrl(content) => {
                    return ChatKeyResult::CopyToClipboard(content);
                }
                MessagePaneAction::YankId(id) => {
                    return ChatKeyResult::CopyToClipboard(id);
                }
                MessagePaneAction::OpenAttachments(message_id) => {
                    return ChatKeyResult::OpenAttachments(message_id);
                }
                MessagePaneAction::JumpToReply(message_id) => {
                    return ChatKeyResult::JumpToMessage(message_id);
                }
                MessagePaneAction::OpenThread(channel_id) => {
                    if let Some(result) = self.on_channel_selected(channel_id) {
                        return result;
                    }

                    if let ViewMode::Forum(state) = &self.message_pane_state.view_mode
                        && let Some(thread) = state.threads.iter().find(|t| t.id == channel_id)
                    {
                        let parent_id = self.selected_channel.as_ref().map(Channel::id);

                        let mut channel =
                            Channel::new(thread.id, thread.name.clone(), ChannelKind::PublicThread)
                                .with_guild(thread.guild_id.unwrap_or(GuildId(0)).as_u64());

                        if let Some(pid) = parent_id {
                            channel = channel.with_parent(pid);
                        }

                        if let Some(guild_id) = thread.guild_id {
                            self.selected_channel = Some(channel.clone());
                            self.message_pane_data
                                .set_channel(channel_id, channel.display_name());
                            self.message_pane_state.on_channel_change();
                            self.message_input_state.set_has_channel(true);
                            self.message_input_state.clear();
                            self.focus_messages_list();

                            return ChatKeyResult::LoadChannelMessages {
                                channel_id,
                                guild_id: Some(guild_id),
                            };
                        }
                    }
                }
                MessagePaneAction::CloseThread => {
                    if let ViewMode::Forum(_) = &self.message_pane_state.view_mode {
                        self.focus_guilds_tree();
                        return ChatKeyResult::Consumed;
                    }
                    if let Some(current_channel) = &self.selected_channel
                        && let Some(parent_id) = current_channel.parent_id()
                        && let Some(result) = self.on_channel_selected(parent_id)
                    {
                        return result;
                    }
                    self.focus_guilds_tree();
                }
                MessagePaneAction::LoadHistory => {
                    if let ViewMode::Forum(forum_state) = &self.message_pane_state.view_mode
                        && let Some(channel_id) = self.message_pane_data.channel_id()
                    {
                        let offset = u32::try_from(forum_state.threads.len()).unwrap_or(0);
                        let guild_id = self.selected_guild;

                        return ChatKeyResult::LoadForumThreads {
                            channel_id,
                            guild_id,
                            offset,
                        };
                    }

                    if let Some(channel_id) = self.message_pane_data.channel_id()
                        && let Some(first_msg) = self.message_pane_data.messages().iter().next()
                    {
                        return ChatKeyResult::LoadHistory {
                            channel_id,
                            before_message_id: first_msg.message.id(),
                        };
                    }
                }
            }
        }
        ChatKeyResult::Ignored
    }

    fn handle_message_input_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if self.registry.find_action(key) == Some(Action::ToggleFileExplorer) {
            self.toggle_file_explorer();
            return ChatKeyResult::Consumed;
        }

        if self.handle_autocomplete_navigation(key) {
            return ChatKeyResult::Consumed;
        }

        let autocomplete_changed;

        if let Some(action) = self.message_input_state.handle_key(key, &self.registry) {
            let value = self.message_input_state.value();
            let cursor_idx = self.message_input_state.get_cursor_index();
            autocomplete_changed = self.autocomplete_service.process_input(&value, cursor_idx);

            match action {
                MessageInputAction::SendMessage {
                    content,
                    reply_to,
                    attachments,
                } => {
                    return ChatKeyResult::SendMessage {
                        content,
                        reply_to,
                        attachments,
                    };
                }
                MessageInputAction::EditMessage {
                    message_id,
                    content,
                } => {
                    return ChatKeyResult::SubmitEdit {
                        message_id,
                        content,
                    };
                }
                MessageInputAction::ExitInput => {
                    self.focus_messages_list();
                }
                MessageInputAction::OpenEditor => {
                    let initial_content = self.message_input_state.value();
                    let message_id = match self.message_input_state.mode() {
                        MessageInputMode::Editing { message_id } => Some(*message_id),
                        _ => None,
                    };
                    return ChatKeyResult::OpenEditor {
                        initial_content,
                        message_id,
                    };
                }
                MessageInputAction::Paste => {
                    return ChatKeyResult::Paste;
                }
                MessageInputAction::StartTyping | MessageInputAction::CancelReply => {}
            }
        } else {
            let value = self.message_input_state.value();
            let cursor_idx = self.message_input_state.get_cursor_index();
            autocomplete_changed = self.autocomplete_service.process_input(&value, cursor_idx);
        }

        if autocomplete_changed {
            self.update_autocomplete_suggestions();
            return ChatKeyResult::StartTyping;
        }

        ChatKeyResult::Ignored
    }

    fn handle_autocomplete_navigation(&mut self, key: KeyEvent) -> bool {
        if !self.autocomplete_service.state().active {
            return false;
        }

        match key.code {
            KeyCode::Up => {
                self.autocomplete_service.select_previous();
                true
            }
            KeyCode::Down => {
                self.autocomplete_service.select_next();
                true
            }
            KeyCode::Enter | KeyCode::Tab => {
                if let Some(user) = self.autocomplete_service.state().selected_user() {
                    let trigger_idx = self.autocomplete_service.state().trigger_index;
                    self.message_input_state
                        .insert_mention(trigger_idx, user.id());
                }
                self.autocomplete_service.reset();
                true
            }
            KeyCode::Esc => {
                self.autocomplete_service.reset();
                true
            }
            _ => false,
        }
    }

    fn update_autocomplete_suggestions(&mut self) {
        if !self.autocomplete_service.state().active {
            return;
        }

        let mut candidates = Vec::new();
        candidates.push(CachedUser::from_user(&self.user));

        let mut seen_ids = std::collections::HashSet::new();
        seen_ids.insert(self.user.id().to_string());

        for msg in self.message_pane_data.messages() {
            let author = msg.message.author();
            if seen_ids.insert(author.id().to_string()) {
                if let Some(cached) = self.user_cache.get(author.id()) {
                    candidates.push(cached);
                } else {
                    let cached = CachedUser::new(
                        author.id(),
                        author.username(),
                        author.discriminator(),
                        author.avatar().map(String::from),
                        author.is_bot(),
                    );
                    candidates.push(cached);
                }
            }

            for mention in msg.message.mentions() {
                if seen_ids.insert(mention.id().to_string()) {
                    if let Some(cached) = self.user_cache.get(&mention.id_str()) {
                        candidates.push(cached);
                    } else {
                        candidates.push(CachedUser::from_user(mention));
                    }
                }
            }
        }

        self.autocomplete_service.update_results(candidates);
    }

    fn on_channel_selected(&mut self, channel_id: ChannelId) -> Option<ChatKeyResult> {
        let mut guild_id = self.selected_guild;

        if let Some(id) = guild_id {
            let channel_in_guild = self
                .guilds_tree_data
                .channels(id)
                .is_some_and(|channels| channels.iter().any(|c| c.id() == channel_id));
            if !channel_in_guild {
                guild_id = None;
            }
        }

        if guild_id.is_none() {
            guild_id = self.guilds_tree_data.find_guild_for_channel(channel_id);
        }

        let channel_info = if let Some(guild_id) = guild_id
            && let Some(channels) = self.guilds_tree_data.channels(guild_id)
            && let Some(channel) = channels.iter().find(|c| c.id() == channel_id)
        {
            Some((guild_id, channel.clone(), channel.topic().map(String::from)))
        } else {
            None
        };

        if let Some((guild_id, channel, topic)) = channel_info {
            self.selected_guild = Some(guild_id);
            self.selected_channel = Some(channel.clone());

            self.guilds_tree_data.set_active_guild(Some(guild_id));
            self.guilds_tree_data.set_active_channel(Some(channel_id));
            self.guilds_tree_data.set_active_dm_user(None);

            let channel_name = channel.display_name();

            if let Some(current_channel_id) = self.message_pane_data.channel_id()
                && let ViewMode::Forum(state) = &self.message_pane_state.view_mode
            {
                self.forum_states.insert(current_channel_id, state.clone());
            }

            self.message_pane_data.set_channel(channel_id, channel_name);
            self.message_pane_state.on_channel_change();

            if channel.kind() == ChannelKind::Forum {
                if let Some(saved_state) = self.forum_states.get(&channel_id) {
                    self.message_pane_state.view_mode = ViewMode::Forum(saved_state.clone());
                } else {
                    self.message_pane_state.view_mode = ViewMode::Forum(ForumState::default());
                }
            }

            self.message_pane_data.set_channel_topic(topic);
            self.message_input_state.set_has_channel(true);
            self.message_input_state.clear();
            self.focus_messages_list();

            if channel.kind() == ChannelKind::Forum {
                if let ViewMode::Forum(ref state) = self.message_pane_state.view_mode
                    && !state.threads.is_empty()
                {
                    return None;
                }

                return Some(ChatKeyResult::LoadForumThreads {
                    channel_id,
                    guild_id: Some(guild_id),
                    offset: 0,
                });
            }
            return Some(ChatKeyResult::LoadChannelMessages {
                channel_id,
                guild_id: Some(guild_id),
            });
        }
        None
    }

    fn on_guild_selected(&mut self, guild_id: GuildId) -> Option<ChatKeyResult> {
        if self.selected_guild == Some(guild_id) {
            return None;
        }

        self.selected_guild = Some(guild_id);
        self.selected_channel = None;
        self.guilds_tree_data.set_active_channel(None);
        self.guilds_tree_data.set_active_guild(Some(guild_id));

        if self.guilds_tree_data.channels(guild_id).is_none() {
            return Some(ChatKeyResult::LoadGuildChannels(guild_id));
        }

        None
    }

    fn on_dm_selected(&mut self, dm_channel_id: &str) -> Option<ChatKeyResult> {
        if let Some(dm_info) = self.dm_channels.get(dm_channel_id) {
            let channel_id = dm_info.channel_id();
            let recipient_name = dm_info.recipient_name().to_string();

            let dm_channel = Channel::new(channel_id, recipient_name.clone(), ChannelKind::Dm);
            self.selected_channel = Some(dm_channel);
            self.selected_guild = None;
            self.guilds_tree_data.set_active_guild(None);
            self.guilds_tree_data.set_active_channel(None);
            self.guilds_tree_data
                .set_active_dm_user(Some(dm_channel_id.to_string()));

            let display_name = format!("@{recipient_name}");
            self.message_pane_data.set_channel(channel_id, display_name);
            self.message_pane_state.on_channel_change();
            self.message_input_state.set_has_channel(true);
            self.message_input_state.clear();

            self.focus_messages_list();

            return Some(ChatKeyResult::LoadDmMessages {
                channel_id,
                recipient_name,
            });
        }
        None
    }

    #[must_use]
    pub const fn guilds_tree_data(&self) -> &GuildsTreeData {
        &self.guilds_tree_data
    }

    pub const fn guilds_tree_state_mut(&mut self) -> &mut GuildsTreeState {
        &mut self.guilds_tree_state
    }

    pub const fn guilds_tree_parts_mut(&mut self) -> (&GuildsTreeData, &mut GuildsTreeState) {
        (&self.guilds_tree_data, &mut self.guilds_tree_state)
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.message_pane_data.set_messages(messages);
    }

    pub fn set_forum_threads(
        &mut self,
        mut threads: Vec<crate::domain::entities::ForumThread>,
        offset: u32,
    ) {
        for thread in &mut threads {
            let read_state = self.read_states.get(&thread.id);
            thread.new = match (read_state, thread.last_message_id) {
                (Some(rs), Some(last_msg_id)) => rs.last_read_message_id != Some(last_msg_id),
                (Some(_), None) => false,
                (None, _) => true,
            };
        }

        if let ViewMode::Forum(state) = &mut self.message_pane_state.view_mode {
            if offset > 0 {
                let old_selection_id = state.threads.get(state.selected_idx).map(|t| t.id);
                let added_count = threads.len();

                let mut new_list = threads;
                new_list.append(&mut state.threads);
                state.threads = new_list;

                if let Some(id) = old_selection_id
                    && let Some(new_idx) = state.threads.iter().position(|t| t.id == id)
                {
                    state.selected_idx = new_idx;
                    state.scroll_offset = state
                        .scroll_offset
                        .saturating_add(u16::try_from(added_count).unwrap_or(0));
                }
                return;
            }

            let was_empty = state.threads.is_empty();
            let was_at_bottom = state.selected_idx + 1 >= state.threads.len();
            let invalid_selection = state.selected_idx >= state.threads.len();

            state.threads = threads;

            if (was_empty || was_at_bottom || invalid_selection) && !state.threads.is_empty() {
                state.selected_idx = state.threads.len().saturating_sub(1);
                state.needs_scroll_to_selection = true;
            }
        } else {
            let mut state = crate::presentation::widgets::ForumState {
                threads,
                ..Default::default()
            };
            if !state.threads.is_empty() {
                state.selected_idx = state.threads.len().saturating_sub(1);
                state.needs_scroll_to_selection = true;
            }
            self.message_pane_state.view_mode = ViewMode::Forum(state);
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.message_pane_data.add_message(message);
        self.message_pane_state.on_new_message();
    }

    pub fn prepend_messages(&mut self, new_messages: Vec<Message>) {
        if new_messages.is_empty() {
            return;
        }

        let width = self.message_pane_state.last_width();
        let pane = MessagePane::new(&mut self.message_pane_data, &self.markdown_service);
        let added_height: u16 = new_messages
            .iter()
            .map(|m| {
                pane.calculate_message_height(m, width, &self.markdown_service, self.theme.accent)
            })
            .sum();

        let added_count = self.message_pane_data.prepend_messages(new_messages);

        if added_count > 0 {
            self.message_pane_state
                .adjust_for_prepend(added_count, added_height.into());
        }
    }

    pub fn update_message(&mut self, message: Message) {
        self.message_pane_data.update_message(message);
    }

    pub fn remove_message(&mut self, message_id: crate::domain::entities::MessageId) {
        self.message_pane_data.remove_message(message_id);
    }

    pub fn set_message_error(&mut self, error: String) {
        self.message_pane_data.set_error(error);
    }

    pub fn set_typing_indicator(&mut self, indicator: Option<String>) {
        self.message_pane_data.set_typing_indicator(indicator);
    }

    #[must_use]
    pub const fn message_pane_data(&self) -> &MessagePaneData {
        &self.message_pane_data
    }

    pub const fn message_pane_data_mut(&mut self) -> &mut MessagePaneData {
        &mut self.message_pane_data
    }

    pub const fn message_pane_parts_mut(
        &mut self,
    ) -> (&mut MessagePaneData, &mut MessagePaneState) {
        (&mut self.message_pane_data, &mut self.message_pane_state)
    }

    pub fn start_reply(&mut self, message_id: MessageId, author_name: String, mention: bool) {
        self.message_input_state
            .start_reply(message_id, author_name, mention);
        self.focus_message_input();
    }

    pub fn cancel_reply(&mut self) {
        self.message_input_state.reset_mode();
    }

    pub fn clear_input(&mut self) {
        self.message_input_state.clear();
    }

    pub fn get_reply_author(&self, message_id: MessageId) -> Option<String> {
        self.message_pane_data
            .messages()
            .iter()
            .find(|m| m.message.id() == message_id)
            .map(|m| m.message.author().display_name().clone())
    }

    pub fn message_input_parts_mut(&mut self) -> &mut MessageInputState<'static> {
        &mut self.message_input_state
    }

    pub fn message_input_state(&self) -> &MessageInputState<'static> {
        &self.message_input_state
    }

    /// Get the current message input value.
    pub fn message_input_value(&self) -> String {
        self.message_input_state.value()
    }

    /// Get the current reply info (`message_id`, author) if in reply mode.
    pub fn message_input_reply_info(&self) -> Option<(MessageId, String, bool)> {
        match self.message_input_state.mode() {
            MessageInputMode::Reply {
                message_id,
                author,
                mention,
            } => Some((*message_id, author.clone(), *mention)),
            MessageInputMode::Normal | MessageInputMode::Editing { .. } => None,
        }
    }

    /// Set the message input content.
    pub fn set_message_input_content(&mut self, content: &str) {
        self.message_input_state.set_content(content);
    }

    /// Returns a reference to the image manager.
    #[must_use]
    pub const fn image_manager(&self) -> &ImageManager {
        &self.image_manager
    }

    /// Returns a mutable reference to the image manager.
    pub fn image_manager_mut(&mut self) -> &mut ImageManager {
        &mut self.image_manager
    }

    /// Collects all image attachments that need loading within the visible range.
    /// Returns a list of (`ImageId`, URL) pairs.
    #[must_use]
    pub fn collect_needed_image_loads(&self) -> Vec<(crate::domain::entities::ImageId, String)> {
        let mut needed = Vec::new();

        let visible_range = self.calculate_visible_range();
        let buffer = super::super::widgets::LOAD_BUFFER;
        let start = visible_range.0.saturating_sub(buffer);
        let end = (visible_range.1 + buffer).min(self.message_pane_data.message_count());

        for idx in start..end {
            if let Some(ui_msg) = self.message_pane_data.messages().get(idx) {
                for load in ui_msg.collect_image_loads() {
                    needed.push(load);
                }
            }
        }

        needed
    }

    /// Calculates the visible message range based on scroll position.
    fn calculate_visible_range(&self) -> (usize, usize) {
        let offset = self.message_pane_state.vertical_scroll;
        let viewport_height = self.message_pane_state.viewport_height() as usize;

        let mut y = 0;
        let mut start_idx = 0;
        let mut end_idx = 0;
        let mut found_start = false;

        for (idx, msg) in self.message_pane_data.messages().iter().enumerate() {
            let h = msg.estimated_height as usize;

            if !found_start && y + h > offset {
                start_idx = idx;
                found_start = true;
            }

            if y >= offset + viewport_height {
                end_idx = idx;
                break;
            }

            y += h;
            end_idx = idx + 1;
        }

        (start_idx, end_idx)
    }

    /// Updates an image attachment when it finishes loading.
    pub fn on_image_loaded(
        &mut self,
        id: &crate::domain::entities::ImageId,
        image: &std::sync::Arc<image::DynamicImage>,
    ) {
        for ui_msg in self.message_pane_data.ui_messages_mut() {
            for attachment in &mut ui_msg.image_attachments {
                if &attachment.id == id {
                    attachment.set_loaded(image.clone());
                }
            }
        }
        self.message_pane_data.mark_dirty();
    }

    /// Updates image protocols for visible messages.
    /// Should be called before rendering.
    /// Only processes images that need protocol updates.
    pub fn update_visible_image_protocols(&mut self, terminal_width: u16) {
        if self.message_pane_data.is_empty() {
            return;
        }

        self.image_manager.set_width(terminal_width);

        let visible_range = self.calculate_visible_range();
        let buffer = super::super::widgets::LOAD_BUFFER;
        let start = visible_range.0.saturating_sub(buffer);
        let end = (visible_range.1 + buffer).min(self.message_pane_data.message_count());

        let picker = self.image_manager.picker();

        for idx in start..end {
            if let Some(ui_msg) = self.message_pane_data.ui_messages_mut().get_mut(idx) {
                for attachment in &mut ui_msg.image_attachments {
                    if attachment.is_ready() {
                        attachment.update_protocol_if_needed(picker);
                    }
                }
            }
        }
    }

    /// Marks an image as downloading.
    pub fn mark_image_downloading(&mut self, id: &crate::domain::entities::ImageId) {
        for ui_msg in self.message_pane_data.ui_messages_mut() {
            for attachment in &mut ui_msg.image_attachments {
                if &attachment.id == id {
                    attachment.set_downloading();
                }
            }
        }
    }

    /// Marks an image as failed.
    pub fn mark_image_failed(&mut self, id: &crate::domain::entities::ImageId, error: &str) {
        for ui_msg in self.message_pane_data.ui_messages_mut() {
            for attachment in &mut ui_msg.image_attachments {
                if &attachment.id == id {
                    attachment.set_failed(error.to_owned());
                }
            }
        }
    }

    pub fn add_attachment(&mut self, path: std::path::PathBuf) {
        self.message_input_state.add_attachment(path);
        self.focus_message_input();
    }

    pub fn insert_text(&mut self, text: &str) {
        self.message_input_state.insert_text_at_cursor(text);
        self.focus_message_input();
    }

    pub fn jump_to_message(&mut self, message_id: crate::domain::entities::MessageId) {
        if let Some(index) = self
            .message_pane_data
            .messages()
            .iter()
            .position(|m| m.message.id() == message_id)
        {
            self.message_pane_state.jump_to_index(index);
        }
    }

    pub fn toggle_file_explorer(&mut self) {
        self.show_file_explorer = !self.show_file_explorer;
        if self.show_file_explorer {
            self.file_explorer = Some(FileExplorerComponent::new());
        } else {
            self.file_explorer = None;
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    fn handle_file_explorer_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if self.registry.find_action(key) == Some(Action::ToggleHiddenFiles)
            && let Some(explorer) = &mut self.file_explorer
        {
            explorer.toggle_hidden();
            return ChatKeyResult::Consumed;
        }

        if let Some(explorer) = &mut self.file_explorer {
            match explorer.handle_key(key) {
                FileExplorerAction::SelectFile(path) => {
                    self.message_input_state.add_attachment(path);
                    self.show_file_explorer = false;
                    self.file_explorer = None;
                    ChatKeyResult::Consumed
                }
                FileExplorerAction::Close => {
                    self.show_file_explorer = false;
                    self.file_explorer = None;
                    ChatKeyResult::Consumed
                }
                FileExplorerAction::None => ChatKeyResult::Consumed,
            }
        } else {
            ChatKeyResult::Consumed
        }
    }
}

impl HasCommands for ChatScreenState {
    fn get_commands(&self, registry: &CommandRegistry) -> Vec<Keybind> {
        let mut commands = Vec::new();

        if self.show_help {
            if let Some(key) = registry.get_first(Action::ToggleHelp) {
                commands.push(Keybind::new(key, Action::ToggleHelp, "Close Help"));
            }
            return commands;
        }

        if self.show_file_explorer {
            if let Some(key) = registry.get_first(Action::Select) {
                commands.push(Keybind::new(key, Action::Select, "Select"));
            }
            if let Some(key) = registry.get_first(Action::ToggleFileExplorer) {
                commands.push(Keybind::new(key, Action::ToggleFileExplorer, "Close"));
            }
            commands.push(Keybind::new(
                KeyEvent::from(KeyCode::Char('.')),
                Action::ToggleHiddenFiles,
                "Hidden",
            ));
        } else {
            match self.focus {
                ChatFocus::GuildsTree => {
                    if let Some(key) = registry.get_first(Action::NavigateUp) {
                        commands.push(Keybind::new(key, Action::NavigateUp, "Nav"));
                    }
                    if let Some(key) = registry.get_first(Action::Select) {
                        commands.push(Keybind::new(key, Action::Select, "Select"));
                    }
                    if let Some(key) = registry.get_first(Action::NavigateRight) {
                        commands.push(Keybind::new(key, Action::NavigateRight, "Expand"));
                    }
                }
                ChatFocus::MessagesList => {
                    if let Some(key) = registry.get_first(Action::NavigateUp) {
                        commands.push(Keybind::new(key, Action::NavigateUp, "Nav"));
                    }
                    if let Some(key) = registry.get_first(Action::Reply) {
                        commands.push(Keybind::new(key, Action::Reply, "Reply"));
                    }
                    if let Some(key) = registry.get_first(Action::EditMessage) {
                        commands.push(Keybind::new(key, Action::EditMessage, "Edit"));
                    }
                    if let Some(key) = registry.get_first(Action::DeleteMessage) {
                        commands.push(Keybind::new(key, Action::DeleteMessage, "Del"));
                    }
                }
                ChatFocus::MessageInput => {
                    if let Some(key) = registry.get_first(Action::SendMessage) {
                        commands.push(Keybind::new(key, Action::SendMessage, "Send"));
                    }
                    if let Some(key) = registry.get_first(Action::OpenEditor) {
                        commands.push(Keybind::new(key, Action::OpenEditor, "Editor"));
                    }
                    if let Some(key) = registry.get_first(Action::Cancel) {
                        commands.push(Keybind::new(key, Action::Cancel, "Cancel"));
                    }
                }
                ChatFocus::ConfirmationModal => {
                    commands.push(Keybind::new(
                        KeyEvent::from(KeyCode::Enter),
                        Action::Select,
                        "Confirm",
                    ));
                    commands.push(Keybind::new(
                        KeyEvent::from(KeyCode::Esc),
                        Action::Cancel,
                        "Cancel",
                    ));
                }
            }

            if let Some(key) = registry.get_first(Action::NextTab) {
                commands.push(Keybind::new(key, Action::NextTab, "Next"));
            }
            if let Some(key) = registry.get_first(Action::ToggleHelp) {
                commands.push(Keybind::new(key, Action::ToggleHelp, "Help"));
            }
        }

        commands
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatKeyResult {
    Consumed,
    Ignored,
    Quit,
    Logout,
    CopyToClipboard(String),
    LoadGuildChannels(GuildId),
    LoadChannelMessages {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    },
    LoadForumThreads {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
        offset: u32,
    },
    LoadDmMessages {
        channel_id: ChannelId,
        recipient_name: String,
    },
    ReplyToMessage {
        message_id: crate::domain::entities::MessageId,
        mention: bool,
    },
    SubmitEdit {
        message_id: crate::domain::entities::MessageId,
        content: String,
    },
    LoadHistory {
        channel_id: ChannelId,
        before_message_id: MessageId,
    },
    EditMessage(crate::domain::entities::MessageId),
    DeleteMessage(crate::domain::entities::MessageId),
    OpenAttachments(crate::domain::entities::MessageId),
    JumpToMessage(crate::domain::entities::MessageId),
    SendMessage {
        content: String,
        reply_to: Option<MessageId>,
        attachments: Vec<std::path::PathBuf>,
    },
    StartTyping,
    OpenEditor {
        initial_content: String,
        message_id: Option<crate::domain::entities::MessageId>,
    },
    Paste,
    ToggleHelp,
}

pub struct ChatScreen;

impl ChatScreen {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ChatScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for ChatScreen {
    type State = ChatScreenState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let main_layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ]);
        let [header_area, content_area, footer_area] = main_layout.areas(area);

        render_header_bar(state, header_area, buf);
        render_content_area(state, content_area, buf);
        render_footer_bar(state, footer_area, buf);

        if state.show_file_explorer {
            render_explorer_popup(state, area, buf);
        }

        if state.focus == ChatFocus::ConfirmationModal {
            let modal = ConfirmationModal::new(
                "Delete Message",
                "Are you sure you want to delete this message?",
                state.theme,
            );
            modal.render(area, buf);
        }

        if state.show_help {
            render_help_popup(state, area, buf);
        }

        if !state.has_entered {
            let duration = state.pending_duration;
            state.pending_duration = Duration::ZERO;

            let overflow = state.entrance_effect.process(duration.into(), buf, area);
            if overflow.is_some() {
                state.has_entered = true;
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn render_help_popup(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::domain::keybinding::Action;
    use ratatui::layout::Alignment;
    use ratatui::style::Style;
    use ratatui::widgets::{Block, Borders, Cell, Clear, Row, Table};

    let width = 80;
    let height = 36;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    Clear.render(popup_area, buf);

    let bindings = [
        (
            "GLOBAL",
            vec![
                (Action::Quit, "Quit Application"),
                (Action::Logout, "Logout"),
                (Action::ToggleHelp, "Toggle Help"),
                (Action::FocusGuilds, "Focus Guilds"),
                (Action::FocusMessages, "Focus Messages"),
                (Action::FocusInput, "Focus Input"),
                (Action::ToggleGuildsTree, "Toggle Guilds Tree"),
            ],
        ),
        (
            "NAVIGATION",
            vec![
                (Action::NavigateLeft, "Left"),
                (Action::NavigateDown, "Down"),
                (Action::NavigateUp, "Up"),
                (Action::NavigateRight, "Right"),
                (Action::NextTab, "Next Pane"),
            ],
        ),
        (
            "MESSAGES",
            vec![
                (Action::Reply, "Reply"),
                (Action::ReplyNoMention, "Reply (no mention)"),
                (Action::EditMessage, "Edit Message"),
                (Action::DeleteMessage, "Delete Message"),
                (Action::CopyContent, "Copy Content"),
                (Action::YankId, "Copy Message ID"),
                (Action::OpenAttachments, "Open Attachment"),
                (Action::JumpToReply, "Jump to Reply"),
            ],
        ),
        (
            "INPUT",
            vec![
                (Action::ToggleFileExplorer, "Attachments"),
                (Action::SendMessage, "Send Message"),
                (Action::Paste, "Paste (Text/Image)"),
                (Action::OpenEditor, "Open External Editor"),
                (Action::ClearInput, "Clear Input"),
                (Action::Cancel, "Cancel Reply / Exit"),
            ],
        ),
    ];

    let format_key = |action: Action| -> String {
        state.registry.get(action).map_or_else(
            || "N/A".to_string(),
            |keys| {
                keys.iter()
                    .map(|k| {
                        use std::fmt::Write;
                        let mut s = String::new();
                        if k.modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                        {
                            s.push_str("Ctrl+");
                        }
                        if k.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                            s.push_str("Alt+");
                        }
                        if k.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            match k.code {
                                KeyCode::Char(c) if c.is_ascii_uppercase() => {}
                                _ => s.push_str("Shift+"),
                            }
                        }

                        match k.code {
                            KeyCode::Char(c) => s.push(c),
                            KeyCode::Enter => s.push_str("Enter"),
                            KeyCode::Esc => s.push_str("Esc"),
                            KeyCode::Tab => s.push_str("Tab"),
                            KeyCode::Up => s.push_str("Up"),
                            KeyCode::Down => s.push_str("Down"),
                            KeyCode::Left => s.push_str("Left"),
                            KeyCode::Right => s.push_str("Right"),
                            KeyCode::F(n) => {
                                let _ = write!(s, "F{n}");
                            }
                            _ => {
                                let _ = write!(s, "{k:?}");
                            }
                        }
                        s
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            },
        )
    };

    let mut rows = Vec::new();

    for (category, keys) in &bindings {
        rows.push(Row::new(vec![
            Cell::from(format!(" {category} ")).style(
                Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Cell::from(""),
        ]));

        for (action, desc) in keys {
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", format_key(*action)))
                    .style(Style::default().fg(ratatui::style::Color::Cyan)),
                Cell::from(*desc).style(Style::default().fg(ratatui::style::Color::Gray)),
            ]));
        }
        rows.push(Row::new(vec!["", ""]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(ratatui::style::Color::Cyan));

    let table = Table::new(
        rows,
        [Constraint::Percentage(40), Constraint::Percentage(60)],
    )
    .block(block)
    .column_spacing(2);

    Widget::render(table, popup_area, buf);
}

fn render_explorer_popup(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    if let Some(explorer) = &mut state.file_explorer {
        let content_area = if state.guilds_tree_visible {
            let chunks = Layout::horizontal([
                Constraint::Percentage(GUILDS_TREE_WIDTH_PERCENT),
                Constraint::Min(0),
            ])
            .split(area);
            chunks[1]
        } else {
            area
        };

        let base_area = if content_area.width < GUILDS_TREE_MIN_WIDTH {
            area
        } else {
            content_area
        };

        let width = base_area.width * 40 / 100;
        let height = base_area.height * 25 / 100;

        let x = base_area.x;
        let bottom_anchor = area.height.saturating_sub(4);
        let y = bottom_anchor.saturating_sub(height);

        let popup_area = Rect::new(x, y, width, height);
        explorer.render(popup_area, buf);
    }
}

fn render_header_bar(state: &ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::presentation::widgets::HeaderBarStyle;

    let style = HeaderBarStyle::from_theme(&state.theme);
    let header = HeaderBar::new(NAME, VERSION)
        .style(style)
        .connection_status(state.connection_status());
    Widget::render(header, area, buf);
}

fn render_footer_bar(state: &ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::presentation::widgets::FooterBarStyle;

    let focus_context = state.focus().to_focus_context();
    let message_count = state.message_pane_data().message_count();

    let right_info = if message_count > 0 {
        format!("{message_count} messages")
    } else {
        String::new()
    };

    let commands = state.get_commands(&state.registry);

    let style = FooterBarStyle::from_theme(&state.theme);
    let footer = FooterBar::new(&commands)
        .style(style)
        .focus_context(focus_context)
        .right_info(if right_info.is_empty() {
            None
        } else {
            Some(Box::leak(right_info.into_boxed_str()))
        });
    Widget::render(footer, area, buf);
}

fn render_content_area(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    if state.guilds_tree_visible {
        let content_layout = Layout::horizontal([
            Constraint::Percentage(GUILDS_TREE_WIDTH_PERCENT),
            Constraint::Min(0),
        ]);
        let [guilds_area, messages_area] = content_layout.areas(area);

        let guilds_area = if guilds_area.width < GUILDS_TREE_MIN_WIDTH {
            Rect {
                width: GUILDS_TREE_MIN_WIDTH,
                ..guilds_area
            }
        } else {
            guilds_area
        };

        render_guilds_tree(state, guilds_area, buf);
        render_messages_area(state, messages_area, buf);
    } else {
        render_messages_area(state, area, buf);
    }
}

fn render_guilds_tree(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::presentation::widgets::GuildsTreeStyle;

    let style = GuildsTreeStyle::from_theme(&state.theme);
    let (data, tree_state) = state.guilds_tree_parts_mut();
    let tree = GuildsTree::new(data).style(style);
    StatefulWidget::render(tree, area, buf, tree_state);
}

fn render_message_pane(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::presentation::widgets::MessagePaneStyle;

    let service = state.markdown_service.clone();
    let disable_user_colors = state.disable_user_colors;

    let inner_width = area.width.saturating_sub(2);
    state
        .message_pane_data
        .update_layout(inner_width, &service, state.theme.accent);

    let style = MessagePaneStyle::from_theme(&state.theme);
    let (data, pane_state) = state.message_pane_parts_mut();

    let pane = MessagePane::new(data, &service)
        .style(style)
        .with_disable_user_colors(disable_user_colors);
    StatefulWidget::render(pane, area, buf, pane_state);
}

fn render_message_input(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    use crate::presentation::widgets::MessageInputStyle;

    let style = MessageInputStyle::from_theme(&state.theme);
    MessageInput::new()
        .style(style)
        .render(state.message_input_parts_mut(), area, buf);
}

fn render_messages_area(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
    let [messages_area, input_area] = layout.areas(area);

    render_message_pane(state, messages_area, buf);
    render_message_input(state, input_area, buf);

    if state.autocomplete_service.state().active {
        let popup_height =
            u16::try_from(state.autocomplete_service.state().results.len().min(5)).unwrap_or(0) + 2;
        let popup_width = 30;
        let popup_area = Rect::new(
            input_area.x,
            input_area.y.saturating_sub(popup_height),
            popup_width,
            popup_height,
        );
        let mut autocomplete_state = state.autocomplete_service.state().clone();
        MentionPopup::new().render(popup_area, buf, &mut autocomplete_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_user() -> User {
        User::new("123", "testuser", "0", None, false, None)
    }

    #[test]
    fn test_chat_screen_state_creation() {
        let state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        assert_eq!(state.focus(), ChatFocus::GuildsTree);
        assert!(state.is_guilds_tree_visible());
        assert!(state.selected_channel().is_none());
    }

    #[test]
    fn test_focus_cycling() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        assert_eq!(state.focus(), ChatFocus::GuildsTree);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessagesList);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessageInput);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::GuildsTree);
    }

    #[test]
    fn test_toggle_guilds_tree() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        assert!(state.is_guilds_tree_visible());

        state.toggle_guilds_tree();
        assert!(!state.is_guilds_tree_visible());
        assert_ne!(state.focus(), ChatFocus::GuildsTree);
    }

    #[test]
    fn test_focus_skip_when_guilds_hidden() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );
        state.toggle_guilds_tree();
        state.set_focus(ChatFocus::MessagesList);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessageInput);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessagesList);
    }

    #[test]
    fn test_message_selection_focuses_list() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );
        let guilds = vec![
            Guild::new(1_u64, "Guild One"),
            Guild::new(2_u64, "Guild Two"),
        ];

        state.set_guilds(guilds);
        assert_eq!(state.guilds_tree_data().guilds().len(), 2);
    }

    #[test]
    fn test_channel_reset_on_guild_change() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        let guild_a = Guild::new(1_u64, "Guild A");
        let guild_b = Guild::new(2_u64, "Guild B");
        let channel_a1 = Channel::new(ChannelId(10), "Channel A1", ChannelKind::Text);

        state.set_guilds(vec![guild_a.clone(), guild_b.clone()]);
        state.set_channels(guild_a.id(), vec![channel_a1.clone()]);
        state.set_channels(guild_b.id(), vec![]);

        state.on_guild_selected(guild_a.id());
        assert_eq!(state.selected_guild(), Some(guild_a.id()));

        state.on_channel_selected(channel_a1.id());
        assert_eq!(
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id),
            Some(channel_a1.id())
        );
        assert_eq!(
            state.guilds_tree_data().active_channel_id(),
            Some(channel_a1.id())
        );

        state.on_guild_selected(guild_b.id());

        assert_eq!(state.selected_guild(), Some(guild_b.id()));
        assert_eq!(
            state.selected_channel(),
            None,
            "Selected channel should be cleared"
        );
        assert_eq!(
            state.guilds_tree_data().active_channel_id(),
            None,
            "Active channel ID in tree data should be cleared"
        );
    }

    #[test]
    fn test_reselecting_same_guild_preserves_channel() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        let guild_a = Guild::new(1_u64, "Guild A");
        let channel_a1 = Channel::new(ChannelId(10), "Channel A1", ChannelKind::Text);

        state.set_guilds(vec![guild_a.clone()]);
        state.set_channels(guild_a.id(), vec![channel_a1.clone()]);

        state.on_guild_selected(guild_a.id());
        state.on_channel_selected(channel_a1.id());

        assert_eq!(state.selected_guild(), Some(guild_a.id()));
        assert_eq!(
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id),
            Some(channel_a1.id())
        );

        state.on_guild_selected(guild_a.id());

        assert_eq!(state.selected_guild(), Some(guild_a.id()));
        assert_eq!(
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id),
            Some(channel_a1.id()),
            "Reselecting the same guild should not clear the active channel"
        );
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_cross_guild_channel_selection() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        let guild_a = Guild::new(1_u64, "Guild A");
        let guild_b = Guild::new(2_u64, "Guild B");
        let channel_a1 = Channel::new(ChannelId(10), "Channel A1", ChannelKind::Text);
        let channel_b1 = Channel::new(ChannelId(20), "Channel B1", ChannelKind::Text);

        state.set_guilds(vec![guild_a.clone(), guild_b.clone()]);
        state.set_channels(guild_a.id(), vec![channel_a1.clone()]);
        state.set_channels(guild_b.id(), vec![channel_b1.clone()]);

        state.on_guild_selected(guild_a.id());
        state.on_channel_selected(channel_a1.id());

        assert_eq!(state.selected_guild(), Some(guild_a.id()));
        assert_eq!(
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id),
            Some(channel_a1.id())
        );

        let result = state.on_channel_selected(channel_b1.id());

        assert!(
            result.is_some(),
            "Should return a result when selecting Channel B1"
        );
        assert_eq!(
            state.selected_guild(),
            Some(guild_b.id()),
            "Should switch to Guild B"
        );
        assert_eq!(
            state
                .selected_channel()
                .map(crate::domain::entities::Channel::id),
            Some(channel_b1.id()),
            "Should switch to Channel B1"
        );
    }

    #[test]
    fn test_chat_screen_state_creation_initial_focus() {
        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        assert_eq!(state.focus(), ChatFocus::GuildsTree);

        let guild = Guild::new(1_u64, "Guild A");
        let channel = Channel::new(ChannelId(10), "Channel A", ChannelKind::Text);
        state.set_guilds(vec![guild.clone()]);
        state.set_channels(guild.id(), vec![channel.clone()]);

        state.on_channel_selected(channel.id());

        assert_eq!(
            state.focus(),
            ChatFocus::MessagesList,
            "Focus should switch to MessagesList on channel selection"
        );
    }

    #[test]
    fn test_focus_to_context_conversion() {
        assert_eq!(
            ChatFocus::GuildsTree.to_focus_context(),
            FocusContext::GuildsTree
        );
        assert_eq!(
            ChatFocus::MessagesList.to_focus_context(),
            FocusContext::MessagesList
        );
        assert_eq!(
            ChatFocus::MessageInput.to_focus_context(),
            FocusContext::MessageInput
        );
    }

    #[test]
    fn test_escape_from_empty_message_list_focuses_tree() {
        use crossterm::event::KeyModifiers;

        let mut state = ChatScreenState::new(
            create_test_user(),
            Arc::new(MarkdownService::new()),
            UserCache::new(),
            false,
            Theme::new("Orange"),
        );

        state.focus_messages_list();
        assert_eq!(state.focus(), ChatFocus::MessagesList);

        state.message_pane_state.clear_selection();

        let esc_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

        let result = state.handle_key(esc_event);

        assert_eq!(result, ChatKeyResult::Consumed);
        assert_eq!(
            state.focus(),
            ChatFocus::GuildsTree,
            "Should return focus to Guilds Tree on Cancel from empty selection"
        );
    }
}
