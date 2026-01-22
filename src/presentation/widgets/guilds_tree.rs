//! Guilds tree widget for server/channel navigation.

use std::collections::HashSet;

use crossterm::event::KeyEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, StatefulWidget, Widget},
};

use crate::domain::entities::{Channel, ChannelId, ChannelKind, Guild, GuildId, ReadState};
use crate::domain::keybinding::Action;
use crate::presentation::commands::CommandRegistry;
use crate::presentation::theme::Theme;
use crate::presentation::ui::utils::clean_text;

/// Unique identifier for nodes in the guilds tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    DirectMessages,

    DirectMessageUser(String),

    Guild(GuildId),

    Category(ChannelId),

    Channel(ChannelId),

    Placeholder(GuildId),
}

impl std::fmt::Display for TreeNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectMessages => write!(f, "dm"),
            Self::DirectMessageUser(id) => write!(f, "dm:{id}"),
            Self::Guild(id) => write!(f, "guild:{id}"),
            Self::Category(id) => write!(f, "cat:{id}"),
            Self::Channel(id) => write!(f, "ch:{id}"),
            Self::Placeholder(id) => write!(f, "placeholder:{id}"),
        }
    }
}

/// Actions that can be triggered by the guilds tree.
#[derive(Debug, Clone)]
pub enum GuildsTreeAction {
    SelectChannel(ChannelId),

    SelectGuild(GuildId),

    SelectDirectMessage(String),

    YankId(String),

    LoadGuildChannels(GuildId),
}

/// State for the guilds tree widget.
pub struct GuildsTreeState {
    expanded: HashSet<TreeNodeId>,
    selected: Option<TreeNodeId>,
    focused: bool,

    list_state: ratatui::widgets::ListState,
}

impl GuildsTreeState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            selected: None,
            focused: false,
            list_state: ratatui::widgets::ListState::default(),
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    #[must_use]
    pub fn selected(&self) -> Option<&TreeNodeId> {
        self.selected.as_ref()
    }

    pub fn select(&mut self, node_id: TreeNodeId) {
        self.selected = Some(node_id);
    }

    pub fn toggle_current(&mut self) {
        if let Some(selected) = &self.selected {
            if self.expanded.contains(selected) {
                self.expanded.remove(selected);
            } else {
                self.expanded.insert(selected.clone());
            }
        }
    }

    pub fn expand(&mut self, node_id: TreeNodeId) {
        self.expanded.insert(node_id);
    }

    pub fn collapse(&mut self, node_id: &TreeNodeId) {
        self.expanded.remove(node_id);
    }

    #[must_use]
    pub fn is_expanded(&self, node_id: &TreeNodeId) -> bool {
        self.expanded.contains(node_id)
    }

    #[allow(clippy::too_many_lines)]
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        data: &GuildsTreeData,
        registry: &CommandRegistry,
        style: &GuildsTreeStyle,
    ) -> Option<GuildsTreeAction> {
        let flattened = data.flatten(self, u16::MAX, style);

        let current_index = self
            .selected
            .as_ref()
            .and_then(|sel| flattened.iter().position(|node| &node.id == sel));

        match registry.find_action(key) {
            Some(Action::NavigateDown) => {
                if !flattened.is_empty() {
                    let next_index = current_index
                        .map_or(0, |i| if i + 1 >= flattened.len() { 0 } else { i + 1 });
                    self.selected = Some(flattened[next_index].id.clone());
                    self.list_state.select(Some(next_index));
                }
                None
            }
            Some(Action::NavigateUp) => {
                if !flattened.is_empty() {
                    let prev_index = current_index.map_or(0, |i| {
                        if i == 0 {
                            flattened.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    });
                    self.selected = Some(flattened[prev_index].id.clone());
                    self.list_state.select(Some(prev_index));
                }
                None
            }
            Some(Action::NavigateLeft) => {
                if let Some(selected) = &self.selected {
                    if self.expanded.contains(selected) {
                        self.expanded.remove(selected);
                    } else if let Some(idx) = current_index {
                        let current_depth = flattened[idx].depth;
                        for i in (0..idx).rev() {
                            if flattened[i].depth < current_depth {
                                self.selected = Some(flattened[i].id.clone());
                                self.list_state.select(Some(i));
                                break;
                            }
                        }
                    }
                }
                None
            }
            Some(Action::NavigateRight) => {
                if let Some(selected) = &self.selected {
                    let can_expand = matches!(
                        selected,
                        TreeNodeId::Guild(_) | TreeNodeId::Category(_) | TreeNodeId::DirectMessages
                    );

                    if can_expand && !self.expanded.contains(selected) {
                        self.expanded.insert(selected.clone());
                        if let TreeNodeId::Guild(id) = selected
                            && data.channels(*id).is_none()
                        {
                            return Some(GuildsTreeAction::LoadGuildChannels(*id));
                        }
                    } else {
                        return self.get_selection_action();
                    }
                }
                None
            }
            Some(Action::SelectFirst) => {
                if !flattened.is_empty() {
                    self.selected = Some(flattened[0].id.clone());
                    self.list_state.select(Some(0));
                }
                None
            }
            Some(Action::SelectLast) => {
                if !flattened.is_empty() {
                    let idx = flattened.len() - 1;
                    self.selected = Some(flattened[idx].id.clone());
                    self.list_state.select(Some(idx));
                }
                None
            }
            Some(Action::Select) => {
                if let Some(selected) = self.selected.clone() {
                    match &selected {
                        TreeNodeId::Guild(id) => {
                            self.toggle_current();
                            if self.expanded.contains(&selected) && data.channels(*id).is_none() {
                                return Some(GuildsTreeAction::LoadGuildChannels(*id));
                            }
                            None
                        }
                        TreeNodeId::Category(_) | TreeNodeId::DirectMessages => {
                            self.toggle_current();
                            None
                        }
                        _ => self.get_selection_action(),
                    }
                } else {
                    None
                }
            }
            Some(Action::YankId) => self.selected.as_ref().map(|node| {
                let id = match node {
                    TreeNodeId::DirectMessages => "direct_messages".to_string(),
                    TreeNodeId::DirectMessageUser(id) => id.clone(),
                    TreeNodeId::Guild(id) | TreeNodeId::Placeholder(id) => id.to_string(),
                    TreeNodeId::Category(id) | TreeNodeId::Channel(id) => id.to_string(),
                };
                GuildsTreeAction::YankId(id)
            }),
            _ => None,
        }
    }

    fn get_selection_action(&self) -> Option<GuildsTreeAction> {
        self.selected.as_ref().and_then(|node| match node {
            TreeNodeId::Channel(id) => Some(GuildsTreeAction::SelectChannel(*id)),
            TreeNodeId::Guild(id) => Some(GuildsTreeAction::SelectGuild(*id)),
            TreeNodeId::DirectMessageUser(id) => {
                Some(GuildsTreeAction::SelectDirectMessage(id.clone()))
            }
            _ => None,
        })
    }
}

impl Default for GuildsTreeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Style configuration for the guilds tree.
#[allow(missing_docs)]
pub struct GuildsTreeStyle {
    pub border_style: Style,
    pub border_style_focused: Style,
    pub title_style: Style,
    pub selected_style: Style,
    pub active_guild_style: Style,
    pub active_channel_style: Style,
    pub guild_style: Style,
    pub guild_unread_style: Style,
    pub channel_style: Style,
    pub channel_unread_style: Style,
    pub category_style: Style,
    pub dm_style: Style,
    pub placeholder_style: Style,
    pub tree_guide_style: Style,
}

impl GuildsTreeStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(theme.accent),
            title_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            selected_style: Style::default().bg(Color::DarkGray).fg(theme.accent),
            active_guild_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            active_channel_style: Style::default()
                .fg(theme.accent)
                .bg(Color::Rgb(30, 40, 50))
                .add_modifier(Modifier::BOLD),
            dm_style: Style::default().fg(theme.accent),
            tree_guide_style: Style::default().fg(Color::Gray),
            ..Self::default()
        }
    }
}

impl Default for GuildsTreeStyle {
    fn default() -> Self {
        Self {
            border_style: Style::default().fg(Color::Gray),
            border_style_focused: Style::default().fg(Color::Cyan),
            title_style: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            selected_style: Style::default().bg(Color::DarkGray).fg(Color::White),
            active_guild_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            active_channel_style: Style::default()
                .fg(Color::Cyan)
                .bg(Color::Rgb(30, 40, 50))
                .add_modifier(Modifier::BOLD),
            guild_style: Style::default().fg(Color::White),
            guild_unread_style: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            channel_style: Style::default().fg(Color::Gray),
            channel_unread_style: Style::default().fg(Color::White),
            category_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            dm_style: Style::default().fg(Color::Magenta),
            placeholder_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            tree_guide_style: Style::default().fg(Color::Gray),
        }
    }
}

/// A flattened representation of a tree node for display.
#[derive(Debug, Clone)]
pub struct FlattenedNode<'a> {
    pub id: TreeNodeId,
    pub label: Line<'a>,
    pub depth: usize,
}

/// Data container for the guilds tree.
pub struct GuildsTreeData {
    guilds: Vec<Guild>,
    channels_by_guild: std::collections::HashMap<GuildId, Vec<Channel>>,
    dm_users: Vec<(String, String)>,
    active_guild_id: Option<GuildId>,
    active_channel_id: Option<ChannelId>,
    active_dm_user_id: Option<String>,
}

impl GuildsTreeData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            guilds: Vec::new(),
            channels_by_guild: std::collections::HashMap::new(),
            dm_users: Vec::new(),
            active_guild_id: None,
            active_channel_id: None,
            active_dm_user_id: None,
        }
    }

    pub fn set_guilds(&mut self, guilds: Vec<Guild>) {
        self.guilds = guilds;
    }

    pub fn set_channels(&mut self, guild_id: GuildId, channels: Vec<Channel>) {
        tracing::debug!(
            guild_id = %guild_id,
            guild_id_raw = guild_id.as_u64(),
            channel_count = channels.len(),
            "Storing channels for guild"
        );
        self.channels_by_guild.insert(guild_id, channels);
    }

    pub fn set_dm_users(&mut self, users: Vec<(String, String)>) {
        self.dm_users = users;
    }

    #[must_use]
    pub fn guilds(&self) -> &[Guild] {
        &self.guilds
    }

    #[must_use]
    pub fn channels(&self, guild_id: GuildId) -> Option<&Vec<Channel>> {
        self.channels_by_guild.get(&guild_id)
    }

    #[must_use]
    pub fn dm_users(&self) -> &[(String, String)] {
        &self.dm_users
    }

    pub const fn set_active_guild(&mut self, guild_id: Option<GuildId>) {
        self.active_guild_id = guild_id;
    }

    pub const fn set_active_channel(&mut self, channel_id: Option<ChannelId>) {
        self.active_channel_id = channel_id;
    }

    pub fn set_active_dm_user(&mut self, user_id: Option<String>) {
        self.active_dm_user_id = user_id;
    }

    #[must_use]
    pub const fn active_guild_id(&self) -> Option<GuildId> {
        self.active_guild_id
    }

    #[must_use]
    pub const fn active_channel_id(&self) -> Option<ChannelId> {
        self.active_channel_id
    }

    #[must_use]
    pub fn active_dm_user_id(&self) -> Option<&str> {
        self.active_dm_user_id.as_deref()
    }

    #[must_use]
    pub fn find_guild_for_channel(&self, channel_id: ChannelId) -> Option<GuildId> {
        for (guild_id, channels) in &self.channels_by_guild {
            if channels.iter().any(|c| c.id() == channel_id) {
                return Some(*guild_id);
            }
        }
        None
    }

    pub fn get_channel_mut(&mut self, channel_id: ChannelId) -> Option<&mut Channel> {
        for channels in self.channels_by_guild.values_mut() {
            if let Some(channel) = channels.iter_mut().find(|c| c.id() == channel_id) {
                return Some(channel);
            }
        }
        None
    }

    pub fn update_unread_status(
        &mut self,
        read_states: &std::collections::HashMap<ChannelId, ReadState>,
    ) {
        for channels in self.channels_by_guild.values_mut() {
            for channel in channels {
                if let Some(read_state) = read_states.get(&channel.id()) {
                    if let Some(last_msg_id) = channel.last_message_id() {
                        let is_unread = if let Some(last_read_id) = read_state.last_read_message_id
                        {
                            last_msg_id.as_u64() > last_read_id.as_u64()
                        } else {
                            true
                        };
                        channel.set_unread(is_unread);
                    } else {
                        channel.set_unread(false);
                    }
                } else {
                    channel.set_unread(false);
                }
            }
        }
    }

    #[must_use]
    pub fn flatten<'a>(
        &'a self,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
    ) -> Vec<FlattenedNode<'a>> {
        let mut nodes = Vec::new();

        let dm_expanded = state.expanded.contains(&TreeNodeId::DirectMessages);
        nodes.push(FlattenedNode {
            id: TreeNodeId::DirectMessages,
            label: Line::from(vec![
                Span::styled(
                    if dm_expanded { "▾ " } else { "▸ " },
                    style.tree_guide_style,
                ),
                Span::raw("Direct Messages"),
            ]),
            depth: 0,
        });

        if dm_expanded {
            for (i, (id, name)) in self.dm_users.iter().enumerate() {
                let is_last = i == self.dm_users.len() - 1;
                let prefix = if is_last { "└── " } else { "├── " };

                let is_active = self.active_dm_user_id() == Some(id);
                let current_style = if is_active {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    style.dm_style
                };

                let clean_name = clean_text(name);

                nodes.push(FlattenedNode {
                    id: TreeNodeId::DirectMessageUser(id.clone()),
                    label: Line::from(vec![
                        Span::styled(prefix, style.tree_guide_style),
                        Span::styled("@ ", current_style),
                        Span::styled(clean_name, current_style),
                    ]),
                    depth: 1,
                });
            }
        }

        for (i, guild) in self.guilds.iter().enumerate() {
            let _is_last_guild = i == self.guilds.len() - 1;
            let guild_id = guild.id();
            let expanded = state.expanded.contains(&TreeNodeId::Guild(guild_id));

            let is_active = self.active_guild_id() == Some(guild_id);
            let guild_style = if is_active {
                style.active_guild_style
            } else if guild.has_unread() {
                style.guild_unread_style
            } else {
                style.guild_style
            };

            let clean_name = clean_text(guild.name());

            let arrow = if expanded { "▾ " } else { "▸ " };
            nodes.push(FlattenedNode {
                id: TreeNodeId::Guild(guild_id),
                label: Line::from(vec![
                    Span::styled(arrow, style.tree_guide_style),
                    Span::styled(clean_name, guild_style),
                ]),
                depth: 0,
            });

            if expanded {
                if let Some(channels) = self.channels(guild_id) {
                    self.flatten_channels(&mut nodes, channels, state, width, style);
                } else {
                    nodes.push(FlattenedNode {
                        id: TreeNodeId::Placeholder(guild_id),
                        label: Line::from(vec![
                            Span::styled("└── ", style.tree_guide_style),
                            Span::styled("Loading...", style.placeholder_style),
                        ]),
                        depth: 1,
                    });
                }
            }
        }

        nodes
    }

    fn flatten_channels<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        channels: &'a [Channel],
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
    ) {
        let mut categories: std::collections::HashMap<ChannelId, Vec<&Channel>> =
            std::collections::HashMap::new();
        let mut orphan_channels: Vec<&Channel> = Vec::new();
        let mut category_channels: Vec<&Channel> = Vec::new();

        for channel in channels {
            if channel.kind().is_category() {
                category_channels.push(channel);
            } else if let Some(parent_id) = channel.parent_id() {
                categories.entry(parent_id).or_default().push(channel);
            } else {
                orphan_channels.push(channel);
            }
        }

        orphan_channels.sort_by_key(|c| c.position());
        category_channels.sort_by_key(|c| c.position());

        for (i, channel) in orphan_channels.iter().enumerate() {
            let is_last = i == orphan_channels.len() - 1 && category_channels.is_empty();
            let prefix = if is_last { "└── " } else { "├── " };
            if let Some(node) = self.create_channel_node(channel, 1, is_last, prefix, width, style)
            {
                nodes.push(node);
            }
        }

        for (i, category) in category_channels.iter().enumerate() {
            let is_last_category = i == category_channels.len() - 1;
            let cat_prefix = if is_last_category {
                "└── "
            } else {
                "├── "
            };
            let child_indent = if is_last_category { "    " } else { "│   " };

            let expanded = state
                .expanded
                .contains(&TreeNodeId::Category(category.id()));
            let arrow = if expanded { "▾ " } else { "▸ " };

            let clean_name = clean_text(category.name());

            nodes.push(FlattenedNode {
                id: TreeNodeId::Category(category.id()),
                label: Line::from(vec![
                    Span::styled(cat_prefix, style.tree_guide_style),
                    Span::styled(arrow, style.tree_guide_style),
                    Span::styled(clean_name.to_uppercase(), style.category_style),
                ]),
                depth: 1,
            });

            if expanded && let Some(children) = categories.get(&category.id()) {
                let mut sorted_children = children.clone();
                sorted_children.sort_by_key(|c| c.position());

                for (j, child) in sorted_children.iter().enumerate() {
                    let is_last_child = j == sorted_children.len() - 1;
                    let mut prefix = child_indent.to_string();
                    prefix.push_str(if is_last_child {
                        "└── "
                    } else {
                        "├── "
                    });

                    if let Some(node) =
                        self.create_channel_node(child, 2, is_last_child, &prefix, width, style)
                    {
                        nodes.push(node);
                    }
                }
            }
        }
    }

    fn create_channel_node<'a>(
        &'a self,
        channel: &'a Channel,
        depth: usize,
        _is_last: bool,
        prefix: &str,
        width: u16,
        style: &GuildsTreeStyle,
    ) -> Option<FlattenedNode<'a>> {
        if !channel.kind().is_text_based()
            && !channel.kind().is_voice()
            && channel.kind() != ChannelKind::Forum
        {
            return None;
        }

        let is_active = self.active_channel_id() == Some(channel.id());
        let channel_style = if is_active {
            style.active_channel_style
        } else if channel.has_unread() {
            style.channel_unread_style
        } else {
            style.channel_style
        };

        let channel_icon = channel.kind().prefix();
        let channel_icon_width =
            u16::try_from(unicode_width::UnicodeWidthStr::width(channel_icon)).unwrap_or(0);

        let prefix_width =
            u16::try_from(unicode_width::UnicodeWidthStr::width(prefix)).unwrap_or(u16::MAX);
        let dot_width = 1;
        let padding_right = 1;

        let max_name_width = width
            .saturating_sub(prefix_width)
            .saturating_sub(channel_icon_width)
            .saturating_sub(dot_width)
            .saturating_sub(padding_right);

        let mut clean_name = clean_text(channel.name());

        let name_width = u16::try_from(unicode_width::UnicodeWidthStr::width(clean_name.as_str()))
            .unwrap_or(u16::MAX);

        if name_width > max_name_width {
            let mut w = 0;
            let mut new_len = 0;
            for (idx, c) in clean_name.char_indices() {
                let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if w + cw > max_name_width as usize {
                    break;
                }
                w += cw;
                new_len = idx + c.len_utf8();
            }
            clean_name = clean_name[..new_len].to_string();
        }

        let mut spans = vec![Span::styled(prefix.to_string(), style.tree_guide_style)];
        spans.push(Span::styled(channel_icon.to_string(), channel_style));
        spans.push(Span::styled(clean_name.clone(), channel_style));

        if channel.has_unread() {
            let used_width = prefix_width
                .saturating_add(channel_icon_width)
                .saturating_add(
                    u16::try_from(unicode_width::UnicodeWidthStr::width(clean_name.as_str()))
                        .unwrap_or(0),
                );
            let total_available = width
                .saturating_sub(dot_width)
                .saturating_sub(padding_right);
            let padding_needed = total_available.saturating_sub(used_width);

            if padding_needed > 0 {
                spans.push(Span::raw(" ".repeat(padding_needed as usize)));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled("⦁", style.channel_unread_style));
        }

        Some(FlattenedNode {
            id: TreeNodeId::Channel(channel.id()),
            label: Line::from(spans),
            depth,
        })
    }
}

impl Default for GuildsTreeData {
    fn default() -> Self {
        Self::new()
    }
}

/// Widget for displaying the guilds tree.
pub struct GuildsTree<'a> {
    data: &'a GuildsTreeData,
    style: GuildsTreeStyle,
    title: &'a str,
}

impl<'a> GuildsTree<'a> {
    #[must_use]
    pub fn new(data: &'a GuildsTreeData) -> Self {
        Self {
            data,
            style: GuildsTreeStyle::default(),
            title: "Guilds",
        }
    }

    #[must_use]
    pub const fn style(mut self, style: GuildsTreeStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}

impl StatefulWidget for GuildsTree<'_> {
    type State = GuildsTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_style = if state.is_focused() {
            self.style.border_style_focused
        } else {
            self.style.border_style
        };

        Widget::render(ratatui::widgets::Clear, area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                format!(" {} ", self.title),
                self.style.title_style,
            ));

        let inner_area = block.inner(area);

        let flattened_nodes = self.data.flatten(state, inner_area.width, &self.style);

        if let Some(selected_id) = &state.selected {
            if let Some(index) = flattened_nodes.iter().position(|n| &n.id == selected_id) {
                state.list_state.select(Some(index));
            } else {
                state.list_state.select(None);
            }
        } else {
            state.list_state.select(None);
        }

        let items: Vec<ListItem> = flattened_nodes
            .iter()
            .map(|node| {
                let is_selected = state.selected.as_ref() == Some(&node.id);

                let mut label = node.label.clone();

                if is_selected {
                    let selected_style = self.style.selected_style;
                    for span in &mut label.spans {
                        if let Some(bg) = selected_style.bg {
                            span.style = span.style.bg(bg);
                        }

                        let content = span.content.as_ref();
                        let is_guide =
                            content.contains('├') || content.contains('└') || content.contains('│');

                        if !is_guide {
                            if let Some(fg) = selected_style.fg {
                                span.style = span.style.fg(fg);
                            }
                            if !selected_style.add_modifier.is_empty() {
                                span.style = span.style.add_modifier(selected_style.add_modifier);
                            }
                        }
                    }
                }

                ListItem::new(label)
            })
            .collect();

        let list = List::new(items).block(block).highlight_style(
            Style::default().bg(self.style.selected_style.bg.unwrap_or(Color::DarkGray)),
        );

        StatefulWidget::render(list, area, buf, &mut state.list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn test_guilds_tree_state_creation() {
        let state = GuildsTreeState::new();
        assert!(!state.is_focused());
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_guilds_tree_state_focus() {
        let mut state = GuildsTreeState::new();
        state.set_focused(true);
        assert!(state.is_focused());
    }

    #[test]
    fn test_tree_node_id_display() {
        assert_eq!(TreeNodeId::DirectMessages.to_string(), "dm");
        assert_eq!(TreeNodeId::Guild(GuildId(123)).to_string(), "guild:123");
        assert_eq!(TreeNodeId::Channel(ChannelId(456)).to_string(), "ch:456");
    }

    #[test]
    fn test_guilds_tree_data() {
        let mut data = GuildsTreeData::new();
        data.set_guilds(vec![Guild::new(1_u64, "Test Guild")]);
        assert_eq!(data.guilds().len(), 1);
    }

    #[test]
    fn test_handle_navigation_keys() {
        let mut state = GuildsTreeState::new();
        let registry = CommandRegistry::default();
        let mut data = GuildsTreeData::new();
        data.set_guilds(vec![Guild::new(1_u64, "Test Guild")]);
        let style = GuildsTreeStyle::default();

        state.handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            &data,
            &registry,
            &style,
        );
        let flattened = data.flatten(&state, 100, &style);
        state.select(flattened[0].id.clone());

        let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

        assert!(state.handle_key(key_j, &data, &registry, &style).is_none());
    }

    #[test]
    fn test_flatten_includes_all_guilds() {
        let mut data = GuildsTreeData::new();
        let guilds = vec![
            Guild::new(1_u64, "Nix/NixOS (unofficial)"),
            Guild::new(2_u64, "r/Unixporn"),
            Guild::new(3_u64, "RespriteApp"),
            Guild::new(4_u64, "Oxicord"),
            Guild::new(5_u64, "Noctalia"),
            Guild::new(6_u64, "L I N U X's TEST SERVER"),
            Guild::new(7_u64, "OpenCode"),
            Guild::new(8_u64, "OpenCode Antigravity Auth"),
        ];

        data.set_guilds(guilds.clone());
        let state = GuildsTreeState::new();
        let style = GuildsTreeStyle::default();
        let nodes = data.flatten(&state, 100, &style);

        assert_eq!(nodes.len(), guilds.len() + 1);

        for (i, guild) in guilds.iter().enumerate() {
            let expected_id = TreeNodeId::Guild(guild.id());
            let node = &nodes[i + 1];
            assert_eq!(node.id, expected_id);
        }
    }

    #[test]
    fn test_large_guild_list() {
        let mut data = GuildsTreeData::new();
        let guilds: Vec<Guild> = (0u64..100)
            .map(|i| Guild::new(i, format!("Guild {i}")))
            .collect();

        data.set_guilds(guilds);
        let state = GuildsTreeState::new();
        let style = GuildsTreeStyle::default();
        let nodes = data.flatten(&state, 100, &style);

        assert_eq!(nodes.len(), 101, "Should have DM node + 100 guilds");
    }
}
