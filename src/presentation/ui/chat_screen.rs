use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{StatefulWidget, Widget},
};
use std::sync::Arc;

use crate::application::services::markdown_service::MarkdownService;
use crate::domain::entities::{
    Channel, ChannelId, ChannelKind, Guild, GuildId, Message, MessageId, User,
};
use crate::presentation::widgets::{
    ConnectionStatus, FocusContext, FooterBar, GuildsTree, GuildsTreeAction, GuildsTreeData,
    GuildsTreeState, HeaderBar, MessageInput, MessageInputAction, MessageInputMode,
    MessageInputState, MessagePane, MessagePaneAction, MessagePaneData, MessagePaneState,
};
use crate::{NAME, VERSION};

const GUILDS_TREE_WIDTH_PERCENT: u16 = 25;
const GUILDS_TREE_MIN_WIDTH: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFocus {
    GuildsTree,
    MessagesList,
    MessageInput,
}

impl ChatFocus {
    const fn next(self, guilds_visible: bool) -> Self {
        if guilds_visible {
            match self {
                Self::GuildsTree => Self::MessagesList,
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput => Self::GuildsTree,
            }
        } else {
            match self {
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput | Self::GuildsTree => Self::MessagesList,
            }
        }
    }

    const fn previous(self, guilds_visible: bool) -> Self {
        if guilds_visible {
            match self {
                Self::GuildsTree => Self::MessageInput,
                Self::MessagesList => Self::GuildsTree,
                Self::MessageInput => Self::MessagesList,
            }
        } else {
            match self {
                Self::MessagesList => Self::MessageInput,
                Self::MessageInput | Self::GuildsTree => Self::MessagesList,
            }
        }
    }

    #[must_use]
    pub const fn to_focus_context(self) -> FocusContext {
        match self {
            Self::GuildsTree => FocusContext::GuildsTree,
            Self::MessagesList => FocusContext::MessagesList,
            Self::MessageInput => FocusContext::MessageInput,
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

pub struct ChatScreenState {
    user: User,
    focus: ChatFocus,
    guilds_tree_visible: bool,
    guilds_tree_state: GuildsTreeState,
    guilds_tree_data: GuildsTreeData,
    message_pane_state: MessagePaneState,
    message_pane_data: MessagePaneData,
    message_input_state: MessageInputState<'static>,
    selected_guild: Option<GuildId>,
    selected_channel: Option<Channel>,
    dm_channels: std::collections::HashMap<String, DmChannelInfo>,
    connection_status: ConnectionStatus,
    markdown_service: Arc<MarkdownService>,
}

impl ChatScreenState {
    #[must_use]
    pub fn new(user: User, markdown_service: Arc<MarkdownService>) -> Self {
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
            selected_guild: None,
            selected_channel: None,
            dm_channels: std::collections::HashMap::new(),
            connection_status: ConnectionStatus::Disconnected,
            markdown_service,
        }
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
        if let Some(result) = self.handle_global_key(key) {
            return result;
        }

        match self.focus {
            ChatFocus::GuildsTree => self.handle_guilds_tree_key(key),
            ChatFocus::MessagesList => self.handle_messages_list_key(key),
            ChatFocus::MessageInput => self.handle_message_input_key(key),
        }
    }

    fn handle_global_key(&mut self, key: KeyEvent) -> Option<ChatKeyResult> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(ChatKeyResult::Quit),
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some(ChatKeyResult::Logout),
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.focus_guilds_tree();
                Some(ChatKeyResult::Consumed)
            }
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                self.focus_messages_list();
                Some(ChatKeyResult::Consumed)
            }
            (KeyCode::Char('i'), KeyModifiers::CONTROL) => {
                self.focus_message_input();
                Some(ChatKeyResult::Consumed)
            }
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.focus_previous();
                Some(ChatKeyResult::Consumed)
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.focus_next();
                Some(ChatKeyResult::Consumed)
            }
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.toggle_guilds_tree();
                Some(ChatKeyResult::Consumed)
            }
            _ => None,
        }
    }

    fn handle_guilds_tree_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if let Some(action) = self.guilds_tree_state.handle_key(key) {
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
        ChatKeyResult::Consumed
    }

    fn handle_messages_list_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if let Some(action) = self
            .message_pane_state
            .handle_key(key, &self.message_pane_data)
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
                    return ChatKeyResult::EditMessage(message_id);
                }
                MessagePaneAction::Delete(message_id) => {
                    return ChatKeyResult::DeleteMessage(message_id);
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
                MessagePaneAction::LoadHistory => {
                    if let Some(channel_id) = self.message_pane_data.channel_id()
                        && let Some(first_msg) = self.message_pane_data.messages().front()
                    {
                        return ChatKeyResult::LoadHistory {
                            channel_id,
                            before_message_id: first_msg.id(),
                        };
                    }
                }
            }
        }
        ChatKeyResult::Consumed
    }

    fn handle_message_input_key(&mut self, key: KeyEvent) -> ChatKeyResult {
        if let Some(action) = self.message_input_state.handle_key(key) {
            match action {
                MessageInputAction::SendMessage { content, reply_to } => {
                    return ChatKeyResult::SendMessage { content, reply_to };
                }
                MessageInputAction::StartTyping => {
                    return ChatKeyResult::StartTyping;
                }
                MessageInputAction::CancelReply => {}
                MessageInputAction::ExitInput => {
                    self.focus_messages_list();
                }
                MessageInputAction::OpenEditor => {
                    return ChatKeyResult::OpenEditor;
                }
            }
        }
        ChatKeyResult::Consumed
    }

    fn on_channel_selected(&mut self, channel_id: ChannelId) -> Option<ChatKeyResult> {
        let guild_id = self
            .selected_guild
            .or_else(|| self.guilds_tree_data.find_guild_for_channel(channel_id));

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
            self.message_pane_data.set_channel(channel_id, channel_name);
            self.message_input_state.set_has_channel(true);
            self.message_input_state.clear();
            if let Some(topic) = topic {
                self.message_pane_data.set_channel_topic(Some(topic));
            }
            return Some(ChatKeyResult::LoadChannelMessages {
                channel_id,
                guild_id: Some(guild_id),
            });
        }
        None
    }

    fn on_guild_selected(&mut self, guild_id: GuildId) -> Option<ChatKeyResult> {
        self.selected_guild = Some(guild_id);
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
            self.message_input_state.set_has_channel(true);
            self.message_input_state.clear();

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

    pub fn add_message(&mut self, message: Message) {
        self.message_pane_data.add_message(message);
        self.message_pane_state.on_new_message();
    }

    pub fn prepend_messages(&mut self, new_messages: Vec<Message>) {
        if new_messages.is_empty() {
            return;
        }

        let width = self.message_pane_state.last_width();
        let pane = MessagePane::new(&self.message_pane_data, &self.markdown_service);
        let added_height: u16 = new_messages
            .iter()
            .map(|m| pane.calculate_message_height(m, width, &self.markdown_service))
            .sum();

        let added_count = self.message_pane_data.prepend_messages(new_messages);

        if added_count > 0 {
            self.message_pane_state
                .adjust_for_prepend(added_count, added_height);
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

    pub const fn message_pane_parts_mut(&mut self) -> (&MessagePaneData, &mut MessagePaneState) {
        (&self.message_pane_data, &mut self.message_pane_state)
    }

    pub fn start_reply(&mut self, message_id: MessageId, author_name: String) {
        self.message_input_state
            .start_reply(message_id, author_name);
        self.focus_message_input();
    }

    pub fn cancel_reply(&mut self) {
        self.message_input_state.cancel_reply();
    }

    pub fn clear_input(&mut self) {
        self.message_input_state.clear();
    }

    pub fn get_reply_author(&self, message_id: MessageId) -> Option<String> {
        self.message_pane_data
            .messages()
            .iter()
            .find(|m| m.id() == message_id)
            .map(|m| m.author().display_name().clone())
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
    pub fn message_input_reply_info(&self) -> Option<(MessageId, String)> {
        match self.message_input_state.mode() {
            MessageInputMode::Reply { message_id, author } => Some((*message_id, author.clone())),
            MessageInputMode::Normal => None,
        }
    }

    /// Set the message input content.
    pub fn set_message_input_content(&mut self, content: &str) {
        self.message_input_state.set_content(content);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatKeyResult {
    Consumed,
    Quit,
    Logout,
    CopyToClipboard(String),
    LoadGuildChannels(GuildId),
    LoadChannelMessages {
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    },
    LoadDmMessages {
        channel_id: ChannelId,
        recipient_name: String,
    },
    ReplyToMessage {
        message_id: crate::domain::entities::MessageId,
        mention: bool,
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
    },
    StartTyping,
    OpenEditor,
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
    }
}

fn render_header_bar(state: &ChatScreenState, area: Rect, buf: &mut Buffer) {
    let header = HeaderBar::new(NAME, VERSION).connection_status(state.connection_status());
    Widget::render(header, area, buf);
}

fn render_footer_bar(state: &ChatScreenState, area: Rect, buf: &mut Buffer) {
    let focus_context = state.focus().to_focus_context();
    let message_count = state.message_pane_data().message_count();

    let right_info = if message_count > 0 {
        format!("{message_count} messages")
    } else {
        String::new()
    };

    let footer =
        FooterBar::new()
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
    let (data, tree_state) = state.guilds_tree_parts_mut();
    let tree = GuildsTree::new(data);
    StatefulWidget::render(tree, area, buf, tree_state);
}

fn render_message_pane(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let service = state.markdown_service.clone();
    let (data, pane_state) = state.message_pane_parts_mut();
    let pane = MessagePane::new(data, &service);
    StatefulWidget::render(pane, area, buf, pane_state);
}

fn render_message_input(state: &ChatScreenState, area: Rect, buf: &mut Buffer) {
    MessageInput::render(state.message_input_state(), area, buf);
}

fn render_messages_area(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
    let [messages_area, input_area] = layout.areas(area);

    render_message_pane(state, messages_area, buf);
    render_message_input(state, input_area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_user() -> User {
        User::new("123", "testuser", "0", None, false)
    }

    #[test]
    fn test_chat_screen_state_creation() {
        let state = ChatScreenState::new(create_test_user(), Arc::new(MarkdownService::new()));

        assert_eq!(state.focus(), ChatFocus::GuildsTree);
        assert!(state.is_guilds_tree_visible());
        assert!(state.selected_channel().is_none());
    }

    #[test]
    fn test_focus_cycling() {
        let mut state = ChatScreenState::new(create_test_user(), Arc::new(MarkdownService::new()));

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
        let mut state = ChatScreenState::new(create_test_user(), Arc::new(MarkdownService::new()));

        assert!(state.is_guilds_tree_visible());

        state.toggle_guilds_tree();
        assert!(!state.is_guilds_tree_visible());
        assert_ne!(state.focus(), ChatFocus::GuildsTree);
    }

    #[test]
    fn test_focus_skip_when_guilds_hidden() {
        let mut state = ChatScreenState::new(create_test_user(), Arc::new(MarkdownService::new()));
        state.toggle_guilds_tree();
        state.set_focus(ChatFocus::MessagesList);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessageInput);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessagesList);
    }

    #[test]
    fn test_set_guilds() {
        let mut state = ChatScreenState::new(create_test_user(), Arc::new(MarkdownService::new()));
        let guilds = vec![
            Guild::new(1_u64, "Guild One"),
            Guild::new(2_u64, "Guild Two"),
        ];

        state.set_guilds(guilds);
        assert_eq!(state.guilds_tree_data().guilds().len(), 2);
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
}
