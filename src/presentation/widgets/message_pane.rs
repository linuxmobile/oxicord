use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::LazyLock;

use crate::application::services::identity_resolver::IdentityResolver;
use crate::application::services::markdown_parser::{
    MdBlock, MdInline, MentionResolver, parse_markdown,
};
use crate::application::services::url_extractor::UrlExtractor;
use crate::domain::entities::{
    ChannelId, Embed, ForumThread, ImageId, Message, MessageId, RelationshipState,
};
use crate::domain::keybinding::Action;

use crate::presentation::commands::CommandRegistry;
use crate::presentation::services::markdown_renderer::MarkdownRenderer;

use crossterm::event::KeyEvent;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, StatefulWidget, Widget},
};
use regex::Regex;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

use super::image_state::{ImageAttachment, MAX_IMAGE_HEIGHT};
use crate::presentation::theme::Theme;
use crate::presentation::ui::utils::{clean_text, get_author_color};

const SCROLL_AMOUNT: u16 = 3;
const SCROLLBAR_MARGIN: u16 = 2;
const CHANNEL_NAME_PREFIX: &str = "[ ";
const CHANNEL_NAME_SUFFIX: &str = " ]";
const DM_CHANNEL_PREFIX: &str = "[ ";
const TIMESTAMP_WIDTH: usize = 6;
const CONTENT_INDENT: usize = 6;
const EMBED_INDENT: usize = 6;
const THREAD_CARD_HEIGHT: u16 = 6;
const GROUPING_WINDOW_SECONDS: i64 = 7 * 60;

/// Pre-calculated layout data for an embed.
pub struct RenderedEmbed {
    pub provider: Option<String>,
    pub title: Vec<String>,
    pub description: Option<Text<'static>>,
    pub description_height: u16,
    pub height: u16,
    pub color: Color,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageGroup {
    #[default]
    Start,
    Compact,
}

/// Represents a renderable item in the message list.
/// Either a single visible message or a collapsed run of blocked messages.
enum RenderItem {
    /// A single visible message with its original index.
    Message { idx: usize },
    /// A run of consecutive blocked messages.
    BlockedRun {
        count: usize,
        /// Index range [start, end) of blocked messages.
        start_idx: usize,
    },
}

/// UI wrapper for a message with rendering state.
pub struct UiMessage {
    pub message: Arc<Message>,
    pub estimated_height: u16,
    pub rendered_content: Option<Text<'static>>,
    pub parsed_content: Vec<MdBlock>,
    /// Image attachments for this message.
    pub image_attachments: Vec<ImageAttachment>,
    /// Pre-calculated embed layouts.
    pub rendered_embeds: Vec<RenderedEmbed>,
    /// Cached reply preview line
    pub reply_preview: Option<Line<'static>>,
    pub group: MessageGroup,
}

impl UiMessage {
    fn new(message: Message) -> Self {
        let mut image_attachments: Vec<ImageAttachment> = message
            .attachments()
            .iter()
            .filter_map(ImageAttachment::from_attachment)
            .collect();

        let content = message.content();
        let parsed_content = parse_markdown(content);
        let inline_images = UrlExtractor::extract_image_urls(content);
        for url in inline_images {
            if !image_attachments.iter().any(|img| img.url == url) {
                let id = crate::domain::entities::ImageId::from_url(&url);
                image_attachments.push(ImageAttachment::new(id, url, None, None));
            }
        }

        Self {
            message: Arc::new(message),
            estimated_height: 1,
            rendered_content: None,
            parsed_content,
            image_attachments,
            rendered_embeds: Vec::new(),
            reply_preview: None,
            group: MessageGroup::Start,
        }
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn total_image_height(&self, width: u16) -> u16 {
        self.image_attachments
            .iter()
            .map(|img| img.height(width))
            .fold(0u16, u16::saturating_add)
    }

    /// Returns true if this message has any image attachments.
    #[must_use]
    pub fn has_images(&self) -> bool {
        !self.image_attachments.is_empty()
    }

    /// Returns true if any images need loading.
    #[must_use]
    pub fn needs_image_load(&self) -> bool {
        self.image_attachments
            .iter()
            .any(ImageAttachment::needs_load)
    }

    /// Collects image IDs that need loading.
    #[must_use]
    pub fn collect_image_loads(&self) -> Vec<(ImageId, String)> {
        self.image_attachments
            .iter()
            .filter(|img| img.needs_load())
            .map(|img| (img.id.clone(), img.url.clone()))
            .collect()
    }
}

struct HashMapResolver<'a> {
    authors: &'a HashMap<String, String>,
    channels: &'a HashMap<String, String>,
}

impl MentionResolver for HashMapResolver<'_> {
    fn resolve(&self, user_id: &str) -> Option<String> {
        self.authors.get(user_id).cloned()
    }

    fn resolve_channel(&self, channel_id: &str) -> Option<String> {
        self.channels.get(channel_id).cloned()
    }
}

fn calculate_embed_layout(
    embed: &Embed,
    width: u16,
    markdown_service: &MarkdownRenderer,
    default_color: Color,
) -> RenderedEmbed {
    let mut height = 0;
    let width = width.saturating_sub(2);

    let mut title_lines = Vec::new();
    let mut description_text = None;
    let mut description_height = 0;

    if embed
        .provider
        .as_ref()
        .and_then(|p| p.name.as_ref())
        .is_some()
    {
        height += 1;
    }

    if let Some(title) = &embed.title {
        title_lines = wrap_text(title, width as usize);
        height += u16::try_from(title_lines.len()).unwrap_or(u16::MAX);
    }

    if let Some(description) = &embed.description {
        let text = markdown_service.render_markdown(description, None, false);

        let wrapped_text = wrap_styled_text(text, width);
        let content_lines = u16::try_from(wrapped_text.lines.len()).unwrap_or(0);

        description_height = content_lines;
        height += description_height;
        description_text = Some(wrapped_text);
    }

    if height > 0 {
        height += 2;
    }

    let color = if let Some(c) = embed.color {
        Color::Rgb(
            u8::try_from((c >> 16) & 0xFF).unwrap_or(0),
            u8::try_from((c >> 8) & 0xFF).unwrap_or(0),
            u8::try_from(c & 0xFF).unwrap_or(0),
        )
    } else {
        default_color
    };

    RenderedEmbed {
        provider: embed.provider.as_ref().and_then(|p| p.name.clone()),
        title: title_lines,
        description: description_text,
        description_height,
        height,
        color,
        url: embed.url.clone(),
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
    CopyImage(ImageId),
    YankContent(String),
    YankUrl(String),
    YankId(String),
    OpenAttachments(MessageId),
    JumpToReply(MessageId),
    LoadHistory,
    OpenThread(ChannelId),
    CloseThread,
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
    channels: HashMap<String, String>,
    last_layout_width: Option<u16>,
    last_show_spoilers: Option<bool>,
    last_image_preview: Option<bool>,
    is_dirty: bool,
    use_display_name: bool,
}

impl MessagePaneData {
    #[must_use]
    pub fn new(use_display_name: bool) -> Self {
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
            channels: HashMap::new(),
            last_layout_width: None,
            last_show_spoilers: None,
            last_image_preview: None,
            is_dirty: true,
            use_display_name,
        }
    }

    pub fn set_channel(&mut self, channel_id: ChannelId, channel_name: String) {
        self.is_dm = channel_name.starts_with('@');
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name);
        self.channel_topic = None;
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
            self.authors.insert(
                msg.author().id().to_string(),
                IdentityResolver::with_preference(self.use_display_name).resolve(msg.author()),
            );
            for mention in msg.mentions() {
                self.authors.insert(
                    mention.id().to_string(),
                    IdentityResolver::with_preference(self.use_display_name).resolve(mention),
                );
            }
        }
        self.messages = messages.into_iter().map(UiMessage::new).collect();
        self.update_grouping();
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
                IdentityResolver::with_preference(self.use_display_name).resolve(message.author()),
            );
            for mention in message.mentions() {
                self.authors.insert(
                    mention.id().to_string(),
                    IdentityResolver::with_preference(self.use_display_name).resolve(mention),
                );
            }
            self.messages.push_back(UiMessage::new(message));
            self.update_grouping();
            self.is_dirty = true;
        }
    }

    pub fn prepend_messages(&mut self, new_messages: Vec<Message>) -> usize {
        let existing_ids: HashSet<_> = self.messages.iter().map(|m| m.message.id()).collect();
        let mut added = 0;
        for msg in new_messages.into_iter().rev() {
            if !existing_ids.contains(&msg.id()) {
                self.authors.insert(
                    msg.author().id().to_string(),
                    IdentityResolver::with_preference(self.use_display_name).resolve(msg.author()),
                );
                for mention in msg.mentions() {
                    self.authors.insert(
                        mention.id().to_string(),
                        IdentityResolver::with_preference(self.use_display_name).resolve(mention),
                    );
                }
                self.messages.push_front(UiMessage::new(msg));
                added += 1;
            }
        }
        if added > 0 {
            self.update_grouping();
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
            self.update_grouping();
            self.is_dirty = true;
        }
    }

    pub fn remove_message(&mut self, message_id: MessageId) {
        self.messages.retain(|m| m.message.id() != message_id);
        self.update_grouping();
        self.is_dirty = true;
    }

    fn update_grouping(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let mut previous_author_id: Option<String> = None;
        let mut previous_timestamp: Option<i64> = None;

        for ui_msg in &mut self.messages {
            let msg = &ui_msg.message;
            let current_author_id = msg.author().id().to_string();
            let current_timestamp = msg.timestamp().timestamp();

            ui_msg.group = MessageGroup::Start;

            if !msg.is_reply()
                && let (Some(prev_id), Some(prev_ts)) = (&previous_author_id, previous_timestamp)
                && prev_id == &current_author_id
            {
                let diff = current_timestamp.saturating_sub(prev_ts);
                if diff < GROUPING_WINDOW_SECONDS {
                    ui_msg.group = MessageGroup::Compact;
                }
            }

            previous_author_id = Some(current_author_id);
            previous_timestamp = Some(current_timestamp);
        }
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
        self.channels.clear();
        self.is_dirty = true;
    }

    pub fn set_use_display_name(&mut self, use_display_name: bool) {
        self.use_display_name = use_display_name;
        self.refresh_authors();
    }

    pub fn refresh_authors(&mut self) {
        self.authors.clear();
        for ui_msg in &self.messages {
            self.authors.insert(
                ui_msg.message.author().id().to_string(),
                IdentityResolver::with_preference(self.use_display_name)
                    .resolve(ui_msg.message.author()),
            );
            for mention in ui_msg.message.mentions() {
                self.authors.insert(
                    mention.id().to_string(),
                    IdentityResolver::with_preference(self.use_display_name).resolve(mention),
                );
            }
        }
        self.is_dirty = true;
    }

    pub fn set_typing_indicator(&mut self, indicator: Option<String>) {
        self.typing_indicator = indicator;
    }

    pub fn register_channel(&mut self, id: String, name: String) {
        self.channels.insert(id, name);
    }

    #[must_use]
    pub fn is_channel_known(&self, id: &str) -> bool {
        self.channels.contains_key(id)
    }

    /// Marks the data as dirty, requiring re-layout.
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Forces a re-layout of all messages, including re-parsing if resolver data changed.
    pub fn force_refresh_layout(&mut self) {
        self.is_dirty = true;
        self.last_layout_width = None;
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
        self.messages.get(index).map(|m| m.message.as_ref())
    }

    #[must_use]
    pub fn ui_messages(&self) -> &VecDeque<UiMessage> {
        &self.messages
    }

    pub fn ui_messages_mut(&mut self) -> &mut VecDeque<UiMessage> {
        &mut self.messages
    }

    pub fn update_layout(
        &mut self,
        width: u16,
        markdown_service: &MarkdownRenderer,
        default_color: Color,
        show_spoilers: bool,
        image_preview: bool,
    ) {
        if !self.is_dirty
            && self.last_layout_width == Some(width)
            && self.last_show_spoilers == Some(show_spoilers)
            && self.last_image_preview == Some(image_preview)
            && !self.messages.iter().any(|m| m.rendered_content.is_none())
        {
            return;
        }

        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);
        let content_width = width
            .saturating_sub(indent_width)
            .saturating_sub(SCROLLBAR_MARGIN);

        if content_width == 0 {
            return;
        }

        let authors = &self.authors;
        let channels = &self.channels;
        let resolver = HashMapResolver { authors, channels };

        for ui_msg in &mut self.messages {
            Self::layout_message(
                ui_msg,
                content_width,
                markdown_service,
                default_color,
                show_spoilers,
                image_preview,
                &resolver,
                authors,
                self.use_display_name,
            );
        }

        self.last_layout_width = Some(width);
        self.last_show_spoilers = Some(show_spoilers);
        self.last_image_preview = Some(image_preview);
        self.is_dirty = false;
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    fn layout_message(
        ui_msg: &mut UiMessage,
        content_width: u16,
        markdown_service: &MarkdownRenderer,
        default_color: Color,
        show_spoilers: bool,
        image_preview: bool,
        resolver: &HashMapResolver<'_>,
        authors: &HashMap<String, String>,
        use_display_name: bool,
    ) {
        let message = &ui_msg.message;

        let text =
            markdown_service.render(ui_msg.parsed_content.clone(), Some(resolver), show_spoilers);

        let wrapped_text = wrap_styled_text(text, content_width);
        let content_lines = u16::try_from(wrapped_text.lines.len()).unwrap_or(0);

        ui_msg.rendered_content = Some(wrapped_text);

        let mut height = content_lines;

        if ui_msg.group == MessageGroup::Start {
            height += 1;
        }

        if message.is_reply() {
            height += 1;
        }

        let non_image_attachments = message
            .attachments()
            .iter()
            .filter(|a| !a.is_image())
            .count();
        height += u16::try_from(non_image_attachments).unwrap_or(0);

        if image_preview {
            height += ui_msg.total_image_height(content_width);
        } else {
            height += u16::try_from(ui_msg.image_attachments.len()).unwrap_or(0);
        }

        let mut rendered_embeds = Vec::new();
        for embed in message.embeds() {
            let layout =
                calculate_embed_layout(embed, content_width, markdown_service, default_color);
            height += layout.height;
            rendered_embeds.push(layout);
        }
        ui_msg.rendered_embeds = rendered_embeds;

        if message.is_reply() {
            if let Some(referenced) = message.referenced() {
                static MENTION_RE: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"<@!?(\d+)>").unwrap());

                let content = referenced.content();
                let resolved_content = MENTION_RE.replace_all(content, |caps: &regex::Captures| {
                    let id = &caps[1];
                    authors
                        .get(id)
                        .map_or_else(|| format!("@{id}"), |name| format!("@{name}"))
                });

                let snippet = truncate_string(&resolved_content, 50);

                let reply_style = Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC);
                let username_style = Style::default().fg(Color::Cyan);

                let spans = vec![
                    Span::raw(" ".repeat(CONTENT_INDENT)),
                    Span::styled("┌─ Replying to ", reply_style),
                    Span::styled(
                        IdentityResolver::with_preference(use_display_name)
                            .resolve(referenced.author()),
                        username_style,
                    ),
                    Span::styled(format!(": {snippet}"), reply_style),
                ];
                ui_msg.reply_preview = Some(Line::from(spans));
            } else {
                let error_style = Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::ITALIC);
                let spans = vec![
                    Span::raw(" ".repeat(CONTENT_INDENT)),
                    Span::styled("┌─ Original message unavailable", error_style),
                ];
                ui_msg.reply_preview = Some(Line::from(spans));
            }
        }

        ui_msg.estimated_height = height;
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
            let clean_name = clean_text(name);
            let display_name = clean_name.trim_start_matches('@').to_uppercase();
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

#[derive(Debug, Clone, Default)]
pub struct ForumState {
    pub threads: Vec<ForumThread>,
    pub selected_idx: usize,
    pub scroll_offset: u16,
    pub needs_scroll_to_selection: bool,
}

#[derive(Debug, Clone, Default)]
pub enum ViewMode {
    #[default]
    Messages,
    Forum(ForumState),
}

impl MentionResolver for MessagePaneData {
    fn resolve(&self, user_id: &str) -> Option<String> {
        self.authors.get(user_id).cloned()
    }

    fn resolve_channel(&self, channel_id: &str) -> Option<String> {
        self.channels.get(channel_id).cloned()
    }
}

impl Default for MessagePaneData {
    fn default() -> Self {
        Self::new(true)
    }
}

pub struct MessagePaneFlags {
    pub focused: bool,
    pub is_following: bool,
    pub scroll_to_selection: bool,
}

pub struct MessagePaneState {
    pub view_mode: ViewMode,
    pub vertical_scroll: usize,
    pub show_spoilers: bool,
    selected_index: Option<usize>,
    flags: MessagePaneFlags,
    content_height: usize,
    viewport_height: u16,
    last_width: u16,
}

impl MessagePaneState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            view_mode: ViewMode::Messages,
            vertical_scroll: 0,
            show_spoilers: false,
            selected_index: None,
            flags: MessagePaneFlags {
                focused: false,
                is_following: true,
                scroll_to_selection: false,
            },
            content_height: 0,
            viewport_height: 0,
            last_width: 0,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.flags.focused = focused;
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.flags.focused
    }

    #[must_use]
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    #[must_use]
    pub const fn viewport_height(&self) -> u16 {
        self.viewport_height
    }

    #[must_use]
    pub const fn last_width(&self) -> u16 {
        self.last_width
    }

    pub fn toggle_spoiler(&mut self) {
        self.show_spoilers = !self.show_spoilers;
    }

    pub fn adjust_for_prepend(&mut self, added_count: usize, added_height: usize) {
        if let Some(idx) = self.selected_index {
            self.selected_index = Some(idx + added_count);
        }

        self.vertical_scroll = self.vertical_scroll.saturating_add(added_height);
        self.content_height = self.content_height.saturating_add(added_height);
    }

    pub fn jump_to_index(&mut self, index: usize) {
        self.selected_index = Some(index);
        self.flags.scroll_to_selection = true;
        self.flags.is_following = false;
    }

    pub fn select_next(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;
        self.selected_index = Some(self.selected_index.map_or_else(
            || message_count.saturating_sub(1),
            |idx| (idx + 1).min(message_count - 1),
        ));
    }

    pub fn select_previous(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }

        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;
        self.selected_index = Some(self.selected_index.map_or_else(
            || message_count.saturating_sub(1),
            |idx| idx.saturating_sub(1),
        ));
    }

    /// Navigate to the next visible render item (skips blocked runs as single unit).
    fn select_next_visible(&mut self, render_items: &[RenderItem], message_count: usize) {
        if render_items.is_empty() || message_count == 0 {
            return;
        }

        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;

        let current_render_idx = self.find_current_render_item_index(render_items);

        let next_render_idx = match current_render_idx {
            Some(idx) => (idx + 1).min(render_items.len() - 1),
            None => render_items.len() - 1,
        };

        self.selected_index = Some(Self::message_index_for_render_item(
            &render_items[next_render_idx],
        ));
    }

    /// Navigate to the previous visible render item (skips blocked runs as single unit).
    fn select_previous_visible(&mut self, render_items: &[RenderItem], message_count: usize) {
        if render_items.is_empty() || message_count == 0 {
            return;
        }

        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;

        let current_render_idx = self.find_current_render_item_index(render_items);

        let prev_render_idx = match current_render_idx {
            Some(idx) => idx.saturating_sub(1),
            None => render_items.len() - 1, // Start from last if no selection
        };

        self.selected_index = Some(Self::message_index_for_render_item(
            &render_items[prev_render_idx],
        ));
    }

    /// Find the render item index containing the current selection.
    fn find_current_render_item_index(&self, render_items: &[RenderItem]) -> Option<usize> {
        let selected = self.selected_index?;
        render_items.iter().position(|item| match item {
            RenderItem::Message { idx } => *idx == selected,
            RenderItem::BlockedRun { start_idx, count } => {
                selected >= *start_idx && selected < *start_idx + *count
            }
        })
    }

    /// Get the representative message index for a render item.
    fn message_index_for_render_item(item: &RenderItem) -> usize {
        match item {
            RenderItem::Message { idx } => *idx,
            RenderItem::BlockedRun { start_idx, .. } => *start_idx,
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn select_first(&mut self) {
        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;
        self.selected_index = Some(0);
        self.scroll_to_top();
    }

    pub fn select_last(&mut self, message_count: usize) {
        if message_count == 0 {
            return;
        }
        self.selected_index = Some(message_count - 1);
        self.flags.is_following = false;
        self.flags.scroll_to_selection = true;
        self.scroll_to_bottom();
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn clear_selection(&mut self) {
        self.selected_index = None;
        self.flags.is_following = true;
        self.scroll_to_bottom();
    }

    pub fn on_new_message(&mut self) {
        if self.flags.is_following {
            self.scroll_to_bottom();
        }
    }

    /// Resets the state for a new channel - clears selection, enables following, resets scroll.
    /// This ensures new channels start at the bottom (following mode).
    pub fn on_channel_change(&mut self) {
        self.selected_index = None;
        self.flags.is_following = true;
        self.vertical_scroll = 0;
        self.content_height = 0;
        self.viewport_height = 0;
        self.view_mode = ViewMode::Messages;
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_down(&mut self) {
        self.flags.is_following = false;
        let max_scroll = self
            .content_height
            .saturating_sub(self.viewport_height as usize);
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_add(SCROLL_AMOUNT as usize)
            .min(max_scroll);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_up(&mut self) {
        self.flags.is_following = false;
        self.vertical_scroll = self.vertical_scroll.saturating_sub(SCROLL_AMOUNT as usize);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_to_bottom(&mut self) {
        if self.content_height > self.viewport_height as usize {
            self.vertical_scroll = self.content_height - self.viewport_height as usize;
        } else {
            self.vertical_scroll = 0;
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_to_top(&mut self) {
        self.vertical_scroll = 0;
    }

    pub fn update_dimensions(&mut self, content_height: usize, viewport_height: u16) {
        self.content_height = content_height;
        self.viewport_height = viewport_height;

        if self.flags.is_following {
            self.scroll_to_bottom();
        } else {
            let max_scroll = content_height.saturating_sub(viewport_height as usize);
            if self.vertical_scroll > max_scroll {
                self.vertical_scroll = max_scroll;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        data: &MessagePaneData,
        registry: &CommandRegistry,
        relationship_state: Option<&RelationshipState>,
        hide_blocked_completely: bool,
    ) -> Option<MessagePaneAction> {
        if let ViewMode::Forum(forum_state) = &mut self.view_mode {
            match registry.find_action(key) {
                Some(Action::NavigateDown) => {
                    let count = forum_state.threads.len();
                    if count > 0 {
                        forum_state.selected_idx = (forum_state.selected_idx + 1).min(count - 1);

                        let visible_items = self.viewport_height / THREAD_CARD_HEIGHT;
                        let selected = u16::try_from(forum_state.selected_idx).unwrap_or(u16::MAX);
                        let offset = forum_state.scroll_offset;

                        if selected >= offset + visible_items {
                            forum_state.scroll_offset =
                                (selected + 1).saturating_sub(visible_items);
                        }
                    }
                    return None;
                }
                Some(Action::NavigateUp) => {
                    if forum_state.selected_idx == 0 {
                        return Some(MessagePaneAction::LoadHistory);
                    }
                    forum_state.selected_idx = forum_state.selected_idx.saturating_sub(1);

                    let selected = u16::try_from(forum_state.selected_idx).unwrap_or(0);
                    if selected < forum_state.scroll_offset {
                        forum_state.scroll_offset = selected;
                    }
                    return None;
                }
                Some(Action::Select | Action::NavigateRight) => {
                    if let Some(thread) = forum_state.threads.get(forum_state.selected_idx) {
                        return Some(MessagePaneAction::OpenThread(thread.id));
                    }
                    return None;
                }
                Some(Action::Cancel | Action::NavigateLeft) => {
                    return Some(MessagePaneAction::CloseThread);
                }
                _ => return None,
            }
        }

        let message_count = data.message_count();

        let render_items = build_render_items(
            data.ui_messages(),
            relationship_state,
            hide_blocked_completely,
        );

        match registry.find_action(key) {
            Some(Action::NavigateDown) => {
                self.select_next_visible(&render_items, message_count);
                None
            }
            Some(Action::NavigateUp) => {
                let at_top = self.selected_index.is_some()
                    && render_items.first().is_some_and(|item| match item {
                        RenderItem::Message { idx } => self.selected_index == Some(*idx),
                        RenderItem::BlockedRun { start_idx, count } => self
                            .selected_index
                            .is_some_and(|sel| sel >= *start_idx && sel < *start_idx + *count),
                    });
                if at_top {
                    return Some(MessagePaneAction::LoadHistory);
                }
                self.select_previous_visible(&render_items, message_count);
                None
            }
            Some(Action::ScrollDown) => {
                self.scroll_down();
                let max_scroll = self
                    .content_height
                    .saturating_sub(self.viewport_height as usize);
                if self.vertical_scroll == max_scroll {
                    self.flags.is_following = true;
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
                    self.flags.is_following = true;
                }
                None
            }
            Some(Action::Cancel | Action::ClearSelection) => {
                if self.selected_index.is_some() {
                    self.clear_selection();
                    Some(MessagePaneAction::ClearSelection)
                } else {
                    Some(MessagePaneAction::CloseThread)
                }
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
            Some(Action::CopyImage) => self.get_selected_message(data).and_then(|m| {
                data.ui_messages()
                    .iter()
                    .find(|ui| ui.message.id() == m.id())
                    .and_then(|ui| {
                        ui.image_attachments
                            .first()
                            .map(|img| MessagePaneAction::CopyImage(img.id.clone()))
                    })
            }),
            Some(Action::YankId) => self
                .get_selected_message_id(data)
                .map(|id| MessagePaneAction::YankId(id.to_string())),
            Some(Action::OpenAttachments) => self
                .get_selected_message_id(data)
                .map(MessagePaneAction::OpenAttachments),
            Some(Action::JumpToReply) => {
                if let Some(msg) = self.get_selected_message(data)
                    && let Some(reference) = msg.reference()
                    && let Some(message_id) = reference.message_id
                {
                    return Some(MessagePaneAction::JumpToReply(message_id));
                }

                if let Some(msg) = self.get_selected_message(data)
                    && let Some(ui_msg) = data
                        .ui_messages()
                        .iter()
                        .find(|m| m.message.id() == msg.id())
                {
                    for block in &ui_msg.parsed_content {
                        if let MdBlock::Paragraph(inlines) = block {
                            for inline in inlines {
                                if let MdInline::Channel(channel_id) = inline {
                                    return Some(MessagePaneAction::OpenThread(ChannelId(
                                        channel_id.parse().unwrap_or(0),
                                    )));
                                }
                            }
                        }
                    }
                }
                None
            }
            Some(Action::Select) => {
                self.show_spoilers = !self.show_spoilers;
                None
            }

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
    pub mention_style: Style,
    pub reply_style: Style,
    pub attachment_style: Style,
    pub system_message_style: Style,
    pub loading_style: Style,
    pub error_style: Style,
    pub empty_style: Style,
    pub scrollbar_track_style: Style,
    pub scrollbar_thumb_style: Style,
    pub blocked_style: Style,
}

impl MessagePaneStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        use crate::presentation::theme::adapter::ColorConverter;

        let accent_hsl = ColorConverter::to_hsl(theme.accent);
        let mut blocked_hsl = accent_hsl;
        blocked_hsl.l = 0.35;
        blocked_hsl.s = (blocked_hsl.s * 0.4).clamp(0.0, 1.0);
        let blocked_fg = ColorConverter::to_ratatui(blocked_hsl);

        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(theme.accent),
            title_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            author_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            selected_style: theme.selection_style,
            mention_style: theme.mention_style,
            reply_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            timestamp_style: Style::default().fg(Color::DarkGray),
            loading_style: Style::default().fg(theme.accent),
            blocked_style: Style::default()
                .fg(blocked_fg)
                .add_modifier(Modifier::ITALIC),
            ..Self::default()
        }
    }
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
            mention_style: Style::default().bg(Color::Rgb(50, 50, 20)),
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
            blocked_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        }
    }
}

#[allow(missing_docs)]
pub struct MessagePane<'a> {
    data: &'a mut MessagePaneData,
    style: MessagePaneStyle,
    disable_user_colors: bool,
    image_preview: bool,
    timestamp_format: &'a str,
    current_user_id: Option<String>,
    markdown_service: &'a MarkdownRenderer,
    /// Optional reference to relationship state for blocked user filtering.
    relationship_state: Option<&'a RelationshipState>,
    /// If true, completely hide blocked messages; if false, show placeholder.
    hide_blocked_completely: bool,
}

impl<'a> MessagePane<'a> {
    #[must_use]
    pub fn new(data: &'a mut MessagePaneData, markdown_service: &'a MarkdownRenderer) -> Self {
        Self {
            data,
            style: MessagePaneStyle::default(),
            disable_user_colors: false,
            image_preview: true,
            timestamp_format: "%H:%M",
            current_user_id: None,
            markdown_service,
            relationship_state: None,
            hide_blocked_completely: false,
        }
    }

    #[must_use]
    pub const fn style(mut self, style: MessagePaneStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub const fn with_disable_user_colors(mut self, disable: bool) -> Self {
        self.disable_user_colors = disable;
        self
    }

    #[must_use]
    pub const fn with_image_preview(mut self, preview: bool) -> Self {
        self.image_preview = preview;
        self
    }

    #[must_use]
    pub const fn with_timestamp_format(mut self, format: &'a str) -> Self {
        self.timestamp_format = format;
        self
    }

    #[must_use]
    pub fn with_current_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.current_user_id = Some(user_id.into());
        self
    }

    /// Sets the relationship state for blocked user filtering.
    #[must_use]
    pub const fn with_relationship_state(mut self, state: &'a RelationshipState) -> Self {
        self.relationship_state = Some(state);
        self
    }

    /// Sets whether to completely hide blocked messages (true) or show placeholder (false).
    #[must_use]
    pub const fn with_hide_blocked_completely(mut self, hide: bool) -> Self {
        self.hide_blocked_completely = hide;
        self
    }

    #[must_use]
    pub fn calculate_message_height(
        &self,
        message: &Message,
        width: u16,
        markdown_service: &MarkdownRenderer,
        default_color: Color,
    ) -> u16 {
        let indent_width = u16::try_from(CONTENT_INDENT).unwrap_or(0);
        let content_width = width
            .saturating_sub(indent_width)
            .saturating_sub(SCROLLBAR_MARGIN);

        let authors = &self.data.authors;
        let channels = &self.data.channels;
        let resolver = HashMapResolver { authors, channels };
        let text = markdown_service.render_markdown(message.content(), Some(&resolver), false);

        let mut content_lines = 0;
        for line in &text.lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            content_lines +=
                u16::try_from(wrap_text(&line_text, content_width as usize).len()).unwrap_or(0);
        }

        let mut height = 1 + content_lines;

        if message.is_reply() {
            height += 1;
        }

        if message.has_attachments() {
            if self.image_preview {
                for attachment in message.attachments().iter().filter(|a| a.is_image()) {
                    if let (Some(w), Some(h)) = (attachment.width, attachment.height) {
                        let aspect = f64::from(h) / f64::from(w);
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let estimated = (f64::from(content_width) * 0.5 * aspect).ceil() as u16;
                        height += estimated.clamp(1, MAX_IMAGE_HEIGHT);
                    } else {
                        height += 3;
                    }
                }

                let non_image_count = message
                    .attachments()
                    .iter()
                    .filter(|a| !a.is_image())
                    .count();
                height += u16::try_from(non_image_count).unwrap_or(0);
            } else {
                height += u16::try_from(message.attachments().len()).unwrap_or(u16::MAX);
            }
        }

        for embed in message.embeds() {
            height += calculate_embed_layout(embed, content_width, markdown_service, default_color)
                .height;
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
            let clean_typing = clean_text(typing);
            block = block.title_bottom(
                Line::from(Span::styled(
                    format!(" {clean_typing} "),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ))
                .alignment(Alignment::Left),
            );
        }

        block
    }

    #[allow(clippy::too_many_lines)]
    fn render_messages(&mut self, area: Rect, buf: &mut Buffer, state: &mut MessagePaneState) {
        let block = self.build_block(state);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let MessagePane {
            data,
            style,
            disable_user_colors,
            image_preview,
            timestamp_format,
            current_user_id,
            markdown_service,
            relationship_state,
            hide_blocked_completely,
        } = self;

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

        data.update_layout(
            inner_area.width,
            markdown_service,
            style.content_style.fg.unwrap_or(Color::White),
            state.show_spoilers,
            *image_preview,
        );

        if let Some(error_msg) = &data.error_message
            && matches!(data.loading_state(), LoadingState::Loaded)
        {
            let error_para = Paragraph::new(format!("Error: {error_msg}"))
                .style(style.error_style)
                .alignment(Alignment::Center);

            let error_area = Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(1),
                inner_area.width,
                1,
            );
            Clear.render(error_area, buf);
            error_para.render(error_area, buf);
        }

        let render_items = build_render_items(
            data.ui_messages(),
            *relationship_state,
            *hide_blocked_completely,
        );

        let content_height: usize = render_items
            .iter()
            .map(|item| match item {
                RenderItem::Message { idx } => data.messages[*idx].estimated_height as usize,
                RenderItem::BlockedRun { .. } => 1,
            })
            .sum();
        state.update_dimensions(content_height, inner_area.height);
        state.last_width = inner_area.width;

        let max_scroll = content_height.saturating_sub(inner_area.height as usize);
        let mut offset = state.vertical_scroll.min(max_scroll);

        if let Some(selected_idx) = state.selected_index
            && state.flags.scroll_to_selection
        {
            let mut selection_y_start = 0;
            let mut selection_height = 0;

            for item in &render_items {
                let h = match item {
                    RenderItem::Message { idx } => data.messages[*idx].estimated_height as usize,
                    RenderItem::BlockedRun { .. } => 1,
                };
                let contains_selection = match item {
                    RenderItem::Message { idx } => *idx == selected_idx,
                    RenderItem::BlockedRun { start_idx, count } => {
                        selected_idx >= *start_idx && selected_idx < *start_idx + *count
                    }
                };
                if contains_selection {
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
            state.flags.scroll_to_selection = false;
        }

        let mut current_y: i32 = 0;

        let authors = &data.authors;
        for item in render_items {
            let h = match &item {
                RenderItem::Message { idx } => data.messages[*idx].estimated_height as usize,
                RenderItem::BlockedRun { .. } => 1,
            };
            let current_y_usize = usize::try_from(current_y).unwrap_or(0);

            if current_y_usize + h > offset && current_y_usize < offset + inner_area.height as usize
            {
                let render_y = current_y - i32::try_from(offset).unwrap_or(0);

                match item {
                    RenderItem::BlockedRun { count, start_idx } => {
                        let is_selected = state
                            .selected_index
                            .is_some_and(|sel| sel >= start_idx && sel < start_idx + count);
                        render_blocked_run(style, render_y, inner_area, buf, is_selected, count);
                    }
                    RenderItem::Message { idx } => {
                        let ui_msg = &mut data.messages[idx];
                        render_ui_message(
                            ui_msg,
                            style,
                            authors,
                            idx,
                            render_y,
                            inner_area,
                            buf,
                            state,
                            *disable_user_colors,
                            data.use_display_name,
                            *image_preview,
                            timestamp_format,
                            current_user_id.as_deref(),
                        );
                    }
                }
            }
            current_y += i32::try_from(h).unwrap_or(0);
        }

        let scroll_lengths = ScrollLengths {
            content_len: content_height,
            viewport_len: inner_area.height as usize,
        };

        let scrollbar = ScrollBar::vertical(scroll_lengths)
            .offset(state.vertical_scroll)
            .glyph_set(GlyphSet::unicode())
            .track_style(style.scrollbar_track_style)
            .thumb_style(style.scrollbar_thumb_style);

        let scrollbar_area = Rect {
            x: inner_area.x + inner_area.width.saturating_sub(1),
            y: inner_area.y,
            width: 1,
            height: inner_area.height,
        };
        scrollbar.render(scrollbar_area, buf);
    }

    fn render_forum(&self, area: Rect, buf: &mut Buffer, state: &mut MessagePaneState) {
        let focused = state.is_focused();
        let block = self.build_block(state);
        let inner_area = block.inner(area);
        block.render(area, buf);

        state.update_dimensions(0, inner_area.height);

        let ViewMode::Forum(forum_state) = &mut state.view_mode else {
            return;
        };

        if forum_state.threads.is_empty() {
            let empty = Paragraph::new("No threads found").style(self.style.empty_style);
            empty.render(inner_area, buf);
            return;
        }

        let visible_count = inner_area.height / THREAD_CARD_HEIGHT;
        let visible_count_usize = usize::from(visible_count);

        if forum_state.needs_scroll_to_selection && visible_count > 0 {
            let selected = u16::try_from(forum_state.selected_idx).unwrap_or(u16::MAX);
            forum_state.scroll_offset = (selected + 1).saturating_sub(visible_count);
            forum_state.needs_scroll_to_selection = false;
        }

        let mut start_idx = forum_state.scroll_offset as usize;

        if start_idx >= forum_state.threads.len() {
            start_idx = forum_state.threads.len().saturating_sub(1);
            forum_state.scroll_offset = u16::try_from(start_idx).unwrap_or(0);
        }

        let count_to_render = visible_count_usize.min(forum_state.threads.len() - start_idx);
        let end_idx = start_idx + count_to_render;

        let mut current_y = inner_area.y;

        for (i, thread) in forum_state
            .threads
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(count_to_render)
        {
            let is_last = i == end_idx - 1;

            let height = if is_last {
                inner_area.bottom().saturating_sub(current_y)
            } else {
                THREAD_CARD_HEIGHT
            };

            if height == 0 {
                break;
            }

            let card_area = Rect::new(inner_area.x, current_y, inner_area.width - 1, height);
            self.render_thread_card(
                card_area,
                buf,
                thread,
                i == forum_state.selected_idx,
                focused,
            );

            current_y += height;
        }

        let scroll_lengths = ScrollLengths {
            content_len: forum_state.threads.len(),
            viewport_len: visible_count_usize,
        };
        let scrollbar = ScrollBar::vertical(scroll_lengths)
            .offset(forum_state.scroll_offset as usize)
            .glyph_set(GlyphSet::unicode())
            .track_style(self.style.scrollbar_track_style)
            .thumb_style(self.style.scrollbar_thumb_style);

        let scrollbar_area = Rect {
            x: inner_area.x + inner_area.width.saturating_sub(1),
            y: inner_area.y,
            width: 1,
            height: inner_area.height,
        };
        scrollbar.render(scrollbar_area, buf);
    }

    fn render_thread_card(
        &self,
        area: Rect,
        buf: &mut Buffer,
        thread: &ForumThread,
        selected: bool,
        focused: bool,
    ) {
        let card_style = if selected {
            Style::default().bg(Color::Rgb(30, 30, 30))
        } else {
            Style::default()
        };

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_style(card_style);
                }
            }
        }

        if selected && focused {
            for y in area.top()..area.bottom() {
                if let Some(cell) = buf.cell_mut((area.left(), y)) {
                    cell.set_symbol("│");
                    cell.set_style(self.style.border_style_focused);
                }
            }
        }

        let content_area = Rect::new(
            area.x + 2,
            area.y + 1,
            area.width.saturating_sub(4),
            area.height.saturating_sub(2),
        );

        let title_style = self.style.title_style;
        let mut line1_spans = vec![Span::styled(&thread.name, title_style), Span::raw(" ")];
        if thread.new {
            line1_spans.push(Span::styled(
                " NEW ",
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        let line1 = Line::from(line1_spans);
        buf.set_line(content_area.x, content_area.y, &line1, content_area.width);

        let author_name = thread
            .starter_message
            .as_ref()
            .map(|m| {
                IdentityResolver::with_preference(self.data.use_display_name).resolve(m.author())
            })
            .or_else(|| {
                self.data
                    .get_author_name(&thread.author_id)
                    .map(String::from)
            })
            .unwrap_or_else(|| thread.author_id.clone());

        let time_str = thread.last_activity_at.as_deref().map_or_else(
            || "?".to_string(),
            crate::presentation::ui::utils::format_iso_timestamp,
        );

        let mut meta_spans = vec![
            Span::styled(format!("@{author_name}"), self.style.author_style),
            Span::styled(" • ", Style::default().fg(Color::Gray)),
            Span::styled(time_str, Style::default().fg(Color::Gray)),
            Span::styled(" | ", Style::default().fg(Color::Gray)),
        ];

        for tag_id in &thread.applied_tags {
            meta_spans.push(Span::styled(
                format!("[{tag_id}] "),
                Style::default().fg(Color::Blue),
            ));
        }

        let line2 = Line::from(meta_spans);
        buf.set_line(
            content_area.x,
            content_area.y + 1,
            &line2,
            content_area.width,
        );

        if let Some(starter) = &thread.starter_message {
            let content = starter.content();
            let wrapped = wrap_text(content, content_area.width as usize);
            for i in 0..2 {
                if let Some(line) = wrapped.get(i) {
                    buf.set_string(
                        content_area.x,
                        content_area.y + 2 + u16::try_from(i).unwrap_or(0),
                        line,
                        self.style.content_style,
                    );
                }
            }
        }

        let upvotes = thread.reaction_count;

        let replies = thread.message_count;

        let footer_text = format!("▲ {upvotes}  💬 {replies}");
        let footer_span = Span::styled(footer_text, Style::default().fg(Color::Green));
        let footer_line = Line::from(footer_span).alignment(Alignment::Right);

        let footer_para = Paragraph::new(footer_line);
        let footer_area = Rect::new(content_area.x, area.bottom() - 2, content_area.width, 1);
        footer_para.render(footer_area, buf);
    }
}

impl StatefulWidget for MessagePane<'_> {
    type State = MessagePaneState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match &mut state.view_mode {
            ViewMode::Messages => self.render_messages(area, buf, state),
            ViewMode::Forum(_) => {
                self.render_forum(area, buf, state);
            }
        }
    }
}

fn draw_embed_border(
    buf: &mut Buffer,
    x: u16,
    y_offset: i32,
    area_y: u16,
    area_height: u16,
    color: Color,
) {
    if y_offset >= 0 && y_offset < i32::from(area_height) {
        let y = u16::try_from(y_offset).unwrap_or(0);
        if let Some(cell) = buf.cell_mut((x, area_y + y)) {
            cell.set_symbol("▎").set_fg(color);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn render_embed(embed: &RenderedEmbed, start_y: i32, area: Rect, buf: &mut Buffer) -> i32 {
    let mut current_y = start_y;
    let indent = u16::try_from(EMBED_INDENT).unwrap_or(0);
    let border_color = embed.color;
    let content_x = area.x.saturating_add(indent);
    let content_width = area
        .width
        .saturating_sub(indent)
        .saturating_sub(SCROLLBAR_MARGIN)
        .saturating_sub(2);

    if embed.height > 0 {
        draw_embed_border(buf, content_x, current_y, area.y, area.height, border_color);
        current_y += 1;
    }

    if let Some(name) = &embed.provider {
        let span = Span::styled(name, Style::default().fg(Color::DarkGray));
        draw_embed_border(buf, content_x, current_y, area.y, area.height, border_color);
        if current_y >= 0 && current_y < i32::from(area.height) {
            let y = u16::try_from(current_y).unwrap_or(0);
            let para = Paragraph::new(Line::from(span)).style(Style::default());
            let text_area = Rect::new(content_x + 2, area.y + y, content_width, 1);
            para.render(text_area, buf);
        }
        current_y += 1;
    }

    if !embed.title.is_empty() {
        let mut style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        if embed.url.is_some() {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        for line in &embed.title {
            let span = Span::styled(line, style);
            draw_embed_border(buf, content_x, current_y, area.y, area.height, border_color);
            if current_y >= 0 && current_y < i32::from(area.height) {
                let y = u16::try_from(current_y).unwrap_or(0);
                let para = Paragraph::new(Line::from(span))
                    .style(Style::default().add_modifier(Modifier::BOLD));
                let text_area = Rect::new(content_x + 2, area.y + y, content_width, 1);
                para.render(text_area, buf);
            }
            current_y += 1;
        }
    }

    if let Some(text) = &embed.description {
        let desc_height = i32::from(embed.description_height);

        if current_y + desc_height > 0 && current_y < i32::from(area.height) {
            for i in 0..desc_height {
                draw_embed_border(
                    buf,
                    content_x,
                    current_y + i,
                    area.y,
                    area.height,
                    border_color,
                );
            }

            let top_clip = if current_y < 0 {
                u16::try_from(current_y.unsigned_abs()).unwrap_or(0)
            } else {
                0
            };

            let target_y = u16::try_from(current_y.max(0)).unwrap_or(0);
            let available_height = area.height.saturating_sub(target_y);
            let effective_height = u16::try_from(desc_height)
                .unwrap_or(0)
                .saturating_sub(top_clip)
                .min(available_height);

            if effective_height > 0 {
                let para = Paragraph::new(text.clone())
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .style(Style::default().fg(Color::Gray))
                    .scroll((top_clip, 0));

                let text_area = Rect::new(
                    content_x + 2,
                    area.y + target_y,
                    content_width.saturating_sub(SCROLLBAR_MARGIN),
                    effective_height,
                );
                para.render(text_area, buf);
            }
        }
        current_y += desc_height;
    }

    if embed.height > 0 {
        draw_embed_border(buf, content_x, current_y, area.y, area.height, border_color);
        current_y += 1;
    }

    current_y - start_y
}

fn build_render_items(
    messages: &VecDeque<UiMessage>,
    relationship_state: Option<&RelationshipState>,
    hide_blocked_completely: bool,
) -> Vec<RenderItem> {
    let mut items = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let author_id = messages[i].message.author().id().to_string();
        let is_blocked = relationship_state.is_some_and(|s| s.is_blocked_str(&author_id));

        if is_blocked && hide_blocked_completely {
            i += 1;
            continue;
        }

        if is_blocked {
            let start_idx = i;
            let mut count = 0;
            while i < messages.len() {
                let aid = messages[i].message.author().id().to_string();
                let blocked = relationship_state.is_some_and(|s| s.is_blocked_str(&aid));
                if !blocked {
                    break;
                }
                count += 1;
                i += 1;
            }
            items.push(RenderItem::BlockedRun { count, start_idx });
        } else {
            items.push(RenderItem::Message { idx: i });
            i += 1;
        }
    }

    items
}

fn render_blocked_run(
    style: &MessagePaneStyle,
    render_y: i32,
    area: Rect,
    buf: &mut Buffer,
    is_selected: bool,
    count: usize,
) {
    if render_y < 0 || render_y >= i32::from(area.height) {
        return;
    }

    let blocked_style = if is_selected {
        style.selected_style.add_modifier(Modifier::ITALIC)
    } else {
        style.blocked_style
    };

    let text = if count == 1 {
        "─ 1 blocked ─".to_string()
    } else {
        format!("─ {count} blocked ─")
    };

    let line = Line::from(Span::styled(text, blocked_style));
    let para = Paragraph::new(line).alignment(Alignment::Center);

    let y = area.y.saturating_add(u16::try_from(render_y).unwrap_or(0));
    let placeholder_area = Rect::new(area.x, y, area.width, 1);
    para.render(placeholder_area, buf);
}

#[allow(
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::too_many_arguments
)]
fn render_ui_message(
    ui_msg: &mut UiMessage,
    style: &MessagePaneStyle,
    _authors: &HashMap<String, String>,
    index: usize,
    render_y: i32,
    area: Rect,
    buf: &mut Buffer,
    state: &mut MessagePaneState,
    disable_user_colors: bool,
    use_display_name: bool,
    image_preview: bool,
    timestamp_format: &str,
    current_user_id: Option<&str>,
) {
    let message = &ui_msg.message;
    let is_selected = state.selected_index == Some(index);
    let is_mentioned = if let Some(id) = current_user_id {
        message.mentions().iter().any(|u| u.id_str() == id)
    } else {
        false
    };
    let mut current_msg_y = render_y;

    let base_style = if is_selected {
        style.selected_style
    } else if is_mentioned {
        style.mention_style
    } else {
        Style::default()
    };

    if message.is_reply() {
        if current_msg_y >= 0
            && current_msg_y < i32::from(area.height)
            && let Some(preview) = &ui_msg.reply_preview
        {
            let render_line = if is_selected || is_mentioned {
                let mut spans = preview.clone().spans;
                if spans.len() == 4 {
                    spans[1].style = Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::ITALIC);
                    spans[2].style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::ITALIC);
                    spans[3].style = Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::ITALIC);
                } else {
                    for span in &mut spans {
                        span.style = span.style.add_modifier(Modifier::ITALIC);
                        span.style = span.style.fg(Color::White);
                    }
                }
                Line::from(spans)
            } else {
                let mut spans = preview.clone().spans;
                for span in &mut spans {
                    span.style = span.style.add_modifier(Modifier::ITALIC);
                }
                Line::from(spans)
            };

            let reply_para = Paragraph::new(render_line).style(base_style);
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

    if ui_msg.group == MessageGroup::Start {
        if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
            let (timestamp_style, edited_style) = if is_selected || is_mentioned {
                (
                    Style::default().fg(Color::White),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::ITALIC),
                )
            } else {
                (style.timestamp_style, style.edited_style)
            };

            let author_color = if disable_user_colors {
                style.author_style.fg.unwrap_or(Color::Yellow)
            } else {
                get_author_color(message.author())
            };

            let mut header_spans = vec![
                Span::styled(
                    format!(
                        "{:<width$}",
                        message.timestamp().format(timestamp_format).to_string(),
                        width = TIMESTAMP_WIDTH
                    ),
                    timestamp_style,
                ),
                Span::styled(
                    IdentityResolver::with_preference(use_display_name).resolve(message.author()),
                    style.author_style.fg(author_color),
                ),
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
    }

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
    let max_image_width = area.width.saturating_sub(indent_width + SCROLLBAR_MARGIN);

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

    let content_height = if let Some(t) = &ui_msg.rendered_content {
        i32::try_from(t.lines.len()).unwrap_or(0)
    } else {
        i32::from(!message.content().is_empty())
    };

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

    for attachment in message.attachments() {
        if attachment.is_image() {
            continue;
        }
        if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
            let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
            let attachment_text = format!("\u{1F4CE} {}", attachment.filename);
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

    for img_attachment in &mut ui_msg.image_attachments {
        if !image_preview {
            if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
                let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
                let attachment_text = format!("\u{1F5BC} {}", img_attachment.url);
                let attachment_line = Line::from(vec![
                    indent_span,
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
            continue;
        }

        if !img_attachment.is_ready() {
            if img_attachment.is_loading()
                && current_msg_y >= 0
                && current_msg_y < i32::from(area.height)
            {
                let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
                let loading_text = "\u{1F5BC}  Loading image...";
                let loading_line = Line::from(vec![
                    indent_span,
                    Span::styled(
                        loading_text,
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                let loading_para = Paragraph::new(loading_line).style(base_style);
                let loading_area = Rect::new(
                    area.x,
                    area.y
                        .saturating_add(u16::try_from(current_msg_y).unwrap_or(0)),
                    area.width,
                    1,
                );
                loading_para.render(loading_area, buf);
                current_msg_y += 1;
            }
            continue;
        }

        let actual_height = img_attachment.height(max_image_width);
        let actual_width = img_attachment.width(max_image_width);

        let has_protocol = img_attachment.protocol.is_some();

        if has_protocol {
            let img_start_y = current_msg_y;
            let img_height = i32::from(actual_height);

            if img_start_y + img_height > 0 && img_start_y < i32::from(area.height) {
                let top_clip = if img_start_y < 0 {
                    u16::try_from(img_start_y.unsigned_abs()).unwrap_or(0)
                } else {
                    0
                };

                let target_y = u16::try_from(img_start_y.max(0)).unwrap_or(0);
                let available_height = area.height.saturating_sub(target_y);
                let effective_height = actual_height.saturating_sub(top_clip).min(available_height);

                if effective_height > 0 {
                    let effective_width = if actual_width > 0 {
                        actual_width.min(max_image_width)
                    } else {
                        max_image_width
                    };

                    let img_area = Rect::new(
                        area.x + u16::try_from(CONTENT_INDENT).unwrap_or(0),
                        area.y + target_y,
                        effective_width,
                        effective_height,
                    );

                    let clear_area =
                        Rect::new(area.x, area.y + target_y, area.width, effective_height);
                    Clear.render(clear_area, buf);

                    if let Some(ref mut protocol) = img_attachment.protocol {
                        use ratatui_image::{Resize, StatefulImage};
                        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
                        ratatui::widgets::StatefulWidget::render(
                            image_widget,
                            img_area,
                            buf,
                            protocol,
                        );
                    }
                }
            }

            current_msg_y += img_height;
        } else {
            if current_msg_y >= 0 && current_msg_y < i32::from(area.height) {
                let indent_span = Span::raw(" ".repeat(CONTENT_INDENT));
                let placeholder_text = "\u{1F5BC}  [Image]";
                let placeholder_line = Line::from(vec![
                    indent_span,
                    Span::styled(placeholder_text, style.attachment_style),
                ]);
                let placeholder_para = Paragraph::new(placeholder_line).style(base_style);
                let placeholder_area = Rect::new(
                    area.x,
                    area.y
                        .saturating_add(u16::try_from(current_msg_y).unwrap_or(0)),
                    area.width,
                    1,
                );
                placeholder_para.render(placeholder_area, buf);
            }
            current_msg_y += 1;
        }
    }

    for embed in &ui_msg.rendered_embeds {
        let height = render_embed(embed, current_msg_y, area, buf);
        current_msg_y += height;
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if UnicodeWidthStr::width(s) <= max_len {
        return s.to_string();
    }

    if max_len < 3 {
        let mut current_width = 0;
        let mut cut_off_index = 0;
        for (i, c) in s.char_indices() {
            let char_len = c.len_utf8();
            let w = UnicodeWidthStr::width(&s[i..i + char_len]);
            if current_width + w > max_len {
                break;
            }
            current_width += w;
            cut_off_index = i + char_len;
        }
        return s[..cut_off_index].to_string();
    }

    let mut current_width = 0;
    let mut cut_off_index = 0;
    let target_width = max_len.saturating_sub(3);

    for (i, c) in s.char_indices() {
        let char_len = c.len_utf8();
        let w = UnicodeWidthStr::width(&s[i..i + char_len]);
        if current_width + w > target_width {
            break;
        }
        current_width += w;
        cut_off_index = i + char_len;
    }

    format!("{}...", &s[..cut_off_index])
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
            let prefix_width = usize::from(i > 0);

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

fn wrap_styled_text(text: Text<'static>, width: u16) -> Text<'static> {
    if width == 0 {
        return text;
    }

    let mut new_lines = Vec::new();

    for line in text.lines {
        if line.width() == 0 {
            new_lines.push(Line::raw(""));
            continue;
        }

        let mut current_line_spans = Vec::new();

        let mut current_width = 0;

        for span in line.spans {
            let content = span.content.clone();
            let style = span.style;

            let words: Vec<&str> = content.split_inclusive(' ').collect();

            for word in words {
                let word_width = UnicodeWidthStr::width(word);

                if word_width > width as usize {
                    let mut remaining_word = word;
                    while !remaining_word.is_empty() {
                        if current_width + UnicodeWidthStr::width(remaining_word) <= width as usize
                        {
                            current_line_spans
                                .push(Span::styled(remaining_word.to_string(), style));
                            current_width += UnicodeWidthStr::width(remaining_word);
                            remaining_word = "";
                        } else {
                            let available = (width as usize).saturating_sub(current_width);
                            if available == 0 {
                                new_lines.push(Line::from(current_line_spans));
                                current_line_spans = Vec::new();
                                current_width = 0;
                                continue;
                            }

                            let mut split_idx = 0;
                            let mut split_w = 0;
                            for (idx, c) in remaining_word.char_indices() {
                                let mut buf = [0u8; 4];
                                let cw = UnicodeWidthStr::width(c.encode_utf8(&mut buf));
                                if split_w + cw > available {
                                    break;
                                }
                                split_w += cw;
                                split_idx = idx + c.len_utf8();
                            }

                            if split_idx > 0 {
                                current_line_spans.push(Span::styled(
                                    remaining_word[..split_idx].to_string(),
                                    style,
                                ));
                                remaining_word = &remaining_word[split_idx..];
                            }

                            new_lines.push(Line::from(current_line_spans));
                            current_line_spans = Vec::new();
                            current_width = 0;
                        }
                    }
                    continue;
                }

                if current_width + word_width > width as usize {
                    new_lines.push(Line::from(current_line_spans));
                    current_line_spans = Vec::new();
                    current_width = 0;
                }

                current_line_spans.push(Span::styled(word.to_string(), style));
                current_width += word_width;
            }
        }

        if !current_line_spans.is_empty() {
            new_lines.push(Line::from(current_line_spans));
        }
    }

    if new_lines.is_empty() {
        new_lines.push(Line::raw(""));
    }

    Text::from(new_lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::MessageAuthor;
    use chrono::Local;
    use test_case::test_case;

    fn create_test_message(id: u64, content: &str) -> Message {
        let author = MessageAuthor {
            id: "1".to_string(),
            username: "testuser".to_string(),
            discriminator: "0".to_string(),
            avatar: None,
            bot: false,
            global_name: None,
        };
        Message::new(
            id.into(),
            ChannelId(100),
            author,
            content.to_string(),
            Local::now(),
            crate::domain::entities::MessageKind::Default,
        )
    }

    #[test]
    fn test_message_pane_data_set_messages() {
        let mut data = MessagePaneData::new(true);
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
    fn test_formatted_channel_title() {
        let mut data = MessagePaneData::new(true);
        data.set_channel(ChannelId(100), "general".to_string());
        assert_eq!(
            data.formatted_channel_title(),
            Some("[ GENERAL ]".to_string())
        );

        let mut dm_data = MessagePaneData::new(true);
        dm_data.set_channel(ChannelId(200), "@username".to_string());
        assert_eq!(
            dm_data.formatted_channel_title(),
            Some("[ USERNAME ]".to_string())
        );
    }

    #[test]
    fn test_scrollbar_position_at_bottom() {
        use crate::presentation::services::markdown_renderer::MarkdownRenderer;

        let mut data = MessagePaneData::new(true);
        data.set_channel(ChannelId(100), "general".to_string());

        let messages: Vec<Message> = (0..50).map(|i| create_test_message(i, "msg")).collect();
        data.set_messages(messages);

        let markdown = MarkdownRenderer::new();
        data.update_layout(100, &markdown, Color::Yellow, false, true);

        let mut state = MessagePaneState::new();
        state.flags.is_following = true;

        let content_height: usize = data
            .ui_messages()
            .iter()
            .map(|m| m.estimated_height as usize)
            .sum();
        assert_eq!(content_height, 51);

        state.update_dimensions(content_height, 50);

        assert_eq!(state.vertical_scroll, 1);
    }

    #[test]
    fn test_message_grouping_logic() {
        use chrono::Duration;

        let now = Local::now();
        let author1 = MessageAuthor {
            id: "1".to_string(),
            username: "user1".to_string(),
            discriminator: "0".to_string(),
            avatar: None,
            bot: false,
            global_name: None,
        };
        let author2 = MessageAuthor {
            id: "2".to_string(),
            username: "user2".to_string(),
            discriminator: "0".to_string(),
            avatar: None,
            bot: false,
            global_name: None,
        };

        let m1 = Message::new(
            1u64.into(),
            ChannelId(100),
            author1.clone(),
            "Base message".to_string(),
            now,
            crate::domain::entities::MessageKind::Default,
        );

        let m2 = Message::new(
            2u64.into(),
            ChannelId(100),
            author1.clone(),
            "Short delay".to_string(),
            now + Duration::minutes(1),
            crate::domain::entities::MessageKind::Default,
        );

        let m3 = Message::new(
            3u64.into(),
            ChannelId(100),
            author1.clone(),
            "Long delay".to_string(),
            now + Duration::minutes(9),
            crate::domain::entities::MessageKind::Default,
        );

        let m4 = Message::new(
            4u64.into(),
            ChannelId(100),
            author2.clone(),
            "Different user".to_string(),
            now + Duration::minutes(10),
            crate::domain::entities::MessageKind::Default,
        );

        let m5 = Message::new(
            5u64.into(),
            ChannelId(100),
            author2.clone(),
            "Reply".to_string(),
            now + Duration::minutes(11),
            crate::domain::entities::MessageKind::Reply,
        );

        let mut data = MessagePaneData::new(true);
        data.set_messages(vec![m1, m2, m3, m4, m5]);

        let messages: Vec<_> = data.messages.iter().collect();

        assert_eq!(messages[0].group, MessageGroup::Start);
        assert_eq!(messages[1].group, MessageGroup::Compact);
        assert_eq!(messages[2].group, MessageGroup::Start);
        assert_eq!(messages[3].group, MessageGroup::Start);
        assert_eq!(messages[4].group, MessageGroup::Start);
    }

    #[test]
    fn test_invalid_timestamp_format() {
        use crate::presentation::services::markdown_renderer::MarkdownRenderer;
        use ratatui::widgets::StatefulWidget;

        let mut data = MessagePaneData::new(true);
        data.set_channel(ChannelId(100), "general".to_string());

        let message = create_test_message(1, "Hello");
        data.set_messages(vec![message]);

        let markdown = MarkdownRenderer::new();
        let format = "%Z %Invalid";

        let pane = MessagePane::new(&mut data, &markdown).with_timestamp_format(format);

        let mut state = MessagePaneState::new();
        let area = Rect::new(0, 0, 100, 20);
        let mut buf = Buffer::empty(area);

        pane.render(area, &mut buf, &mut state);
    }

    #[test]
    fn test_embed_height_calculation_padding() {
        use crate::domain::entities::Embed;
        use crate::presentation::services::markdown_renderer::MarkdownRenderer;

        let mut data = MessagePaneData::new(true);
        let markdown = MarkdownRenderer::new();
        let pane = MessagePane::new(&mut data, &markdown);

        let mut embed = Embed::new();
        embed.title = Some("Test Title".to_string());
        embed.description = Some("Test Description".to_string());

        let message = create_test_message(1, "Content").with_embeds(vec![embed]);

        let height = pane.calculate_message_height(&message, 100, &markdown, Color::Yellow);
        assert_eq!(height, 6, "Height should be 6 with padding changes");
    }

    #[test_case(true, 2 ; "hide blocked completely")]
    #[test_case(false, 2 ; "show placeholder")]
    fn test_blocked_user_rendering(hide_completely: bool, expected_count: usize) {
        use crate::domain::entities::{RelationshipState, UserId};
        use crate::presentation::services::markdown_renderer::MarkdownRenderer;
        use ratatui::widgets::StatefulWidget;

        let mut data = MessagePaneData::new(true);
        data.set_channel(ChannelId(100), "general".to_string());

        let blocked_author = MessageAuthor {
            id: "999".to_string(),
            username: "blockeduser".to_string(),
            discriminator: "0".to_string(),
            avatar: None,
            bot: false,
            global_name: None,
        };
        let normal_author = MessageAuthor {
            id: "123".to_string(),
            username: "normaluser".to_string(),
            discriminator: "0".to_string(),
            avatar: None,
            bot: false,
            global_name: None,
        };

        let blocked_message = Message::new(
            1u64.into(),
            ChannelId(100),
            blocked_author,
            "Hidden message".to_string(),
            Local::now(),
            crate::domain::entities::MessageKind::Default,
        );
        let normal_message = Message::new(
            2u64.into(),
            ChannelId(100),
            normal_author,
            "Visible message".to_string(),
            Local::now(),
            crate::domain::entities::MessageKind::Default,
        );

        data.set_messages(vec![blocked_message, normal_message]);

        let relationship_state = RelationshipState::new();
        relationship_state.block_user(UserId(999));

        let markdown = MarkdownRenderer::new();
        let pane = MessagePane::new(&mut data, &markdown)
            .with_relationship_state(&relationship_state)
            .with_hide_blocked_completely(hide_completely);

        let mut state = MessagePaneState::new();
        let area = Rect::new(0, 0, 100, 20);
        let mut buf = Buffer::empty(area);

        pane.render(area, &mut buf, &mut state);

        assert_eq!(data.message_count(), expected_count);
    }
}
