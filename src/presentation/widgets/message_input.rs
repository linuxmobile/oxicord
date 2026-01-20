use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use tui_textarea::TextArea;

use crate::domain::entities::MessageId;
use crate::domain::keybinding::Action;
use crate::presentation::commands::CommandRegistry;

const MAX_MESSAGE_LENGTH: usize = 2000;
const PLACEHOLDER_TEXT: &str = "Type a message...";
const PLACEHOLDER_NO_CHANNEL: &str = "Select a channel first";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MessageInputMode {
    #[default]
    Normal,
    Reply {
        message_id: MessageId,
        author: String,
    },
    Editing {
        message_id: MessageId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageInputAction {
    SendMessage {
        content: String,
        reply_to: Option<MessageId>,
        attachments: Vec<PathBuf>,
    },
    EditMessage {
        message_id: MessageId,
        content: String,
    },
    StartTyping,
    CancelReply,
    ExitInput,
    OpenEditor,
}

pub struct MessageInputState<'a> {
    textarea: TextArea<'a>,
    focused: bool,
    mode: MessageInputMode,
    has_channel: bool,
    attachments: Vec<PathBuf>,
    scroll_offset: usize,
}

impl MessageInputState<'_> {
    #[must_use]
    pub fn new() -> Self {
        let textarea = TextArea::default();

        Self {
            textarea,
            focused: false,
            mode: MessageInputMode::Normal,
            has_channel: false,
            attachments: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn add_attachment(&mut self, path: PathBuf) {
        self.attachments.push(path);
    }

    pub fn clear_attachments(&mut self) {
        self.attachments.clear();
    }

    pub fn attachments(&self) -> &[PathBuf] {
        &self.attachments
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_has_channel(&mut self, has_channel: bool) {
        self.has_channel = has_channel;
        self.update_placeholder();
    }

    #[must_use]
    pub const fn has_channel(&self) -> bool {
        self.has_channel
    }

    #[must_use]
    pub fn value(&self) -> String {
        self.textarea.lines().join("\n")
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    #[must_use]
    pub const fn mode(&self) -> &MessageInputMode {
        &self.mode
    }

    #[must_use]
    pub fn is_replying(&self) -> bool {
        matches!(self.mode, MessageInputMode::Reply { .. })
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        matches!(self.mode, MessageInputMode::Editing { .. })
    }

    pub fn start_reply(&mut self, message_id: MessageId, author: String) {
        self.mode = MessageInputMode::Reply { message_id, author };
    }

    pub fn start_edit(&mut self, message_id: MessageId, content: &str) {
        self.mode = MessageInputMode::Editing { message_id };
        self.set_content(content);
    }

    pub fn reset_mode(&mut self) {
        self.mode = MessageInputMode::Normal;
    }

    pub fn clear(&mut self) {
        self.textarea.select_all();
        self.textarea.cut();
        self.mode = MessageInputMode::Normal;
    }

    pub fn set_content(&mut self, content: &str) {
        self.textarea.select_all();
        self.textarea.cut();
        self.textarea.insert_str(content);
    }

    fn update_placeholder(&mut self) {
        let placeholder = if self.has_channel {
            PLACEHOLDER_TEXT
        } else {
            PLACEHOLDER_NO_CHANNEL
        };
        self.textarea.set_placeholder_text(placeholder);
    }

    pub fn get_cursor_index(&self) -> usize {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let mut index = 0;
        for i in 0..row {
            if let Some(line) = lines.get(i) {
                index += line.len() + 1;
            }
        }
        index + col
    }

    pub fn insert_mention(&mut self, trigger_index: usize, user_id: &str) {
        let content = self.value();
        let mention = format!("<@{user_id}> ");

        if trigger_index >= content.len() {
            return;
        }

        let cursor_idx = self.get_cursor_index();

        if cursor_idx < trigger_index {
            return;
        }
        if cursor_idx > content.len() {
            return;
        }

        let prefix = &content[..trigger_index];
        let suffix = &content[cursor_idx..];

        let new_content = format!("{prefix}{mention}{suffix}");

        self.set_content(&new_content);
    }

    fn enforce_message_limit(&mut self) {
        let content = self.value();
        if content.len() > MAX_MESSAGE_LENGTH {
            let truncated: String = content.chars().take(MAX_MESSAGE_LENGTH).collect();
            self.set_content(&truncated);
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        registry: &CommandRegistry,
    ) -> Option<MessageInputAction> {
        if !self.has_channel {
            if key.code == KeyCode::Esc {
                return Some(MessageInputAction::ExitInput);
            }
            return None;
        }

        match if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
            Some(Action::SendMessage)
        } else {
            registry.find_action(key)
        } {
            Some(Action::Cancel) => {
                if self.is_replying() || self.is_editing() {
                    self.reset_mode();
                    Some(MessageInputAction::CancelReply)
                } else {
                    Some(MessageInputAction::ExitInput)
                }
            }
            Some(Action::SendMessage) => {
                let content = self.value();
                if content.trim().is_empty() && self.attachments.is_empty() {
                    return None;
                }

                if let MessageInputMode::Editing { message_id } = &self.mode {
                    let message_id = *message_id;
                    self.clear();
                    return Some(MessageInputAction::EditMessage {
                        message_id,
                        content,
                    });
                }

                let reply_to = match &self.mode {
                    MessageInputMode::Reply { message_id, .. } => Some(*message_id),
                    _ => None,
                };
                let attachments = self.attachments.clone();
                self.clear();
                self.clear_attachments();
                Some(MessageInputAction::SendMessage {
                    content,
                    reply_to,
                    attachments,
                })
            }
            Some(Action::OpenEditor) => Some(MessageInputAction::OpenEditor),
            Some(Action::ClearInput) => {
                self.clear();
                None
            }
            _ => {
                if let KeyCode::Char(c) = key.code
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                {
                    self.textarea.insert_char(c);
                } else if key.code == KeyCode::Backspace {
                    self.textarea.delete_char();
                } else if key.code == KeyCode::Delete {
                    self.textarea.delete_next_char();
                } else if key.code == KeyCode::Left {
                    self.textarea.move_cursor(tui_textarea::CursorMove::Back);
                } else if key.code == KeyCode::Right {
                    self.textarea.move_cursor(tui_textarea::CursorMove::Forward);
                } else if key.code == KeyCode::Up {
                    self.textarea.move_cursor(tui_textarea::CursorMove::Up);
                } else if key.code == KeyCode::Down {
                    self.textarea.move_cursor(tui_textarea::CursorMove::Down);
                } else if key.code == KeyCode::Enter {
                    self.textarea.insert_newline();
                }

                let was_empty = self.is_empty();
                self.enforce_message_limit();

                if !was_empty || !self.is_empty() {
                    Some(MessageInputAction::StartTyping)
                } else {
                    None
                }
            }
        }
    }

    fn setup_block(&self) -> Block<'static> {
        let border_color = if self.focused {
            Color::Cyan
        } else {
            Color::Gray
        };

        let border_style = Style::default().fg(border_color);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        match &self.mode {
            MessageInputMode::Reply { author, .. } => {
                let reply_title = format!(" Replying to @{author} ");
                block = block.title(reply_title).title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::ITALIC),
                );
            }
            MessageInputMode::Editing { .. } => {
                let edit_title = " Editing Message ";
                block = block.title(edit_title).title_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
            MessageInputMode::Normal => {}
        }

        if !self.attachments.is_empty() {
            let attachments_title = format!(" {} Attachments ", self.attachments.len());
            block = block.title(attachments_title).title_style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            );
        }

        block
    }

    /// Render using manual rendering instead of tui-textarea's widget
    /// to avoid ratatui version incompatibility (project: 0.30, tui-textarea: 0.29)
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let block = self.setup_block();
        let inner = block.inner(area);
        block.render(area, buf);

        let width = inner.width as usize;
        if width == 0 {
            return;
        }

        let (_, cursor_col) = self.textarea.cursor();

        if cursor_col >= self.scroll_offset + width {
            self.scroll_offset = cursor_col - width + 1;
        } else if cursor_col < self.scroll_offset {
            self.scroll_offset = cursor_col;
        }

        let content = self.value();
        let display_content = if content.is_empty() {
            if self.has_channel {
                PLACEHOLDER_TEXT.to_string()
            } else {
                PLACEHOLDER_NO_CHANNEL.to_string()
            }
        } else {
            content
        };

        let text_style = if self.value().is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let (cursor_row, _) = self.textarea.cursor();

        let lines: Vec<&str> = display_content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if i >= inner.height as usize {
                break;
            }
            let y = inner.y + u16::try_from(i).unwrap_or(0);

            for (j, ch) in line
                .chars()
                .enumerate()
                .skip(self.scroll_offset)
                .take(width)
            {
                let visual_x = j - self.scroll_offset;
                let x = inner.x + u16::try_from(visual_x).unwrap_or(0);

                let style = if self.focused
                    && !self.value().is_empty()
                    && i == cursor_row
                    && j == cursor_col
                {
                    Style::default().bg(Color::White).fg(Color::Black)
                } else {
                    text_style
                };

                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(&ch.to_string());
                    cell.set_style(style);
                }
            }

            if self.focused
                && !self.value().is_empty()
                && i == cursor_row
                && cursor_col >= line.len()
                && cursor_col >= self.scroll_offset
                && cursor_col < self.scroll_offset + width
            {
                let visual_x = cursor_col - self.scroll_offset;
                let x = inner.x + u16::try_from(visual_x).unwrap_or(0);

                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
                }
            }
        }

        if self.focused && !self.value().is_empty() && cursor_row >= lines.len() {
            let y = inner.y + u16::try_from(cursor_row).unwrap_or(0);
            if y < inner.y + inner.height
                && cursor_col >= self.scroll_offset
                && cursor_col < self.scroll_offset + width
            {
                let visual_x = cursor_col - self.scroll_offset;
                let x = inner.x + u16::try_from(visual_x).unwrap_or(0);
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
                }
            }
        }
    }
}

impl Default for MessageInputState<'_> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MessageInputStyle {
    pub border_style: Style,
    pub border_style_focused: Style,
    pub text_style: Style,
    pub placeholder_style: Style,
    pub cursor_style: Style,
    pub reply_indicator_style: Style,
}

impl Default for MessageInputStyle {
    fn default() -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(Color::Cyan),
            text_style: Style::default().fg(Color::White),
            placeholder_style: Style::default().fg(Color::DarkGray),
            cursor_style: Style::default().bg(Color::White).fg(Color::Black),
            reply_indicator_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        }
    }
}

pub struct MessageInput;

impl MessageInput {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn render(state: &mut MessageInputState<'_>, area: Rect, buf: &mut Buffer) {
        state.render(area, buf);
    }
}

impl Default for MessageInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_message_input_state_creation() {
        let state = MessageInputState::new();
        assert!(state.is_empty());
        assert!(!state.is_focused());
        assert!(!state.is_replying());
        assert!(!state.is_editing());
    }

    #[test]
    fn test_input_via_key_handling() {
        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_focused(true);

        state.handle_key(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            &registry,
        );

        assert_eq!(state.value(), "hi");
    }

    #[test]
    fn test_clear() {
        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &registry,
        );

        state.clear();
        assert!(state.is_empty());
    }

    #[test]
    fn test_reply_mode() {
        let mut state = MessageInputState::new();
        state.start_reply(MessageId(123), "testuser".to_string());
        assert!(state.is_replying());
        state.reset_mode();
        assert!(!state.is_replying());
    }

    #[test]
    fn test_edit_mode() {
        let mut state = MessageInputState::new();
        state.start_edit(MessageId(123), "old content");
        assert!(state.is_editing());
        assert_eq!(state.value(), "old content");
        state.reset_mode();
        assert!(!state.is_editing());
        assert_eq!(state.value(), "old content");
    }

    #[test]
    fn test_send_message_clears_state() {
        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            &registry,
        );
        state.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &registry,
        );
        state.start_reply(MessageId(123), "user".to_string());

        let action = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &registry);

        assert!(matches!(
            action,
            Some(MessageInputAction::SendMessage {
                content,
                reply_to: Some(_),
                attachments: _
            }) if content == "test"
        ));
        assert!(state.is_empty());
        assert!(!state.is_replying());
    }

    #[test]
    fn test_enter_and_shift_enter() {
        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_content("hello");

        let action = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
            &registry,
        );
        assert!(matches!(action, Some(MessageInputAction::StartTyping)));
        assert_eq!(state.value(), "hello\n");

        let action = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &registry);
        assert!(matches!(
            action,
            Some(MessageInputAction::SendMessage { .. })
        ));
        assert!(state.is_empty());
    }
}
