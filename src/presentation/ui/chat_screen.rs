//! Chat screen with guilds tree and messages pane.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use crate::domain::entities::{Channel, ChannelId, Guild, GuildId, Message, User};
use crate::presentation::widgets::{
    GuildsTree, GuildsTreeAction, GuildsTreeData, GuildsTreeState, MessagePane, MessagePaneAction,
    MessagePaneData, MessagePaneState,
};

const GUILDS_TREE_WIDTH_PERCENT: u16 = 25;
const GUILDS_TREE_MIN_WIDTH: u16 = 20;

/// Focus target within the chat screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFocus {
    /// Guilds/channels tree panel.
    GuildsTree,
    /// Messages list panel.
    MessagesList,
    /// Message input field.
    MessageInput,
}

impl ChatFocus {
    fn next(self, guilds_visible: bool) -> Self {
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

    fn previous(self, guilds_visible: bool) -> Self {
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
}

/// State for the chat screen.
pub struct ChatScreenState {
    user: User,
    focus: ChatFocus,
    guilds_tree_visible: bool,
    guilds_tree_state: GuildsTreeState,
    guilds_tree_data: GuildsTreeData,
    message_pane_state: MessagePaneState,
    message_pane_data: MessagePaneData,
    selected_guild: Option<GuildId>,
    selected_channel: Option<Channel>,
}

impl ChatScreenState {
    /// Creates a new chat screen state for the given user.
    #[must_use]
    pub fn new(user: User) -> Self {
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
            selected_guild: None,
            selected_channel: None,
        }
    }

    /// Returns the current user.
    #[must_use]
    pub fn user(&self) -> &User {
        &self.user
    }

    /// Returns the current focus target.
    #[must_use]
    pub fn focus(&self) -> ChatFocus {
        self.focus
    }

    /// Returns whether the guilds tree is visible.
    #[must_use]
    pub fn is_guilds_tree_visible(&self) -> bool {
        self.guilds_tree_visible
    }

    /// Returns the currently selected channel.
    #[must_use]
    pub fn selected_channel(&self) -> Option<&Channel> {
        self.selected_channel.as_ref()
    }

    /// Sets the guilds list.
    pub fn set_guilds(&mut self, guilds: Vec<Guild>) {
        self.guilds_tree_data.set_guilds(guilds);
    }

    /// Sets the channels for a specific guild.
    pub fn set_channels(&mut self, guild_id: GuildId, channels: Vec<Channel>) {
        self.guilds_tree_data.set_channels(guild_id, channels);
    }

    /// Sets the DM users list.
    pub fn set_dm_users(&mut self, users: Vec<(String, String)>) {
        self.guilds_tree_data.set_dm_users(users);
    }

    /// Toggles the visibility of the guilds tree.
    pub fn toggle_guilds_tree(&mut self) {
        self.guilds_tree_visible = !self.guilds_tree_visible;
        if !self.guilds_tree_visible && self.focus == ChatFocus::GuildsTree {
            self.focus_next();
        }
    }

    /// Focuses the guilds tree panel.
    pub fn focus_guilds_tree(&mut self) {
        if self.guilds_tree_visible {
            self.set_focus(ChatFocus::GuildsTree);
        }
    }

    /// Focuses the messages list panel.
    pub fn focus_messages_list(&mut self) {
        self.set_focus(ChatFocus::MessagesList);
    }

    /// Focuses the message input field.
    pub fn focus_message_input(&mut self) {
        self.set_focus(ChatFocus::MessageInput);
    }

    /// Moves focus to the next panel.
    pub fn focus_next(&mut self) {
        let new_focus = self.focus.next(self.guilds_tree_visible);
        self.set_focus(new_focus);
    }

    /// Moves focus to the previous panel.
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
    }

    /// Handles a key event and returns the result.
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
                GuildsTreeAction::SelectDirectMessage(_user_id) => {}
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
            }
        }
        ChatKeyResult::Consumed
    }

    fn handle_message_input_key(&mut self, _key: KeyEvent) -> ChatKeyResult {
        let _ = self; // Suppress unused_self warning until implementation
        ChatKeyResult::Consumed
    }

    fn on_channel_selected(&mut self, channel_id: ChannelId) -> Option<ChatKeyResult> {
        if let Some(guild_id) = self.selected_guild
            && let Some(channels) = self.guilds_tree_data.channels(guild_id)
            && let Some(channel) = channels.iter().find(|c| c.id() == channel_id)
        {
            self.selected_channel = Some(channel.clone());
            let channel_name = channel.display_name();
            self.message_pane_data.set_channel(channel_id, channel_name);
            return Some(ChatKeyResult::LoadChannelMessages(channel_id));
        }
        None
    }

    fn on_guild_selected(&mut self, guild_id: GuildId) -> Option<ChatKeyResult> {
        self.selected_guild = Some(guild_id);

        // Check if channels are already loaded for this guild
        if self.guilds_tree_data.channels(guild_id).is_none() {
            // Request lazy loading of channels
            return Some(ChatKeyResult::LoadGuildChannels(guild_id));
        }

        None
    }

    /// Returns a reference to the guilds tree data.
    #[must_use]
    pub fn guilds_tree_data(&self) -> &GuildsTreeData {
        &self.guilds_tree_data
    }

    /// Returns a mutable reference to the guilds tree state.
    pub fn guilds_tree_state_mut(&mut self) -> &mut GuildsTreeState {
        &mut self.guilds_tree_state
    }

    /// Returns mutable references to both tree data and state.
    pub fn guilds_tree_parts_mut(&mut self) -> (&GuildsTreeData, &mut GuildsTreeState) {
        (&self.guilds_tree_data, &mut self.guilds_tree_state)
    }

    /// Sets the messages for the current channel.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.message_pane_data.set_messages(messages);
    }

    /// Adds a new message to the current channel.
    pub fn add_message(&mut self, message: Message) {
        self.message_pane_data.add_message(message);
        self.message_pane_state.on_new_message();
    }

    /// Updates an existing message in the current channel.
    pub fn update_message(&mut self, message: Message) {
        self.message_pane_data.update_message(message);
    }

    /// Removes a message from the current channel.
    pub fn remove_message(&mut self, message_id: crate::domain::entities::MessageId) {
        self.message_pane_data.remove_message(message_id);
    }

    /// Sets an error state for message loading.
    pub fn set_message_error(&mut self, error: String) {
        self.message_pane_data.set_error(error);
    }

    /// Returns a reference to the message pane data.
    #[must_use]
    pub fn message_pane_data(&self) -> &MessagePaneData {
        &self.message_pane_data
    }

    /// Returns mutable references to message pane data and state.
    pub fn message_pane_parts_mut(&mut self) -> (&MessagePaneData, &mut MessagePaneState) {
        (&self.message_pane_data, &mut self.message_pane_state)
    }
}

/// Result of handling a key event in the chat screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatKeyResult {
    /// Event was consumed, no further action needed.
    Consumed,
    /// User requested to quit.
    Quit,
    /// User requested to log out.
    Logout,
    /// User requested to copy text to clipboard.
    CopyToClipboard(String),
    /// Request to load channels for a guild (lazy loading).
    LoadGuildChannels(GuildId),
    /// Request to load messages for a channel.
    LoadChannelMessages(ChannelId),
    /// Request to reply to a message.
    ReplyToMessage {
        /// The message ID to reply to.
        message_id: crate::domain::entities::MessageId,
        /// Whether to mention the author.
        mention: bool,
    },
    /// Request to edit a message.
    EditMessage(crate::domain::entities::MessageId),
    /// Request to delete a message.
    DeleteMessage(crate::domain::entities::MessageId),
    /// Request to open attachments for a message.
    OpenAttachments(crate::domain::entities::MessageId),
    /// Request to jump to a specific message.
    JumpToMessage(crate::domain::entities::MessageId),
}

/// Chat screen widget.
pub struct ChatScreen;

impl ChatScreen {
    /// Creates a new chat screen widget.
    #[must_use]
    pub fn new() -> Self {
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
}

fn render_guilds_tree(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let (data, tree_state) = state.guilds_tree_parts_mut();
    let tree = GuildsTree::new(data);
    StatefulWidget::render(tree, area, buf, tree_state);
}

fn render_message_pane(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let (data, pane_state) = state.message_pane_parts_mut();
    let pane = MessagePane::new(data);
    StatefulWidget::render(pane, area, buf, pane_state);
}

fn render_input_placeholder(focus: ChatFocus, has_channel: bool, area: Rect, buf: &mut Buffer) {
    let is_focused = focus == ChatFocus::MessageInput;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let placeholder = if has_channel {
        "Type a message..."
    } else {
        "Select a channel first"
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let paragraph = Paragraph::new(Span::styled(
        placeholder,
        Style::default().fg(Color::DarkGray),
    ))
    .block(block);

    paragraph.render(area, buf);
}

fn render_messages_area(state: &mut ChatScreenState, area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
    let [messages_area, input_area] = layout.areas(area);

    render_message_pane(state, messages_area, buf);

    let focus = state.focus;
    let has_channel = state.selected_channel().is_some();
    render_input_placeholder(focus, has_channel, input_area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_user() -> User {
        User::new("123", "testuser", "0", None, false)
    }

    #[test]
    fn test_chat_screen_state_creation() {
        let state = ChatScreenState::new(create_test_user());

        assert_eq!(state.focus(), ChatFocus::GuildsTree);
        assert!(state.is_guilds_tree_visible());
        assert!(state.selected_channel().is_none());
    }

    #[test]
    fn test_focus_cycling() {
        let mut state = ChatScreenState::new(create_test_user());

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
        let mut state = ChatScreenState::new(create_test_user());

        assert!(state.is_guilds_tree_visible());

        state.toggle_guilds_tree();
        assert!(!state.is_guilds_tree_visible());
        assert_ne!(state.focus(), ChatFocus::GuildsTree);
    }

    #[test]
    fn test_focus_skip_when_guilds_hidden() {
        let mut state = ChatScreenState::new(create_test_user());
        state.toggle_guilds_tree();
        state.set_focus(ChatFocus::MessagesList);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessageInput);

        state.focus_next();
        assert_eq!(state.focus(), ChatFocus::MessagesList);
    }

    #[test]
    fn test_set_guilds() {
        let mut state = ChatScreenState::new(create_test_user());
        let guilds = vec![
            Guild::new(1_u64, "Guild One"),
            Guild::new(2_u64, "Guild Two"),
        ];

        state.set_guilds(guilds);
        assert_eq!(state.guilds_tree_data().guilds().len(), 2);
    }
}
