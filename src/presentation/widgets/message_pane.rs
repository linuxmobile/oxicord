//! Message pane widget for displaying channel messages.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use tui_scrollview::{ScrollView, ScrollViewState};

use crate::domain::entities::{ChannelId, Message, MessageId};

const MESSAGE_HEIGHT_BASE: u16 = 2;
const MESSAGE_CONTENT_PADDING: u16 = 2;
const SCROLL_AMOUNT: u16 = 3;

#[derive(Debug, Clone)]
pub enum MessagePaneAction {
    SelectMessage(MessageId),
    ClearSelection,
    Reply {
        message_id: MessageId,
        mention: bool,
    },
    Edit(MessageId),
    Delete(MessageId),
    YankContent(String),
    YankUrl(String),
    YankId(String),
    OpenAttachments(MessageId),
    JumpToReply(MessageId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingState {
    Idle,
    Loading,
    Loaded,
    Error,
}

pub struct MessagePaneData {
    channel_id: Option<ChannelId>,
    channel_name: Option<String>,
    messages: Vec<Message>,
    loading_state: LoadingState,
    error_message: Option<String>,
}

impl MessagePaneData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            channel_id: None,
            channel_name: None,
            messages: Vec::new(),
            loading_state: LoadingState::Idle,
            error_message: None,
        }
    }

    pub fn set_channel(&mut self, channel_id: ChannelId, channel_name: String) {
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name);
        self.messages.clear();
        self.loading_state = LoadingState::Loading;
        self.error_message = None;
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.loading_state = LoadingState::Loaded;
        self.error_message = None;
    }

    pub fn add_message(&mut self, message: Message) {
        if self.channel_id == Some(message.channel_id()) {
            self.messages.push(message);
        }
    }

    pub fn update_message(&mut self, updated: Message) {
        if let Some(pos) = self.messages.iter().position(|m| m.id() == updated.id()) {
            self.messages[pos] = updated;
        }
    }

    pub fn remove_message(&mut self, message_id: MessageId) {
        self.messages.retain(|m| m.id() != message_id);
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_state = LoadingState::Error;
        self.error_message = Some(error);
    }

    pub fn clear(&mut self) {
        self.channel_id = None;
        self.channel_name = None;
        self.messages.clear();
        self.loading_state = LoadingState::Idle;
        self.error_message = None;
    }

    #[must_use]
    pub fn channel_id(&self) -> Option<ChannelId> {
        self.channel_id
    }

    #[must_use]
    pub fn channel_name(&self) -> Option<&str> {
        self.channel_name.as_deref()
    }

    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[must_use]
    pub fn loading_state(&self) -> LoadingState {
        self.loading_state
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for MessagePaneData {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MessagePaneState {
    scroll_state: ScrollViewState,
    selected_index: Option<usize>,
    focused: bool,
    auto_scroll: bool,
    content_height: u16,
    viewport_height: u16,
}

impl MessagePaneState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll_state: ScrollViewState::default(),
            selected_index: None,
            focused: false,
            auto_scroll: true,
            content_height: 0,
            viewport_height: 0,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn select_next(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.auto_scroll = false;
        self.selected_index = Some(match self.selected_index {
            Some(idx) => (idx + 1).min(message_count - 1),
            None => message_count.saturating_sub(1),
        });
    }

    pub fn select_previous(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.auto_scroll = false;
        self.selected_index = Some(match self.selected_index {
            Some(idx) => idx.saturating_sub(1),
            None => message_count.saturating_sub(1),
        });
    }

    pub fn select_first(&mut self) {
        self.auto_scroll = false;
        self.selected_index = Some(0);
        self.scroll_state.scroll_to_top();
    }

    pub fn select_last(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }
        self.selected_index = Some(message_count - 1);
        self.auto_scroll = true;
        self.scroll_to_bottom();
    }

    pub fn clear_selection(&mut self) {
        self.selected_index = None;
        self.auto_scroll = true;
    }

    pub fn scroll_down(&mut self) {
        self.auto_scroll = false;
        for _ in 0..SCROLL_AMOUNT {
            self.scroll_state.scroll_down();
        }
    }

    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        for _ in 0..SCROLL_AMOUNT {
            self.scroll_state.scroll_up();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        if self.content_height > self.viewport_height {
            let offset = self.content_height.saturating_sub(self.viewport_height);
            self.scroll_state
                .set_offset(ratatui::layout::Position { x: 0, y: offset });
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_state.scroll_to_top();
    }

    pub fn on_new_message(&mut self) {
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn update_dimensions(&mut self, content_height: u16, viewport_height: u16) {
        self.content_height = content_height;
        self.viewport_height = viewport_height;

        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        data: &MessagePaneData,
    ) -> Option<MessagePaneAction> {
        let message_count = data.message_count();

        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => {
                self.select_next(message_count);
                None
            }
            (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => {
                self.select_previous(message_count);
                None
            }
            (KeyCode::Char('J'), KeyModifiers::SHIFT) => {
                self.scroll_down();
                None
            }
            (KeyCode::Char('K'), KeyModifiers::SHIFT) => {
                self.scroll_up();
                None
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.select_first();
                None
            }
            (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                self.select_last(message_count);
                None
            }
            (KeyCode::Home, KeyModifiers::NONE) => {
                self.scroll_to_top();
                None
            }
            (KeyCode::End, KeyModifiers::NONE) => {
                self.scroll_to_bottom();
                self.auto_scroll = true;
                None
            }
            (KeyCode::Esc, KeyModifiers::NONE) => {
                self.clear_selection();
                Some(MessagePaneAction::ClearSelection)
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.get_selected_message_id(data)
                    .map(|id| MessagePaneAction::Reply {
                        message_id: id,
                        mention: true,
                    })
            }
            (KeyCode::Char('R'), KeyModifiers::SHIFT) => {
                self.get_selected_message_id(data)
                    .map(|id| MessagePaneAction::Reply {
                        message_id: id,
                        mention: false,
                    })
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::Edit),
            (KeyCode::Char('d'), KeyModifiers::NONE) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::Delete),
            (KeyCode::Char('y'), KeyModifiers::NONE) => self
                .get_selected_message(data)
                .map(|m| MessagePaneAction::YankContent(m.content().to_string())),
            (KeyCode::Char('i'), KeyModifiers::NONE) => self
                .get_selected_message_id(data)
                .map(|id| MessagePaneAction::YankId(id.to_string())),
            (KeyCode::Char('o'), KeyModifiers::NONE) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::OpenAttachments),
            (KeyCode::Char('s'), KeyModifiers::NONE) => self
                .get_selected_message(data)
                .and_then(|m| m.reference())
                .and_then(crate::domain::entities::MessageReference::message_id)
                .map(MessagePaneAction::JumpToReply),
            _ => None,
        }
    }

    fn get_selected_message_id(&self, data: &MessagePaneData) -> Option<MessageId> {
        self.selected_index
            .and_then(|idx| data.messages().get(idx))
            .map(crate::domain::entities::Message::id)
    }

    fn get_selected_message<'a>(&self, data: &'a MessagePaneData) -> Option<&'a Message> {
        self.selected_index.and_then(|idx| data.messages().get(idx))
    }
}

impl Default for MessagePaneState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(missing_docs)]
pub struct MessagePaneStyle {
    pub border_style: Style,
    pub border_style_focused: Style,
    pub title_style: Style,
    pub author_style: Style,
    pub bot_badge_style: Style,
    pub timestamp_style: Style,
    pub content_style: Style,
    pub edited_style: Style,
    pub selected_style: Style,
    pub reply_style: Style,
    pub attachment_style: Style,
    pub system_message_style: Style,
    pub loading_style: Style,
    pub error_style: Style,
    pub empty_style: Style,
}

impl Default for MessagePaneStyle {
    fn default() -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(Color::Cyan),
            title_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            author_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            bot_badge_style: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            timestamp_style: Style::default().fg(Color::DarkGray),
            content_style: Style::default().fg(Color::White),
            edited_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            selected_style: Style::default().bg(Color::DarkGray),
            reply_style: Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
            attachment_style: Style::default().fg(Color::Blue),
            system_message_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            loading_style: Style::default().fg(Color::Yellow),
            error_style: Style::default().fg(Color::Red),
            empty_style: Style::default().fg(Color::DarkGray),
        }
    }
}

/// Widget for displaying channel messages.
#[allow(missing_docs)]
pub struct MessagePane<'a> {
    data: &'a MessagePaneData,
    style: MessagePaneStyle,
}

impl<'a> MessagePane<'a> {
    #[must_use]
    pub fn new(data: &'a MessagePaneData) -> Self {
        Self {
            data,
            style: MessagePaneStyle::default(),
        }
    }

    #[must_use]
    pub fn style(mut self, style: MessagePaneStyle) -> Self {
        self.style = style;
        self
    }

    fn calculate_content_height(&self, width: u16) -> u16 {
        self.data
            .messages()
            .iter()
            .map(|m| Self::calculate_message_height(m, width))
            .sum()
    }

    fn calculate_message_height(message: &Message, width: u16) -> u16 {
        let content_width = width.saturating_sub(MESSAGE_CONTENT_PADDING);
        let content_lines = if content_width > 0 {
            let content_len = u16::try_from(message.content().len()).unwrap_or(u16::MAX);
            (content_len / content_width).max(1)
        } else {
            1
        };

        let mut height = MESSAGE_HEIGHT_BASE + content_lines;

        if message.is_reply() && message.referenced().is_some() {
            height += 1;
        }

        if message.has_attachments() {
            height += u16::try_from(message.attachments().len()).unwrap_or(u16::MAX);
        }

        height
    }

    fn render_message(
        &self,
        message: &Message,
        y_offset: u16,
        width: u16,
        is_selected: bool,
        scroll_view: &mut ScrollView,
    ) -> u16 {
        let mut current_y = y_offset;
        let base_style = if is_selected {
            self.style.selected_style
        } else {
            Style::default()
        };

        if message.is_reply()
            && let Some(referenced) = message.referenced()
        {
            let reply_text = format!(
                "↳ {} {}",
                referenced.author().display_name(),
                truncate_string(referenced.content(), 50)
            );
            let reply_line = Line::from(Span::styled(reply_text, self.style.reply_style));
            let reply_para = Paragraph::new(reply_line).style(base_style);
            scroll_view.render_widget(reply_para, Rect::new(0, current_y, width, 1));
            current_y += 1;
        }

        let mut header_spans = vec![
            Span::styled(message.formatted_timestamp(), self.style.timestamp_style),
            Span::raw(" "),
            Span::styled(message.author().display_name(), self.style.author_style),
        ];

        if message.author().is_bot() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("[BOT]", self.style.bot_badge_style));
        }

        if message.is_edited() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("(edited)", self.style.edited_style));
        }

        let header_line = Line::from(header_spans);
        let header_para = Paragraph::new(header_line).style(base_style);
        scroll_view.render_widget(header_para, Rect::new(0, current_y, width, 1));
        current_y += 1;

        let content_style = if message.kind().is_system() {
            self.style.system_message_style
        } else {
            self.style.content_style
        };

        let content = message.content();
        let content_width = width.saturating_sub(MESSAGE_CONTENT_PADDING);
        let content_lines = wrap_text(content, content_width as usize);

        for line_text in content_lines {
            let content_line = Line::from(Span::styled(line_text, content_style));
            let content_para = Paragraph::new(content_line).style(base_style);
            scroll_view.render_widget(
                content_para,
                Rect::new(2, current_y, width.saturating_sub(2), 1),
            );
            current_y += 1;
        }

        for attachment in message.attachments() {
            let attachment_text = format!("📎 {}", attachment.filename());
            let attachment_line =
                Line::from(Span::styled(attachment_text, self.style.attachment_style));
            let attachment_para = Paragraph::new(attachment_line).style(base_style);
            scroll_view.render_widget(
                attachment_para,
                Rect::new(2, current_y, width.saturating_sub(2), 1),
            );
            current_y += 1;
        }

        current_y += 1;
        current_y - y_offset
    }
}

impl StatefulWidget for MessagePane<'_> {
    type State = MessagePaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_style = if state.is_focused() {
            self.style.border_style_focused
        } else {
            self.style.border_style
        };

        let title = match self.data.channel_name() {
            Some(name) => format!(" #{name} "),
            None => " Messages ".to_string(),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(title, self.style.title_style));

        let inner_area = block.inner(area);
        block.render(area, buf);

        match self.data.loading_state() {
            LoadingState::Loading => {
                let loading = Paragraph::new("Loading messages...").style(self.style.loading_style);
                loading.render(inner_area, buf);
                return;
            }
            LoadingState::Error => {
                let error_msg = self
                    .data
                    .error_message
                    .as_deref()
                    .unwrap_or("Unknown error");
                let error =
                    Paragraph::new(format!("Error: {error_msg}")).style(self.style.error_style);
                error.render(inner_area, buf);
                return;
            }
            LoadingState::Idle => {
                let empty = Paragraph::new("Select a channel to view messages")
                    .style(self.style.empty_style);
                empty.render(inner_area, buf);
                return;
            }
            LoadingState::Loaded => {}
        }

        if self.data.is_empty() {
            let empty = Paragraph::new("No messages in this channel").style(self.style.empty_style);
            empty.render(inner_area, buf);
            return;
        }

        let content_width = inner_area.width;
        let content_height = self.calculate_content_height(content_width);

        state.update_dimensions(content_height, inner_area.height);

        let mut scroll_view = ScrollView::new(Size::new(content_width, content_height));

        let mut y_offset: u16 = 0;
        for (idx, message) in self.data.messages().iter().enumerate() {
            let is_selected = state.selected_index == Some(idx);
            let msg_height = self.render_message(
                message,
                y_offset,
                content_width,
                is_selected,
                &mut scroll_view,
            );
            y_offset += msg_height;
        }

        scroll_view.render(inner_area, buf, &mut state.scroll_state);
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    if width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::MessageAuthor;
    use chrono::Utc;

    fn create_test_message(id: u64, content: &str) -> Message {
        let author = MessageAuthor::new("1", "testuser", "0", None, false);
        Message::new(id, 100_u64, author, content, Utc::now())
    }

    #[test]
    fn test_message_pane_data_creation() {
        let data = MessagePaneData::new();
        assert!(data.is_empty());
        assert!(data.channel_id().is_none());
        assert_eq!(data.loading_state(), LoadingState::Idle);
    }

    #[test]
    fn test_message_pane_data_set_messages() {
        let mut data = MessagePaneData::new();
        data.set_channel(ChannelId(100), "general".to_string());

        let messages = vec![
            create_test_message(1, "Hello"),
            create_test_message(2, "World"),
        ];
        data.set_messages(messages);

        assert_eq!(data.message_count(), 2);
        assert_eq!(data.loading_state(), LoadingState::Loaded);
    }

    #[test]
    fn test_message_pane_state_navigation() {
        let mut state = MessagePaneState::new();

        state.select_next(5);
        assert_eq!(state.selected_index(), Some(4));

        state.select_previous(5);
        assert_eq!(state.selected_index(), Some(3));

        state.select_first();
        assert_eq!(state.selected_index(), Some(0));

        state.select_last(5);
        assert_eq!(state.selected_index(), Some(4));
    }

    #[test]
    fn test_wrap_text() {
        let text = "Hello world this is a test";
        let lines = wrap_text(text, 10);
        assert!(lines.len() > 1);

        let empty_lines = wrap_text("", 10);
        assert_eq!(empty_lines.len(), 1);
    }
}
