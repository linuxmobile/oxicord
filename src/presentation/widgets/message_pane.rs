use std::collections::{HashMap, HashSet, VecDeque};

use crate::application::services::markdown_service::{MarkdownService, MentionResolver};
use crate::domain::keybinding::Action;
use crate::presentation::commands::CommandRegistry;

use crossterm::event::KeyEvent;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};
use unicode_width::UnicodeWidthStr;

use crate::domain::entities::{ChannelId, Message, MessageId};
const SCROLL_AMOUNT: u16 = 3;
const SCROLLBAR_MARGIN: u16 = 2;
const CHANNEL_NAME_PREFIX: &str = "[ ";
const CHANNEL_NAME_SUFFIX: &str = " ]";
const DM_CHANNEL_PREFIX: &str = "[ ";
const TIMESTAMP_WIDTH: usize = 6;
const CONTENT_INDENT: usize = 6;

pub struct UiMessage {
    pub message: Message,
    pub estimated_height: u16,
    pub rendered_content: Option<Text<'static>>,
}

impl UiMessage {
    fn new(message: Message) -> Self {
        Self {
            message,
            estimated_height: 1,
            rendered_content: None,
        }
    }
}

struct HashMapResolver<'a>(&'a HashMap<String, String>);

impl MentionResolver for HashMapResolver<'_> {
    fn resolve(&self, user_id: &str) -> Option<String> {
        self.0.get(user_id).cloned()
    }
}

#[derive(Debug, Clone)]
pub enum MessagePaneAction {
    SelectMessage(MessageId),
    ClearSelection,
    Reply {
        message_id: MessageId,
        mention: bool,
    },
    Edit(MessageId),
    EditExternal(MessageId),
    Delete(MessageId),
    YankContent(String),
    YankUrl(String),
    YankId(String),
    OpenAttachments(MessageId),
    JumpToReply(MessageId),
    LoadHistory,
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
    channel_topic: Option<String>,
    channel_icon: Option<String>,
    online_count: Option<u32>,
    messages: VecDeque<UiMessage>,
    loading_state: LoadingState,
    error_message: Option<String>,
    is_dm: bool,
    typing_indicator: Option<String>,
    authors: HashMap<String, String>,
    last_layout_width: Option<u16>,
    is_dirty: bool,
}

impl MessagePaneData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            channel_id: None,
            channel_name: None,
            channel_topic: None,
            channel_icon: None,
            online_count: None,
            messages: VecDeque::new(),
            loading_state: LoadingState::Idle,
            error_message: None,
            is_dm: false,
            typing_indicator: None,
            authors: HashMap::new(),
            last_layout_width: None,
            is_dirty: true,
        }
    }

    pub fn set_channel(&mut self, channel_id: ChannelId, channel_name: String) {
        self.is_dm = channel_name.starts_with('@');
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name);
        self.messages.clear();
        self.loading_state = LoadingState::Loading;
        self.error_message = None;
        self.is_dirty = true;
    }

    pub fn set_channel_topic(&mut self, topic: Option<String>) {
        self.channel_topic = topic;
    }

    pub fn set_channel_icon(&mut self, icon: Option<String>) {
        self.channel_icon = icon;
    }

    pub const fn set_online_count(&mut self, count: Option<u32>) {
        self.online_count = count;
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        for msg in &messages {
            self.authors
                .insert(msg.author().id().to_string(), msg.author().display_name());
            for mention in msg.mentions() {
                self.authors
                    .insert(mention.id().to_string(), mention.display_name());
            }
        }
        self.messages = messages.into_iter().map(UiMessage::new).collect();
        self.loading_state = LoadingState::Loaded;
        self.error_message = None;
        self.is_dirty = true;
    }

    pub fn add_message(&mut self, message: Message) {
        if self.channel_id == Some(message.channel_id())
            && !self.messages.iter().any(|m| m.message.id() == message.id())
        {
            self.authors.insert(
                message.author().id().to_string(),
                message.author().display_name(),
            );
            for mention in message.mentions() {
                self.authors
                    .insert(mention.id().to_string(), mention.display_name());
            }
            self.messages.push_back(UiMessage::new(message));
            self.is_dirty = true;
        }
    }

    pub fn prepend_messages(&mut self, new_messages: Vec<Message>) -> usize {
        let existing_ids: HashSet<_> = self.messages.iter().map(|m| m.message.id()).collect();
        let mut added = 0;
        for msg in new_messages.into_iter().rev() {
            if !existing_ids.contains(&msg.id()) {
                self.authors
                    .insert(msg.author().id().to_string(), msg.author().display_name());
                for mention in msg.mentions() {
                    self.authors
                        .insert(mention.id().to_string(), mention.display_name());
                }
                self.messages.push_front(UiMessage::new(msg));
                added += 1;
            }
        }
        if added > 0 {
            self.is_dirty = true;
        }
        added
    }

    pub fn update_message(&mut self, updated: Message) {
        if let Some(pos) = self
            .messages
            .iter()
            .position(|m| m.message.id() == updated.id())
        {
            let new_msg = UiMessage::new(updated);
            self.messages[pos] = new_msg;
            self.is_dirty = true;
        }
    }

    pub fn remove_message(&mut self, message_id: MessageId) {
        self.messages.retain(|m| m.message.id() != message_id);
        self.is_dirty = true;
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_state = LoadingState::Error;
        self.error_message = Some(error);
        self.is_dirty = true;
    }

    pub fn clear(&mut self) {
        self.channel_id = None;
        self.channel_name = None;
        self.channel_topic = None;
        self.channel_icon = None;
        self.online_count = None;
        self.messages.clear();
        self.loading_state = LoadingState::Idle;
        self.error_message = None;
        self.is_dm = false;
        self.typing_indicator = None;
        self.authors.clear();
        self.is_dirty = true;
    }

    pub fn set_typing_indicator(&mut self, indicator: Option<String>) {
        self.typing_indicator = indicator;
    }

    #[must_use]
    pub fn typing_indicator(&self) -> Option<&str> {
        self.typing_indicator.as_deref()
    }

    #[must_use]
    pub const fn has_typing_indicator(&self) -> bool {
        self.typing_indicator.is_some()
    }

    #[must_use]
    pub const fn channel_id(&self) -> Option<ChannelId> {
        self.channel_id
    }

    #[must_use]
    pub fn channel_name(&self) -> Option<&str> {
        self.channel_name.as_deref()
    }

    #[must_use]
    pub fn channel_topic(&self) -> Option<&str> {
        self.channel_topic.as_deref()
    }

    #[must_use]
    pub fn channel_icon(&self) -> Option<&str> {
        self.channel_icon.as_deref()
    }

    #[must_use]
    pub const fn online_count(&self) -> Option<u32> {
        self.online_count
    }

    #[must_use]
    pub fn messages(&self) -> &VecDeque<UiMessage> {
        &self.messages
    }

    #[must_use]
    pub fn get_message(&self, index: usize) -> Option<&Message> {
        self.messages.get(index).map(|m| &m.message)
    }

    #[must_use]
    pub fn ui_messages(&self) -> &VecDeque<UiMessage> {
        &self.messages
    }

    pub fn ui_messages_mut(&mut self) -> &mut VecDeque<UiMessage> {
        &mut self.messages
    }

    pub fn update_layout(&mut self, width: u16, markdown_service: &MarkdownService) {
        if !self.is_dirty && self.last_layout_width == Some(width) {
            return;
        }

        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);
        let content_width = width
            .saturating_sub(indent_width)
            .saturating_sub(SCROLLBAR_MARGIN);

        let authors = &self.authors;
        let resolver = HashMapResolver(authors);

        for ui_msg in &mut self.messages {
            let message = &ui_msg.message;

            let text = markdown_service.render(message.content(), Some(&resolver));

            let mut content_lines = 0;
            for line in &text.lines {
                let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                content_lines +=
                    u16::try_from(wrap_text(&line_text, content_width as usize).len()).unwrap_or(0);
            }

            ui_msg.rendered_content = Some(text);

            let mut height = 1 + content_lines;

            if message.is_reply() && message.referenced().is_some() {
                height += 1;
            }

            if message.has_attachments() {
                height += u16::try_from(message.attachments().len()).unwrap_or(u16::MAX);
            }

            ui_msg.estimated_height = height;
        }

        self.last_layout_width = Some(width);
        self.is_dirty = false;
    }

    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    #[must_use]
    pub fn loading_state(&self) -> LoadingState {
        self.loading_state
    }

    #[must_use]
    pub fn formatted_channel_title(&self) -> Option<String> {
        self.channel_name.as_ref().map(|name| {
            let display_name = name.trim_start_matches('@').to_uppercase();
            if self.is_dm {
                format!("{DM_CHANNEL_PREFIX}{display_name}{CHANNEL_NAME_SUFFIX}")
            } else {
                format!("{CHANNEL_NAME_PREFIX}{display_name}{CHANNEL_NAME_SUFFIX}")
            }
        })
    }

    pub fn get_author_name(&self, user_id: &str) -> Option<&str> {
        self.authors.get(user_id).map(String::as_str)
    }
}

impl MentionResolver for MessagePaneData {
    fn resolve(&self, user_id: &str) -> Option<String> {
        self.authors.get(user_id).cloned()
    }
}

impl Default for MessagePaneData {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MessagePaneState {
    pub vertical_scroll: usize,
    pub scrollbar_state: ScrollbarState,
    selected_index: Option<usize>,
    focused: bool,
    is_following: bool,
    scroll_to_selection: bool,
    content_height: usize,
    viewport_height: u16,
    last_width: u16,
}

impl MessagePaneState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            vertical_scroll: 0,
            scrollbar_state: ScrollbarState::default(),
            selected_index: None,
            focused: false,
            is_following: true,
            scroll_to_selection: false,
            content_height: 0,
            viewport_height: 0,
            last_width: 0,
        }
    }

    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    #[must_use]
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    #[must_use]
    pub const fn last_width(&self) -> u16 {
        self.last_width
    }

    pub fn adjust_for_prepend(&mut self, added_count: usize, added_height: usize) {
        if let Some(idx) = self.selected_index {
            self.selected_index = Some(idx + added_count);
        }

        self.vertical_scroll = self.vertical_scroll.saturating_add(added_height);
        self.scrollbar_state = self.scrollbar_state.position(self.vertical_scroll);
    }

    pub fn select_next(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.is_following = false;
        self.scroll_to_selection = true;
        self.selected_index = Some(self.selected_index.map_or_else(
            || message_count.saturating_sub(1),
            |idx| (idx + 1).min(message_count - 1),
        ));
    }

    pub fn select_previous(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.is_following = false;
        self.scroll_to_selection = true;
        self.selected_index = Some(self.selected_index.map_or_else(
            || message_count.saturating_sub(1),
            |idx| idx.saturating_sub(1),
        ));
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn select_first(&mut self) {
        self.is_following = false;
        self.scroll_to_selection = true;
        self.selected_index = Some(0);
        self.scroll_to_top();
    }

    pub fn select_last(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }
        self.selected_index = Some(message_count - 1);
        self.is_following = false;
        self.scroll_to_selection = true;
        self.scroll_to_bottom();
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn clear_selection(&mut self) {
        self.selected_index = None;
        self.is_following = true;
        self.scroll_to_bottom();
    }

    pub fn on_new_message(&mut self) {
        if self.is_following {
            self.scroll_to_bottom();
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_down(&mut self) {
        self.is_following = false;
        let max_scroll = self
            .content_height
            .saturating_sub(self.viewport_height as usize);
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_add(SCROLL_AMOUNT as usize)
            .min(max_scroll);
        self.scrollbar_state = self.scrollbar_state.position(self.vertical_scroll);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_up(&mut self) {
        self.is_following = false;
        self.vertical_scroll = self.vertical_scroll.saturating_sub(SCROLL_AMOUNT as usize);
        self.scrollbar_state = self.scrollbar_state.position(self.vertical_scroll);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_to_bottom(&mut self) {
        if self.content_height > self.viewport_height as usize {
            self.vertical_scroll = self.content_height - self.viewport_height as usize;
        } else {
            self.vertical_scroll = 0;
        }
        self.scrollbar_state = self.scrollbar_state.position(self.vertical_scroll);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_to_top(&mut self) {
        self.vertical_scroll = 0;
        self.scrollbar_state = self.scrollbar_state.position(0);
    }

    pub fn update_dimensions(&mut self, content_height: usize, viewport_height: u16) {
        self.content_height = content_height;
        self.viewport_height = viewport_height;

        if self.is_following {
            self.scroll_to_bottom();
        } else {
            let max_scroll = content_height.saturating_sub(viewport_height as usize);
            self.vertical_scroll = self.vertical_scroll.min(max_scroll);
        }

        self.scrollbar_state = ScrollbarState::new(content_height)
            .viewport_content_length(viewport_height as usize)
            .position(self.vertical_scroll);
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        data: &MessagePaneData,
        registry: &CommandRegistry,
    ) -> Option<MessagePaneAction> {
        let message_count = data.message_count();

        match registry.find_action(key) {
            Some(Action::NavigateDown) => {
                self.select_next(message_count);
                None
            }
            Some(Action::NavigateUp) => {
                if self.selected_index == Some(0) {
                    return Some(MessagePaneAction::LoadHistory);
                }
                self.select_previous(message_count);
                None
            }
            Some(Action::ScrollDown) => {
                self.scroll_down();
                let max_scroll = self
                    .content_height
                    .saturating_sub(self.viewport_height as usize);
                if self.vertical_scroll == max_scroll {
                    self.is_following = true;
                }
                None
            }
            Some(Action::ScrollUp) => {
                if self.vertical_scroll == 0 {
                    return Some(MessagePaneAction::LoadHistory);
                }
                self.scroll_up();
                None
            }
            Some(Action::SelectFirst) => {
                self.select_first();
                Some(MessagePaneAction::LoadHistory)
            }
            Some(Action::SelectLast) => {
                self.select_last(message_count);
                None
            }
            Some(Action::ScrollToTop) => {
                self.scroll_to_top();
                None
            }
            Some(Action::ScrollToBottom) => {
                self.scroll_to_bottom();
                if self.selected_index.is_none() {
                    self.is_following = true;
                }
                None
            }
            Some(Action::Cancel | Action::ClearSelection) => {
                self.clear_selection();
                Some(MessagePaneAction::ClearSelection)
            }
            Some(Action::Reply) => {
                self.get_selected_message_id(data)
                    .map(|id| MessagePaneAction::Reply {
                        message_id: id,
                        mention: true,
                    })
            }
            Some(Action::ReplyNoMention) => {
                self.get_selected_message_id(data)
                    .map(|id| MessagePaneAction::Reply {
                        message_id: id,
                        mention: false,
                    })
            }
            Some(Action::EditMessage) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::Edit),
            Some(Action::OpenEditor) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::EditExternal),
            Some(Action::DeleteMessage) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::Delete),
            Some(Action::CopyContent) => self
                .get_selected_message(data)
                .map(|m| MessagePaneAction::YankContent(m.content().to_string())),
            Some(Action::YankId) => self
                .get_selected_message_id(data)
                .map(|id| MessagePaneAction::YankId(id.to_string())),
            Some(Action::OpenAttachments) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::OpenAttachments),
            Some(Action::JumpToReply) => self
                .get_selected_message(data)
                .and_then(|m| m.reference())
                .and_then(crate::domain::entities::MessageReference::message_id)
                .map(MessagePaneAction::JumpToReply),
            _ => None,
        }
    }

    pub fn get_selected_message_id(&self, data: &MessagePaneData) -> Option<MessageId> {
        self.selected_index
            .and_then(|idx| data.get_message(idx))
            .map(crate::domain::entities::Message::id)
    }

    fn get_selected_message<'a>(&self, data: &'a MessagePaneData) -> Option<&'a Message> {
        self.selected_index.and_then(|idx| data.get_message(idx))
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
    pub topic_style: Style,
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
    pub scrollbar_track_style: Style,
    pub scrollbar_thumb_style: Style,
}

impl Default for MessagePaneStyle {
    fn default() -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(Color::Cyan),
            title_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            topic_style: Style::default().fg(Color::DarkGray),
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
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            attachment_style: Style::default().fg(Color::Blue),
            system_message_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            loading_style: Style::default().fg(Color::Yellow),
            error_style: Style::default().fg(Color::Red),
            empty_style: Style::default().fg(Color::DarkGray),
            scrollbar_track_style: Style::default().fg(Color::DarkGray),
            scrollbar_thumb_style: Style::default().fg(Color::Gray),
        }
    }
}

#[allow(missing_docs)]
pub struct MessagePane<'a> {
    data: &'a mut MessagePaneData,
    style: MessagePaneStyle,
}

impl<'a> MessagePane<'a> {
    #[must_use]
    pub fn new(data: &'a mut MessagePaneData, _markdown_service: &'a MarkdownService) -> Self {
        Self {
            data,
            style: MessagePaneStyle::default(),
        }
    }

    #[must_use]
    pub const fn style(mut self, style: MessagePaneStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn calculate_message_height(
        &self,
        message: &Message,
        width: u16,
        markdown_service: &MarkdownService,
    ) -> u16 {
        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);
        let content_width = width
            .saturating_sub(indent_width)
            .saturating_sub(SCROLLBAR_MARGIN);

        let authors = &self.data.authors;
        let resolver = HashMapResolver(authors);
        let text = markdown_service.render(message.content(), Some(&resolver));

        let mut content_lines = 0;
        for line in &text.lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            content_lines +=
                u16::try_from(wrap_text(&line_text, content_width as usize).len()).unwrap_or(0);
        }

        let mut height = 1 + content_lines;

        if message.is_reply() && message.referenced().is_some() {
            height += 1;
        }

        if message.has_attachments() {
            height += u16::try_from(message.attachments().len()).unwrap_or(u16::MAX);
        }

        height
    }

    fn build_block(&self, state: &MessagePaneState) -> Block<'_> {
        let border_style = if state.is_focused() {
            self.style.border_style_focused
        } else {
            self.style.border_style
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        if let Some(title) = self.data.formatted_channel_title() {
            block = block.title(Line::from(Span::styled(title, self.style.title_style)));
        }

        if let Some(topic) = self.data.channel_topic() {
            let truncated_topic = truncate_string(topic, 60);
            block = block.title(
                Line::from(Span::styled(
                    format!(" {truncated_topic} "),
                    self.style.topic_style,
                ))
                .alignment(Alignment::Right),
            );
        }

        if let Some(typing) = self.data.typing_indicator() {
            block = block.title_bottom(
                Line::from(Span::styled(
                    format!(" {typing} "),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ))
                .alignment(Alignment::Left),
            );
        }

        block
    }
}

fn render_ui_message(
    ui_msg: &mut UiMessage,
    style: &MessagePaneStyle,
    index: usize,
    render_y: i32,
    area: Rect,
    buf: &mut Buffer,
    state: &mut MessagePaneState,
) {
    let message = &ui_msg.message;
    let is_selected = state.selected_index == Some(index);
    let mut current_msg_y = render_y;

    let base_style = if is_selected {
        style.selected_style
    } else {
        Style::default()
    };

    if message.is_reply()
        && let Some(referenced) = message.referenced()
    {
        if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
            let reply_text = format!(
                "↱ Replying to {}: {}",
                referenced.author().display_name(),
                truncate_string(referenced.content(), 50)
            );
            let reply_style = if is_selected {
                style.reply_style.fg(Color::White)
            } else {
                style.reply_style
            };
            let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
            let reply_line = Line::from(vec![indent_span, Span::styled(reply_text, reply_style)]);
            let reply_para = Paragraph::new(reply_line).style(base_style);

            let reply_area = Rect::new(
                area.x,
                area.y
                    .saturating_add(u16::try_from(current_msg_y).unwrap_or(0)),
                area.width,
                1,
            );
            reply_para.render(reply_area, buf);
        }
        current_msg_y += 1;
    }

    if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
        let (timestamp_style, edited_style) = if is_selected {
            (
                Style::default().fg(Color::White),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            )
        } else {
            (style.timestamp_style, style.edited_style)
        };

        let mut header_spans = vec![
            Span::styled(
                format!(
                    "{:<width$}",
                    message.formatted_timestamp(),
                    width = TIMESTAMP_WIDTH
                ),
                timestamp_style,
            ),
            Span::styled(message.author().display_name(), style.author_style),
        ];

        if message.author().is_bot() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("[BOT]", style.bot_badge_style));
        }

        if message.is_edited() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("(edited)", edited_style));
        }

        let header_line = Line::from(header_spans);
        let header_para = Paragraph::new(header_line).style(base_style);

        let header_area = Rect::new(
            area.x,
            area.y
                .saturating_add(u16::try_from(current_msg_y).unwrap_or(0)),
            area.width,
            1,
        );
        header_para.render(header_area, buf);
    }
    current_msg_y += 1;

    let content_style = if message.kind().is_system() {
        style.system_message_style
    } else {
        style.content_style
    };
    let paragraph_style = if is_selected {
        base_style
    } else {
        base_style.patch(content_style)
    };

    let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);

    let text = if let Some(t) = &ui_msg.rendered_content {
        t.clone()
    } else {
        Text::raw(message.content())
    };

    let mut para = Paragraph::new(text)
        .block(Block::default().padding(Padding::new(indent_width, SCROLLBAR_MARGIN, 0, 0)))
        .style(paragraph_style)
        .wrap(ratatui::widgets::Wrap { trim: false });

    let content_start_y = current_msg_y;

    let mut content_height = i32::from(ui_msg.estimated_height) - 1;
    if message.is_reply() && message.referenced().is_some() {
        content_height -= 1;
    }
    if message.has_attachments() {
        content_height -= i32::try_from(message.attachments().len()).unwrap_or(0);
    }

    let content_height = content_height.max(0);

    if content_start_y + content_height > 0 && content_start_y < i32::from(area.height) {
        let top_clip = if content_start_y < 0 {
            u16::try_from(content_start_y.unsigned_abs()).unwrap_or(0)
        } else {
            0
        };

        let target_y = u16::try_from(content_start_y.max(0)).unwrap_or(0);
        let available_height = area.height.saturating_sub(target_y);
        let effective_height = u16::try_from(content_height)
            .unwrap_or(0)
            .saturating_sub(top_clip)
            .min(available_height);

        if effective_height > 0 {
            para = para.scroll((top_clip, 0));
            let para_area = Rect::new(area.x, area.y + target_y, area.width, effective_height);
            para.render(para_area, buf);
        }
    }
    current_msg_y += content_height;

    if message.has_attachments() {
        for attachment in message.attachments() {
            if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
                let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
                let attachment_text = format!("📎 {}", attachment.filename());
                let attachment_line = Line::from(vec![
                    indent_span.clone(),
                    Span::styled(attachment_text, style.attachment_style),
                ]);
                let attachment_para = Paragraph::new(attachment_line).style(base_style);
                let att_area = Rect::new(
                    area.x,
                    area.y
                        .saturating_add(u16::try_from(current_msg_y).unwrap_or(0)),
                    area.width,
                    1,
                );
                attachment_para.render(att_area, buf);
            }
            current_msg_y += 1;
        }
    }
}

impl StatefulWidget for MessagePane<'_> {
    type State = MessagePaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = self.build_block(state);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let MessagePane { data, style } = self;

        match data.loading_state() {
            LoadingState::Loading => {
                let loading = Paragraph::new("Loading messages...").style(style.loading_style);
                loading.render(inner_area, buf);
                return;
            }
            LoadingState::Error => {
                let error_msg = data.error_message.as_deref().unwrap_or("Unknown error");
                let error = Paragraph::new(format!("Error: {error_msg}")).style(style.error_style);
                error.render(inner_area, buf);
                return;
            }
            LoadingState::Idle => {
                let empty =
                    Paragraph::new("Select a channel to view messages").style(style.empty_style);
                empty.render(inner_area, buf);
                return;
            }
            LoadingState::Loaded => {}
        }

        if data.is_empty() {
            let empty = Paragraph::new("No messages in this channel").style(style.empty_style);
            empty.render(inner_area, buf);
            return;
        }

        let content_height: usize = data
            .ui_messages()
            .iter()
            .map(|m| m.estimated_height as usize)
            .sum();
        state.update_dimensions(content_height, inner_area.height);
        state.last_width = inner_area.width;

        let mut offset = state.vertical_scroll;

        if let Some(selected_idx) = state.selected_index
            && state.scroll_to_selection
        {
            let mut selection_y_start = 0;
            let mut selection_height = 0;

            for (i, msg) in data.ui_messages().iter().enumerate() {
                let h = msg.estimated_height as usize;
                if i == selected_idx {
                    selection_height = h;
                    break;
                }
                selection_y_start += h;
            }

            let selection_y_end = selection_y_start + selection_height;
            let viewport_height = inner_area.height as usize;

            if selection_y_start < offset {
                offset = selection_y_start;
            } else if selection_y_end > offset + viewport_height {
                offset = selection_y_end.saturating_sub(viewport_height);
            }

            state.vertical_scroll = offset;
            state.scrollbar_state = state.scrollbar_state.position(offset);
            state.scroll_to_selection = false;
        }

        let mut current_y: i32 = 0;

        for (idx, ui_msg) in data.ui_messages_mut().iter_mut().enumerate() {
            let h = ui_msg.estimated_height as usize;

            if (current_y as usize) + h > offset
                && (current_y as usize) < offset + inner_area.height as usize
            {
                let render_y = current_y - i32::try_from(offset).unwrap_or(0);
                render_ui_message(ui_msg, &style, idx, render_y, inner_area, buf, state);
            }
            current_y += i32::try_from(h).unwrap_or(0);
        }

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(style.scrollbar_track_style)
            .thumb_style(style.scrollbar_thumb_style);
        let scrollbar_area = inner_area;
        scrollbar.render(scrollbar_area, buf, &mut state.scrollbar_state);
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
        let mut current_width = 0;

        for (i, word) in paragraph.split(' ').enumerate() {
            let prefix = if i > 0 { " " } else { "" };
            let prefix_width = if i > 0 { 1 } else { 0 };

            let word_width = UnicodeWidthStr::width(word);
            let total_word_width = prefix_width + word_width;

            if current_width + total_word_width <= width {
                current_line.push_str(prefix);
                current_line.push_str(word);
                current_width += total_word_width;
                continue;
            }

            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
                current_width = 0;
            }

            if word_width > width {
                let mut remaining_word = word;
                while !remaining_word.is_empty() {
                    let mut split_idx = remaining_word.len();
                    let mut split_width = 0;

                    for (idx, c) in remaining_word.char_indices() {
                        let w = UnicodeWidthStr::width(c.to_string().as_str());
                        if split_width + w > width {
                            split_idx = idx;
                            break;
                        }
                        split_width += w;
                    }

                    if split_idx == 0 && !remaining_word.is_empty() {
                        let (idx, c) = remaining_word.char_indices().next().unwrap();
                        split_idx = idx + c.len_utf8();
                    }

                    lines.push(remaining_word[..split_idx].to_string());
                    remaining_word = &remaining_word[split_idx..];
                }
            } else {
                current_line.push_str(word);
                current_width = word_width;
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
    use chrono::Local;

    fn create_test_message(id: u64, content: &str) -> Message {
        let author = MessageAuthor::new("1", "testuser", "0", None, false);
        Message::new(id, 100_u64, author, content, Local::now())
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

    #[test]
    fn test_formatted_channel_title() {
        let mut data = MessagePaneData::new();
        data.set_channel(ChannelId(100), "general".to_string());
        assert_eq!(
            data.formatted_channel_title(),
            Some("[ GENERAL ]".to_string())
        );

        let mut dm_data = MessagePaneData::new();
        dm_data.set_channel(ChannelId(200), "@username".to_string());
        assert_eq!(
            dm_data.formatted_channel_title(),
            Some("[ USERNAME ]".to_string())
        );
    }

    #[test]
    fn test_typing_indicator() {
        let mut data = MessagePaneData::new();
        data.set_channel(ChannelId(100), "general".to_string());

        assert!(data.typing_indicator().is_none());
        assert!(!data.has_typing_indicator());

        data.set_typing_indicator(Some("Alice is typing...".to_string()));
        assert_eq!(data.typing_indicator(), Some("Alice is typing..."));
        assert!(data.has_typing_indicator());

        data.set_typing_indicator(None);
        assert!(data.typing_indicator().is_none());
        assert!(!data.has_typing_indicator());
    }

    #[test]
    fn test_scrollbar_position_at_bottom() {
        use crate::application::services::markdown_service::MarkdownService;

        let mut data = MessagePaneData::new();
        data.set_channel(ChannelId(100), "general".to_string());

        // Add 50 messages of 1 line each.
        // Each message: 1 header + 1 content = 2 lines.
        // Total content height should be 100.
        let messages: Vec<Message> = (0..50).map(|i| create_test_message(i, "msg")).collect();
        data.set_messages(messages);

        let markdown = MarkdownService::new();
        // Layout width large enough to not wrap.
        data.update_layout(100, &markdown);

        let mut state = MessagePaneState::new();
        state.is_following = true; // Default

        // Viewport height 50.
        // Content height 100.
        // Max scroll = 50.
        // Vertical scroll should be 50.

        // Calculate content height
        let content_height: usize = data
            .ui_messages()
            .iter()
            .map(|m| m.estimated_height as usize)
            .sum();
        assert_eq!(content_height, 100);

        state.update_dimensions(content_height, 50);

        assert_eq!(state.vertical_scroll, 50);
        // Verify scrollbar state? Ratatui ScrollbarState doesn't expose fields easily,
        // but we can assume if vertical_scroll is correct, it's correct.
    }
}
