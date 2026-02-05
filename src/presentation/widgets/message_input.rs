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
use crate::presentation::theme::Theme;
use unicode_width::UnicodeWidthChar;

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
        mention: bool,
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
    Paste,
}

pub struct MessageInputState<'a> {
    textarea: TextArea<'a>,
    focused: bool,
    mode: MessageInputMode,
    has_channel: bool,
    attachments: Vec<PathBuf>,
    scroll_offset: usize,
    last_width: usize,
    mentions: std::collections::HashMap<String, String>,
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
            last_width: 0,
            mentions: std::collections::HashMap::new(),
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
    pub fn message_content(&self) -> String {
        let mut content = self.value();
        let mut sorted_mentions: Vec<_> = self.mentions.iter().collect();
        sorted_mentions.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));

        for (name, id) in sorted_mentions {
            content = content.replace(name, &format!("<@{id}>"));
        }
        content
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

    pub fn start_reply(&mut self, message_id: MessageId, author: String, mention: bool) {
        self.mode = MessageInputMode::Reply {
            message_id,
            author,
            mention,
        };
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
        self.mentions.clear();
    }

    pub fn set_content(&mut self, content: &str) {
        self.textarea.select_all();
        self.textarea.cut();
        self.textarea.insert_str(content);
    }

    pub fn insert_text_at_cursor(&mut self, content: &str) {
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

    fn get_visual_info(&self, width: usize) -> (Vec<String>, usize, usize) {
        if width == 0 {
            let logical_lines: Vec<String> = self.textarea.lines().to_vec();
            let (row, col) = self.textarea.cursor();
            return (logical_lines, row, col);
        }

        let mut visual_lines = Vec::new();
        let mut visual_cursor_row = 0;
        let mut visual_cursor_col = 0;

        let (cursor_row, cursor_col) = self.textarea.cursor();
        let logical_lines = self.textarea.lines();

        for (i, line) in logical_lines.iter().enumerate() {
            let is_cursor_line = i == cursor_row;

            if line.is_empty() {
                if is_cursor_line {
                    visual_cursor_row = visual_lines.len();
                    visual_cursor_col = 0;
                }
                visual_lines.push(String::new());
                continue;
            }

            let mut current_line = String::new();
            let mut current_width = 0;

            for (j, ch) in line.chars().enumerate() {
                let ch_width = ch.width().unwrap_or(0);

                if current_width + ch_width > width {
                    visual_lines.push(current_line.clone());
                    current_line.clear();
                    current_width = 0;
                }

                if is_cursor_line && j == cursor_col {
                    visual_cursor_row = visual_lines.len();
                    visual_cursor_col = current_width;
                }

                current_line.push(ch);
                current_width += ch_width;
            }

            if is_cursor_line && cursor_col == line.chars().count() {
                if current_width >= width {
                    visual_lines.push(current_line.clone());
                    current_line.clear();
                    visual_cursor_row = visual_lines.len();
                    visual_cursor_col = 0;
                } else {
                    visual_cursor_row = visual_lines.len();
                    visual_cursor_col = current_width;
                }
            }

            visual_lines.push(current_line);
        }

        (visual_lines, visual_cursor_row, visual_cursor_col)
    }

    fn get_logical_pos(
        &self,
        target_v_row: usize,
        target_v_col: usize,
        width: usize,
    ) -> (usize, usize) {
        let mut v_row_counter = 0;

        for (l_row, line) in self.textarea.lines().iter().enumerate() {
            if line.is_empty() {
                if v_row_counter == target_v_row {
                    return (l_row, 0);
                }
                v_row_counter += 1;
                continue;
            }

            let mut current_v_width = 0;
            let mut line_start_char_idx = 0;

            for (char_idx, ch) in line.chars().enumerate() {
                let ch_width = ch.width().unwrap_or(0);

                if current_v_width + ch_width > width {
                    if v_row_counter == target_v_row {
                        let mut run_width = 0;
                        for (k, k_char) in line
                            .chars()
                            .enumerate()
                            .skip(line_start_char_idx)
                            .take(char_idx - line_start_char_idx)
                        {
                            let w = k_char.width().unwrap_or(0);
                            if run_width + w > target_v_col {
                                return (l_row, k);
                            }
                            run_width += w;
                        }
                        return (l_row, char_idx);
                    }

                    v_row_counter += 1;
                    current_v_width = 0;
                    line_start_char_idx = char_idx;
                }

                current_v_width += ch_width;
            }

            if v_row_counter == target_v_row {
                let mut run_width = 0;
                for (k, k_char) in line.chars().enumerate().skip(line_start_char_idx) {
                    let w = k_char.width().unwrap_or(0);
                    if run_width + w > target_v_col {
                        return (l_row, k);
                    }
                    run_width += w;
                }
                return (l_row, line.chars().count());
            }
            v_row_counter += 1;
        }

        (0, 0)
    }

    pub fn insert_mention(&mut self, trigger_index: usize, resolved_name: &str, user_id: &str) {
        let content = self.value();
        let mention_text = format!("@{resolved_name} ");
        self.mentions
            .insert(format!("@{resolved_name}"), user_id.to_string());

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

        let new_content = format!("{prefix}{mention_text}{suffix}");

        self.set_content(&new_content);
    }

    fn move_cursor_up(&mut self) {
        let width = self.last_width;
        if width == 0 {
            self.textarea.move_cursor(tui_textarea::CursorMove::Up);
            return;
        }

        let (_, v_row, v_col) = self.get_visual_info(width);

        if v_row == 0 {
            return;
        }

        let target_v_row = v_row - 1;
        let (l_row, l_col) = self.get_logical_pos(target_v_row, v_col, width);
        self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            u16::try_from(l_row).unwrap_or(u16::MAX),
            u16::try_from(l_col).unwrap_or(u16::MAX),
        ));
    }

    fn move_cursor_down(&mut self) {
        let width = self.last_width;
        if width == 0 {
            self.textarea.move_cursor(tui_textarea::CursorMove::Down);
            return;
        }

        let (visual_lines, v_row, v_col) = self.get_visual_info(width);

        if v_row >= visual_lines.len().saturating_sub(1) {
            return;
        }

        let target_v_row = v_row + 1;
        let (l_row, l_col) = self.get_logical_pos(target_v_row, v_col, width);
        self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            u16::try_from(l_row).unwrap_or(u16::MAX),
            u16::try_from(l_col).unwrap_or(u16::MAX),
        ));
    }

    fn move_cursor_start(&mut self) {
        let width = self.last_width;
        if width == 0 {
            self.textarea.move_cursor(tui_textarea::CursorMove::Head);
            return;
        }

        let (_, v_row, _) = self.get_visual_info(width);
        let (l_row, l_col) = self.get_logical_pos(v_row, 0, width);
        self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            u16::try_from(l_row).unwrap_or(u16::MAX),
            u16::try_from(l_col).unwrap_or(u16::MAX),
        ));
    }

    fn move_cursor_end(&mut self) {
        let width = self.last_width;
        if width == 0 {
            self.textarea.move_cursor(tui_textarea::CursorMove::End);
            return;
        }

        let (visual_lines, v_row, _) = self.get_visual_info(width);
        if let Some(line) = visual_lines.get(v_row) {
            let v_col_end = line.chars().map(|c| c.width().unwrap_or(0)).sum();
            let (l_row, l_col) = self.get_logical_pos(v_row, v_col_end, width);
            self.textarea.move_cursor(tui_textarea::CursorMove::Jump(
                u16::try_from(l_row).unwrap_or(u16::MAX),
                u16::try_from(l_col).unwrap_or(u16::MAX),
            ));
        }
    }

    fn enforce_message_limit(&mut self) {
        let content = self.value();
        if content.len() > MAX_MESSAGE_LENGTH {
            let truncated: String = content.chars().take(MAX_MESSAGE_LENGTH).collect();
            self.set_content(&truncated);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        registry: &CommandRegistry,
    ) -> Option<MessageInputAction> {
        match key.code {
            KeyCode::Home => {
                self.move_cursor_start();
                return Some(MessageInputAction::StartTyping);
            }
            KeyCode::End => {
                self.move_cursor_end();
                return Some(MessageInputAction::StartTyping);
            }
            _ => {}
        }

        match if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
            Some(Action::SendMessage)
        } else {
            registry.find_action(key)
        } {
            Some(Action::Cancel) => {
                if self.is_editing() {
                    self.reset_mode();
                    self.clear();
                    Some(MessageInputAction::CancelReply)
                } else if self.is_replying() {
                    self.reset_mode();
                    Some(MessageInputAction::CancelReply)
                } else if !self.is_empty() || !self.attachments.is_empty() {
                    self.clear();
                    self.clear_attachments();
                    None
                } else {
                    Some(MessageInputAction::ExitInput)
                }
            }
            Some(Action::SendMessage) => {
                let content = self.message_content();
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
            Some(Action::Paste) => Some(MessageInputAction::Paste),
            _ => {
                if let KeyCode::Char(c) = key.code
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                {
                    self.textarea.insert_char(c);
                } else if key.code == KeyCode::Backspace {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT)
                    {
                        self.textarea.delete_word();
                        return Some(MessageInputAction::StartTyping);
                    }

                    let (row, col) = self.textarea.cursor();
                    let lines = self.textarea.lines();
                    if let Some(line) = lines.get(row) {
                        let byte_index = line
                            .char_indices()
                            .map(|(i, _)| i)
                            .nth(col)
                            .unwrap_or(line.len());

                        let prefix = &line[..byte_index];
                        let mut deleted_mention = false;
                        let mut longest_match: Option<String> = None;
                        for mention in self.mentions.keys() {
                            if prefix.ends_with(mention) {
                                if let Some(ref current) = longest_match {
                                    if mention.len() > current.len() {
                                        longest_match = Some(mention.clone());
                                    }
                                } else {
                                    longest_match = Some(mention.clone());
                                }
                            }
                        }

                        if let Some(mention) = longest_match {
                            for _ in 0..mention.len() {
                                self.textarea.delete_char();
                            }
                            self.mentions.remove(&mention);
                            deleted_mention = true;
                        }

                        if deleted_mention {
                            return Some(MessageInputAction::StartTyping);
                        }
                    }

                    self.textarea.delete_char();
                } else if (key.code == KeyCode::Delete
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT)))
                    || (key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::ALT))
                {
                    self.textarea.delete_next_word();
                    return Some(MessageInputAction::StartTyping);
                } else if (key.code == KeyCode::Char('w')
                    && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('h')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    self.textarea.delete_word();
                    return Some(MessageInputAction::StartTyping);
                } else if key.code == KeyCode::Delete {
                    self.textarea.delete_next_char();
                } else if key.code == KeyCode::Left {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT)
                    {
                        self.textarea
                            .move_cursor(tui_textarea::CursorMove::WordBack);
                    } else {
                        self.textarea.move_cursor(tui_textarea::CursorMove::Back);
                    }
                } else if key.code == KeyCode::Right {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT)
                    {
                        self.textarea
                            .move_cursor(tui_textarea::CursorMove::WordForward);
                    } else {
                        self.textarea.move_cursor(tui_textarea::CursorMove::Forward);
                    }
                } else if key.code == KeyCode::Up {
                    self.move_cursor_up();
                } else if key.code == KeyCode::Down {
                    self.move_cursor_down();
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

    fn setup_block<'a>(&self, style: &'a MessageInputStyle) -> Block<'a> {
        let border_style = if self.focused {
            style.border_style_focused
        } else {
            style.border_style
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        match &self.mode {
            MessageInputMode::Reply { author, .. } => {
                let reply_title = format!(" Replying to @{author} ");
                block = block
                    .title(reply_title)
                    .title_style(style.reply_indicator_style);
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
    pub fn render_with_style(&mut self, area: Rect, buf: &mut Buffer, style: &MessageInputStyle) {
        let block = self.setup_block(style);
        let inner = block.inner(area);
        block.render(area, buf);

        let width = inner.width as usize;
        if width == 0 {
            return;
        }
        self.last_width = width;

        let (visual_lines, v_cursor_row, v_cursor_col) = self.get_visual_info(width);

        let height = inner.height as usize;

        if v_cursor_row >= self.scroll_offset + height {
            self.scroll_offset = v_cursor_row - height + 1;
        } else if v_cursor_row < self.scroll_offset {
            self.scroll_offset = v_cursor_row;
        }

        let text_style = if self.value().is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            style.text_style
        };

        for (i, line) in visual_lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(height)
        {
            let y = inner.y + u16::try_from(i - self.scroll_offset).unwrap_or(0);

            if self.value().is_empty() && visual_lines.len() == 1 && line.is_empty() {
                let placeholder = if self.has_channel {
                    PLACEHOLDER_TEXT
                } else {
                    PLACEHOLDER_NO_CHANNEL
                };
                let placeholder_chars: Vec<char> = placeholder.chars().collect();

                for j in 0..width {
                    let x = inner.x + u16::try_from(j).unwrap_or(0);
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        if j < placeholder_chars.len() {
                            cell.set_symbol(&placeholder_chars[j].to_string());
                            cell.set_style(style.placeholder_style);
                        } else {
                            cell.set_symbol(" ");
                            cell.set_style(Style::default());
                        }
                    }
                }
            } else {
                let line_chars: Vec<char> = line.chars().collect();
                let mut current_width = 0;

                for ch in &line_chars {
                    let ch_width = ch.width().unwrap_or(0);
                    if current_width + ch_width > width {
                        break;
                    }

                    let x = inner.x + u16::try_from(current_width).unwrap_or(0);
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_symbol(&ch.to_string());
                        cell.set_style(text_style);
                    }
                    current_width += ch_width;
                }

                for j in current_width..width {
                    let x = inner.x + u16::try_from(j).unwrap_or(0);
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_symbol(" ");
                        cell.set_style(Style::default());
                    }
                }
            }

            if self.focused && i == v_cursor_row {
                let cursor_x = inner.x + u16::try_from(v_cursor_col).unwrap_or(0);
                if cursor_x < inner.x + inner.width
                    && let Some(cell) = buf.cell_mut((cursor_x, y))
                {
                    cell.set_style(style.cursor_style);
                    if cell.symbol().is_empty() {
                        cell.set_symbol(" ");
                    }
                }
            }
        }

        let lines_rendered = visual_lines
            .len()
            .saturating_sub(self.scroll_offset)
            .min(height);
        for i in lines_rendered..height {
            let y = inner.y + u16::try_from(i).unwrap_or(0);
            for j in 0..width {
                let x = inner.x + u16::try_from(j).unwrap_or(0);
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(Style::default());
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

impl MessageInputStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(theme.accent),
            reply_indicator_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::ITALIC),
            ..Self::default()
        }
    }
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

#[derive(Default)]
pub struct MessageInput {
    style: MessageInputStyle,
}

impl MessageInput {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            style: MessageInputStyle {
                border_style: Style::new(),
                border_style_focused: Style::new(),
                text_style: Style::new(),
                placeholder_style: Style::new(),
                cursor_style: Style::new(),
                reply_indicator_style: Style::new(),
            },
        }
    }

    #[must_use]
    pub const fn style(mut self, style: MessageInputStyle) -> Self {
        self.style = style;
        self
    }

    pub fn render(&self, state: &mut MessageInputState<'_>, area: Rect, buf: &mut Buffer) {
        state.render_with_style(area, buf, &self.style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use test_case::test_case;

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
        state.start_reply(MessageId(123), "user".to_string(), true);

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

    #[test]
    fn test_multiline_wrapping() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        let mut state = MessageInputState::new();
        state.set_content("1234567890"); // 10 chars

        let style = MessageInputStyle::default();

        let area = Rect::new(0, 0, 7, 5);
        let mut buf = Buffer::empty(area);

        state.render_with_style(area, &mut buf, &style);

        assert_eq!(buf[(1, 1)].symbol(), "1");
        assert_eq!(buf[(5, 1)].symbol(), "5");

        assert_eq!(buf[(1, 2)].symbol(), "6");
        assert_eq!(buf[(5, 2)].symbol(), "0");
    }

    #[test]
    fn test_ctrl_left_right_move_word() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use tui_textarea::CursorMove;

        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_content("hello world test");

        state.textarea.move_cursor(CursorMove::Head);

        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL);
        state.handle_key(key, &registry);
        let (_, col) = state.textarea.cursor();
        assert_eq!(col, 6);

        state.handle_key(key, &registry);
        let (_, col) = state.textarea.cursor();
        assert_eq!(col, 12);

        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
        state.handle_key(key, &registry);
        let (_, col) = state.textarea.cursor();
        assert_eq!(col, 6);
    }

    #[test]
    fn test_delete_mention_at_once() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use tui_textarea::CursorMove;

        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_content("Hello @Ant");
        state.textarea.move_cursor(CursorMove::End);

        state.insert_mention(6, "Antonio", "12345");

        state.textarea.move_cursor(CursorMove::End);

        state.textarea.delete_char();

        assert_eq!(state.value(), "Hello @Antonio", "Pre-condition failed");

        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        state.handle_key(key, &registry);

        assert_eq!(state.value(), "Hello ");

        assert!(!state.mentions.contains_key("@Antonio"));
    }

    #[test_case(KeyCode::Delete, KeyModifiers::CONTROL, "hello world", true, " world" ; "ctrl+delete from head")]
    #[test_case(KeyCode::Backspace, KeyModifiers::CONTROL, "hello world", false, "hello " ; "ctrl+backspace from end")]
    #[test_case(KeyCode::Char('w'), KeyModifiers::CONTROL, "hello world", false, "hello " ; "ctrl+w from end")]
    fn test_word_deletion(
        key: KeyCode,
        modifiers: KeyModifiers,
        initial: &str,
        is_head: bool,
        expected: &str,
    ) {
        use crossterm::event::KeyEvent;
        use tui_textarea::CursorMove;

        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_content(initial);

        if is_head {
            state.textarea.move_cursor(CursorMove::Head);
        } else {
            state.textarea.move_cursor(CursorMove::End);
        }

        let key_event = KeyEvent::new(key, modifiers);
        state.handle_key(key_event, &registry);

        assert_eq!(state.value(), expected);
    }

    #[test_case("Despupé", 0, true, KeyCode::Backspace, KeyModifiers::NONE, "Despup" ; "backspace multibyte")]
    #[test_case("hello world", 5, false, KeyCode::Backspace, KeyModifiers::CONTROL, " world" ; "ctrl+backspace word at start")]
    #[test_case("hello    world", 0, true, KeyCode::Backspace, KeyModifiers::CONTROL, "hello    " ; "ctrl+backspace multiple spaces")]
    #[test_case("", 0, true, KeyCode::Backspace, KeyModifiers::CONTROL, "" ; "ctrl+backspace empty")]
    #[test_case("hello", 0, true, KeyCode::Backspace, KeyModifiers::CONTROL, "" ; "ctrl+backspace single word")]
    #[test_case("hello world café", 0, true, KeyCode::Backspace, KeyModifiers::CONTROL, "hello world " ; "ctrl+backspace unicode")]
    fn test_deletion_edge_cases(
        initial: &str,
        offset: usize,
        is_end: bool,
        key: KeyCode,
        modifiers: KeyModifiers,
        expected: &str,
    ) {
        use crossterm::event::KeyEvent;
        use tui_textarea::CursorMove;

        let mut state = MessageInputState::new();
        let registry = CommandRegistry::default();
        state.set_has_channel(true);
        state.set_content(initial);

        if is_end {
            state.textarea.move_cursor(CursorMove::End);
        } else {
            state.textarea.move_cursor(CursorMove::Head);
            for _ in 0..offset {
                state.textarea.move_cursor(CursorMove::Forward);
            }
        }

        let key_event = KeyEvent::new(key, modifiers);
        state.handle_key(key_event, &registry);

        assert_eq!(state.value(), expected);
    }
}
