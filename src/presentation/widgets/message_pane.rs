use std::collections::{HashMap, HashSet, VecDeque};

use crate::application::services::markdown_service::{MarkdownService, MentionResolver};
use crate::domain::keybinding::Action;
use crate::presentation::commands::CommandRegistry;

use crossterm::event::KeyEvent;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, StatefulWidget, Widget},
};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::domain::entities::{ChannelId, Message, MessageId};

const SCROLL_AMOUNT: u16 = 3;
const SCROLLBAR_MARGIN: u16 = 2;
const CHANNEL_NAME_PREFIX: &str = "[ ";
const CHANNEL_NAME_SUFFIX: &str = " ]";
const DM_CHANNEL_PREFIX: &str = "[ ";
const TIMESTAMP_WIDTH: usize = 6;
const CONTENT_INDENT: usize = 6;

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
    messages: VecDeque<Message>,
    loading_state: LoadingState,
    error_message: Option<String>,
    is_dm: bool,
    typing_indicator: Option<String>,
    authors: HashMap<String, String>,
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
        }
    }

    pub fn set_channel(&mut self, channel_id: ChannelId, channel_name: String) {
        self.is_dm = channel_name.starts_with('@');
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name);
        self.messages.clear();
        self.loading_state = LoadingState::Loading;
        self.error_message = None;
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
        self.messages = messages.into();
        self.loading_state = LoadingState::Loaded;
        self.error_message = None;
    }

    pub fn add_message(&mut self, message: Message) {
        if self.channel_id == Some(message.channel_id())
            && !self.messages.iter().any(|m| m.id() == message.id())
        {
            self.authors.insert(
                message.author().id().to_string(),
                message.author().display_name(),
            );
            for mention in message.mentions() {
                self.authors
                    .insert(mention.id().to_string(), mention.display_name());
            }
            self.messages.push_back(message);
        }
    }

    pub fn prepend_messages(&mut self, new_messages: Vec<Message>) -> usize {
        let existing_ids: HashSet<_> = self
            .messages
            .iter()
            .map(crate::domain::entities::Message::id)
            .collect();
        let mut added = 0;
        for msg in new_messages.into_iter().rev() {
            if !existing_ids.contains(&msg.id()) {
                self.authors
                    .insert(msg.author().id().to_string(), msg.author().display_name());
                for mention in msg.mentions() {
                    self.authors
                        .insert(mention.id().to_string(), mention.display_name());
                }
                self.messages.push_front(msg);
                added += 1;
            }
        }
        added
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
        self.channel_topic = None;
        self.channel_icon = None;
        self.online_count = None;
        self.messages.clear();
        self.loading_state = LoadingState::Idle;
        self.error_message = None;
        self.is_dm = false;
        self.typing_indicator = None;
        self.authors.clear();
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
    pub fn messages(&self) -> &VecDeque<Message> {
        &self.messages
    }

    #[must_use]
    pub const fn loading_state(&self) -> LoadingState {
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

    #[must_use]
    pub const fn is_dm(&self) -> bool {
        self.is_dm
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
    scroll_state: ScrollViewState,
    selected_index: Option<usize>,
    focused: bool,
    auto_scroll: bool,
    scroll_to_selection: bool,
    content_height: u16,
    viewport_height: u16,
    last_width: u16,
}

impl MessagePaneState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll_state: ScrollViewState::new(),
            selected_index: None,
            focused: false,
            auto_scroll: true,
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

    pub fn adjust_for_prepend(&mut self, added_count: usize, added_height: u16) {
        if let Some(idx) = self.selected_index {
            self.selected_index = Some(idx + added_count);
        }

        let current_offset = self.scroll_state.offset();
        let new_y = current_offset.y.saturating_add(added_height);
        self.scroll_state
            .set_offset(ratatui::layout::Position { x: 0, y: new_y });
    }

    pub fn select_next(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.auto_scroll = false;
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

        self.auto_scroll = false;
        self.scroll_to_selection = true;
        self.selected_index = Some(self.selected_index.map_or_else(
            || message_count.saturating_sub(1),
            |idx| idx.saturating_sub(1),
        ));
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn select_first(&mut self) {
        self.auto_scroll = false;
        self.scroll_to_selection = true;
        self.selected_index = Some(0);
        self.scroll_state.scroll_to_top();
    }

    pub fn select_last(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }
        self.selected_index = Some(message_count - 1);
        self.auto_scroll = true;
        self.scroll_to_selection = true;
        self.scroll_to_bottom();
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn clear_selection(&mut self) {
        self.selected_index = None;
        self.auto_scroll = true;
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_down(&mut self) {
        self.auto_scroll = false;
        for _ in 0..SCROLL_AMOUNT {
            self.scroll_state.scroll_down();
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        for _ in 0..SCROLL_AMOUNT {
            self.scroll_state.scroll_up();
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_to_bottom(&mut self) {
        if self.content_height > self.viewport_height {
            let offset = self.content_height.saturating_sub(self.viewport_height);
            self.scroll_state
                .set_offset(ratatui::layout::Position { x: 0, y: offset });
        }
    }

    #[allow(clippy::missing_const_for_fn)]
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
                None
            }
            Some(Action::ScrollUp) => {
                if self.scroll_state.offset().y == 0 {
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
                self.auto_scroll = true;
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
        }
    }
}

#[allow(missing_docs)]
pub struct MessagePane<'a> {
    data: &'a MessagePaneData,
    style: MessagePaneStyle,
    markdown_service: &'a MarkdownService,
}

impl<'a> MessagePane<'a> {
    #[must_use]
    pub fn new(data: &'a MessagePaneData, markdown_service: &'a MarkdownService) -> Self {
        Self {
            data,
            style: MessagePaneStyle::default(),
            markdown_service,
        }
    }

    #[must_use]
    pub const fn style(mut self, style: MessagePaneStyle) -> Self {
        self.style = style;
        self
    }

    fn calculate_content_height(&self, width: u16) -> u16 {
        self.data
            .messages()
            .iter()
            .map(|m| self.calculate_message_height(m, width, self.markdown_service))
            .sum()
    }

    fn calculate_content_lines_count(
        &self,
        markdown_service: &MarkdownService,
        message: &Message,
        width: u16,
    ) -> u16 {
        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);
        let content_width = (width)
            .saturating_sub(indent_width)
            .saturating_sub(SCROLLBAR_MARGIN);

        let text = markdown_service.render(message.content(), Some(self.data));

        let mut content_lines = 0;
        for line in text.lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            content_lines +=
                u16::try_from(wrap_text(&line_text, content_width as usize).len()).unwrap_or(0);
        }
        content_lines
    }

    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn calculate_message_height(
        &self,
        message: &Message,
        width: u16,
        markdown_service: &MarkdownService,
    ) -> u16 {
        let content_lines = self.calculate_content_lines_count(markdown_service, message, width);

        let mut height = 1 + content_lines;

        if message.is_reply() && message.referenced().is_some() {
            height += 1;
        }

        if message.has_attachments() {
            height += u16::try_from(message.attachments().len()).unwrap_or(u16::MAX);
        }

        height
    }

    fn render_reply(
        &self,
        message: &Message,
        width: u16,
        base_style: Style,
        is_selected: bool,
        current_y: &mut u16,
        scroll_view: &mut ScrollView,
    ) {
        if message.is_reply()
            && let Some(referenced) = message.referenced()
        {
            let reply_text = format!(
                "↱ Replying to {}: {}",
                referenced.author().display_name(),
                truncate_string(referenced.content(), 50)
            );

            let reply_style = if is_selected {
                self.style.reply_style.fg(Color::White)
            } else {
                self.style.reply_style
            };

            let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
            let reply_line = Line::from(vec![indent_span, Span::styled(reply_text, reply_style)]);
            let reply_block = Block::default()
                .padding(Padding::new(0, SCROLLBAR_MARGIN, 0, 0))
                .style(base_style);
            let reply_para = Paragraph::new(reply_line)
                .block(reply_block)
                .style(base_style);
            scroll_view.render_widget(reply_para, Rect::new(0, *current_y, width, 1));
            *current_y += 1;
        }
    }

    fn render_header(
        &self,
        message: &Message,
        width: u16,
        base_style: Style,
        is_selected: bool,
        current_y: &mut u16,
        scroll_view: &mut ScrollView,
    ) {
        let (timestamp_style, edited_style) = if is_selected {
            (
                Style::default().fg(Color::White),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            )
        } else {
            (self.style.timestamp_style, self.style.edited_style)
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
            Span::styled(message.author().display_name(), self.style.author_style),
        ];

        if message.author().is_bot() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("[BOT]", self.style.bot_badge_style));
        }

        if message.is_edited() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled("(edited)", edited_style));
        }

        let header_line = Line::from(header_spans);
        let header_block = Block::default()
            .padding(Padding::new(0, SCROLLBAR_MARGIN, 0, 0))
            .style(base_style);
        let header_para = Paragraph::new(header_line)
            .block(header_block)
            .style(base_style);
        scroll_view.render_widget(header_para, Rect::new(0, *current_y, width, 1));
        *current_y += 1;
    }

    fn render_attachments(
        &self,
        message: &Message,
        width: u16,
        base_style: Style,
        current_y: &mut u16,
        scroll_view: &mut ScrollView,
    ) {
        let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
        for attachment in message.attachments() {
            let attachment_text = format!("📎 {}", attachment.filename());
            let attachment_line = Line::from(vec![
                indent_span.clone(),
                Span::styled(attachment_text, self.style.attachment_style),
            ]);
            let attachment_block = Block::default()
                .padding(Padding::new(0, SCROLLBAR_MARGIN, 0, 0))
                .style(base_style);
            let attachment_para = Paragraph::new(attachment_line)
                .block(attachment_block)
                .style(base_style);
            scroll_view.render_widget(attachment_para, Rect::new(0, *current_y, width, 1));
            *current_y += 1;
        }
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

        self.render_reply(
            message,
            width,
            base_style,
            is_selected,
            &mut current_y,
            scroll_view,
        );
        self.render_header(
            message,
            width,
            base_style,
            is_selected,
            &mut current_y,
            scroll_view,
        );

        let content_style = if message.kind().is_system() {
            self.style.system_message_style
        } else {
            self.style.content_style
        };

        let paragraph_style = if is_selected {
            base_style
        } else {
            base_style.patch(content_style)
        };

        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);

        let text = self
            .markdown_service
            .render(message.content(), Some(self.data));
        let rendered_text = text;

        let block = Block::default()
            .padding(Padding::new(indent_width, SCROLLBAR_MARGIN, 0, 0))
            .style(paragraph_style);
        let para = Paragraph::new(rendered_text)
            .block(block)
            .style(paragraph_style)
            .wrap(ratatui::widgets::Wrap { trim: false });

        let content_height =
            self.calculate_content_lines_count(self.markdown_service, message, width);

        scroll_view.render_widget(para, Rect::new(0, current_y, width, content_height));

        current_y += content_height;

        self.render_attachments(message, width, base_style, &mut current_y, scroll_view);

        current_y - y_offset
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

impl StatefulWidget for MessagePane<'_> {
    type State = MessagePaneState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = self.build_block(state);
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
        state.last_width = content_width;

        let vertical_scroll = if content_height <= inner_area.height {
            ScrollbarVisibility::Never
        } else {
            ScrollbarVisibility::Always
        };

        let mut scroll_view = ScrollView::new(Size::new(content_width, content_height))
            .horizontal_scrollbar_visibility(ScrollbarVisibility::Never)
            .vertical_scrollbar_visibility(vertical_scroll);

        let mut y_offset: u16 = 0;
        let mut selected_position = None;

        for (idx, message) in self.data.messages().iter().enumerate() {
            let is_selected = state.selected_index == Some(idx);
            let msg_height = self.render_message(
                message,
                y_offset,
                content_width,
                is_selected,
                &mut scroll_view,
            );

            if is_selected {
                selected_position = Some((y_offset, msg_height));
            }

            y_offset += msg_height;
        }

        if state.scroll_to_selection {
            if let Some((msg_y, msg_height)) = selected_position {
                let current_scroll = state.scroll_state.offset().y;
                let viewport_height = inner_area.height;

                let new_scroll = if msg_y < current_scroll {
                    Some(msg_y)
                } else if msg_y.saturating_add(msg_height)
                    > current_scroll.saturating_add(viewport_height)
                {
                    Some(
                        msg_y
                            .saturating_add(msg_height)
                            .saturating_sub(viewport_height),
                    )
                } else {
                    None
                };

                if let Some(scroll) = new_scroll {
                    state
                        .scroll_state
                        .set_offset(ratatui::layout::Position { x: 0, y: scroll });
                }
            }
            state.scroll_to_selection = false;
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
}
