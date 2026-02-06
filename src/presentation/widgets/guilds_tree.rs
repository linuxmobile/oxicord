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

use crate::application::services::identity_resolver::IdentityResolver;
use crate::domain::entities::{
    Channel, ChannelId, ChannelKind, Guild, GuildFolder, GuildId, ReadState, User,
};
use crate::domain::keybinding::Action;
use crate::domain::ports::DirectMessageChannel;
use crate::presentation::commands::CommandRegistry;
use crate::presentation::theme::Theme;
use crate::presentation::ui::utils::clean_text;

/// Unique identifier for nodes in the guilds tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    DirectMessages,

    DirectMessageUser(String),

    Folder(Option<u64>),

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
            Self::Folder(id) => write!(f, "folder:{id:?}"),
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
        use_display_name: bool,
    ) -> Option<GuildsTreeAction> {
        let flattened = data.flatten(self, u16::MAX, style, use_display_name);

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
                        TreeNodeId::Guild(_)
                            | TreeNodeId::Category(_)
                            | TreeNodeId::DirectMessages
                            | TreeNodeId::Folder(_)
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
                        TreeNodeId::Category(_)
                        | TreeNodeId::DirectMessages
                        | TreeNodeId::Folder(_) => {
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
                    TreeNodeId::Folder(id) => format!("{id:?}"),
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
    pub folder_style: Style,
    pub mention_style: Style,
}

impl GuildsTreeStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            border_style: theme.border_style,
            border_style_focused: Style::default().fg(theme.accent),
            title_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            selected_style: theme.selection_style,
            active_guild_style: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            active_channel_style: theme.selection_style.add_modifier(Modifier::BOLD),
            dm_style: Style::default().fg(theme.accent),
            tree_guide_style: theme.dimmed_style,
            folder_style: Style::default().fg(theme.accent),
            mention_style: Style::default().fg(theme.accent),
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
            guild_unread_style: Style::default().fg(Color::White),
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
            folder_style: Style::default().fg(Color::Blue),
            mention_style: Style::default().fg(Color::Red),
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
pub struct CategoryNode {
    pub category: Channel,
    pub children: Vec<Channel>,
}

pub struct SortedGuildChannels {
    pub orphans: Vec<Channel>,
    pub categories: Vec<CategoryNode>,
    pub threads: Vec<Channel>,
}

impl SortedGuildChannels {
    pub fn iter(&self) -> impl Iterator<Item = &Channel> {
        self.orphans
            .iter()
            .chain(
                self.categories
                    .iter()
                    .flat_map(|cat| std::iter::once(&cat.category).chain(cat.children.iter())),
            )
            .chain(self.threads.iter())
    }
}

pub struct GuildsTreeData {
    guilds: Vec<Guild>,
    folders: Vec<GuildFolder>,
    group_guilds: bool,
    channels_by_guild: std::collections::HashMap<GuildId, SortedGuildChannels>,
    dm_users: Vec<DirectMessageChannel>,
    active_guild_id: Option<GuildId>,
    active_channel_id: Option<ChannelId>,
    active_dm_user_id: Option<String>,
}

impl GuildsTreeData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            guilds: Vec::new(),
            folders: Vec::new(),
            group_guilds: false,
            channels_by_guild: std::collections::HashMap::new(),
            dm_users: Vec::new(),
            active_guild_id: None,
            active_channel_id: None,
            active_dm_user_id: None,
        }
    }

    pub fn set_guilds(&mut self, guilds: Vec<Guild>) {
        self.guilds = guilds;
        self.sort_guilds();
    }

    pub fn set_folders(&mut self, folders: Vec<GuildFolder>) {
        self.folders = folders;
        self.sort_guilds();
    }

    fn sort_guilds(&mut self) {
        if self.folders.is_empty() {
            return;
        }

        let mut positions = std::collections::HashMap::new();
        let mut index = 0;

        for folder in &self.folders {
            for guild_id in &folder.guild_ids {
                positions.insert(*guild_id, index);
                index += 1;
            }
        }

        for guild in &mut self.guilds {
            if let Some(pos) = positions.get(&guild.id()) {
                guild.set_position(*pos);
            }
        }

        self.guilds
            .sort_by_key(crate::domain::entities::Guild::position);
    }

    pub fn set_group_guilds(&mut self, group: bool) {
        self.group_guilds = group;
    }

    pub fn set_channels(&mut self, guild_id: GuildId, channels: Vec<Channel>) {
        tracing::debug!(
            guild_id = %guild_id,
            guild_id_raw = guild_id.as_u64(),
            channel_count = channels.len(),
            "Storing channels for guild"
        );

        let mut orphans = Vec::new();
        let mut threads = Vec::new();
        let mut category_map: std::collections::HashMap<ChannelId, (Channel, Vec<Channel>)> =
            std::collections::HashMap::new();

        let temp_channels = channels;
        let mut potential_children = Vec::new();

        for channel in temp_channels {
            if channel.kind().is_thread() {
                threads.push(channel);
            } else if channel.kind().is_category() {
                category_map.insert(channel.id(), (channel, Vec::new()));
            } else {
                potential_children.push(channel);
            }
        }

        for channel in potential_children {
            if let Some(parent_id) = channel.parent_id() {
                if let Some((_, children)) = category_map.get_mut(&parent_id) {
                    children.push(channel);
                } else {
                    orphans.push(channel);
                }
            } else {
                orphans.push(channel);
            }
        }

        let sort_fn = |a: &Channel, b: &Channel| match a.position().cmp(&b.position()) {
            std::cmp::Ordering::Equal => {
                let a_is_text = a.kind().is_text_based() || a.kind() == ChannelKind::Forum;
                let b_is_text = b.kind().is_text_based() || b.kind() == ChannelKind::Forum;

                match (a_is_text, b_is_text) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => match (a.kind() as u8).cmp(&(b.kind() as u8)) {
                        std::cmp::Ordering::Equal => a.id().as_u64().cmp(&b.id().as_u64()),
                        other => other,
                    },
                }
            }
            other => other,
        };

        orphans.sort_by(sort_fn);
        threads.sort_by(sort_fn);

        let mut categories: Vec<CategoryNode> = category_map
            .into_values()
            .map(|(category, mut children)| {
                children.sort_by(sort_fn);
                CategoryNode { category, children }
            })
            .filter(|node| !node.children.is_empty())
            .collect();

        categories.sort_by(|a, b| sort_fn(&a.category, &b.category));

        self.channels_by_guild.insert(
            guild_id,
            SortedGuildChannels {
                orphans,
                categories,
                threads,
            },
        );
    }

    pub fn set_dm_users(&mut self, mut users: Vec<DirectMessageChannel>) {
        users.sort_by(|a, b| {
            b.last_message_id
                .map_or(0, crate::domain::entities::MessageId::as_u64)
                .cmp(
                    &a.last_message_id
                        .map_or(0, crate::domain::entities::MessageId::as_u64),
                )
        });
        self.dm_users = users;
    }

    #[must_use]
    pub fn guilds(&self) -> &[Guild] {
        &self.guilds
    }

    #[must_use]
    pub fn channels(&self, guild_id: GuildId) -> Option<&SortedGuildChannels> {
        self.channels_by_guild.get(&guild_id)
    }

    #[must_use]
    pub fn dm_users(&self) -> &[DirectMessageChannel] {
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
        for (guild_id, sorted) in &self.channels_by_guild {
            if sorted.orphans.iter().any(|c| c.id() == channel_id) {
                return Some(*guild_id);
            }
            if sorted.threads.iter().any(|c| c.id() == channel_id) {
                return Some(*guild_id);
            }
            for cat in &sorted.categories {
                if cat.category.id() == channel_id {
                    return Some(*guild_id);
                }
                if cat.children.iter().any(|c| c.id() == channel_id) {
                    return Some(*guild_id);
                }
            }
        }
        None
    }

    pub fn get_channel_mut(&mut self, channel_id: ChannelId) -> Option<&mut Channel> {
        for sorted in self.channels_by_guild.values_mut() {
            if let Some(channel) = sorted.orphans.iter_mut().find(|c| c.id() == channel_id) {
                return Some(channel);
            }
            if let Some(channel) = sorted.threads.iter_mut().find(|c| c.id() == channel_id) {
                return Some(channel);
            }
            for cat in &mut sorted.categories {
                if cat.category.id() == channel_id {
                    return Some(&mut cat.category);
                }
                if let Some(channel) = cat.children.iter_mut().find(|c| c.id() == channel_id) {
                    return Some(channel);
                }
            }
        }
        None
    }

    #[must_use]
    pub fn get_channel(&self, channel_id: ChannelId) -> Option<&Channel> {
        for sorted in self.channels_by_guild.values() {
            if let Some(channel) = sorted.orphans.iter().find(|c| c.id() == channel_id) {
                return Some(channel);
            }
            if let Some(channel) = sorted.threads.iter().find(|c| c.id() == channel_id) {
                return Some(channel);
            }
            for cat in &sorted.categories {
                if cat.category.id() == channel_id {
                    return Some(&cat.category);
                }
                if let Some(channel) = cat.children.iter().find(|c| c.id() == channel_id) {
                    return Some(channel);
                }
            }
        }
        None
    }

    #[allow(clippy::items_after_statements)]
    #[must_use]
    pub fn flatten<'a>(
        &'a self,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        use_display_name: bool,
    ) -> Vec<FlattenedNode<'a>> {
        let mut nodes = Vec::new();

        enum RootItem<'a> {
            Dm,
            Folder(&'a GuildFolder),
            Guild(&'a Guild),
        }

        let mut items = Vec::new();
        items.push(RootItem::Dm);

        if self.group_guilds {
            let mut processed_guilds = std::collections::HashSet::new();
            for folder in &self.folders {
                if folder.id.is_none() {
                    for gid in &folder.guild_ids {
                        if let Some(guild) = self.guilds.iter().find(|g| g.id() == *gid) {
                            items.push(RootItem::Guild(guild));
                            processed_guilds.insert(*gid);
                        }
                    }
                } else {
                    items.push(RootItem::Folder(folder));
                    for gid in &folder.guild_ids {
                        processed_guilds.insert(*gid);
                    }
                }
            }
            // Add any guilds not in folders (fallback)
            for guild in &self.guilds {
                if !processed_guilds.contains(&guild.id()) {
                    items.push(RootItem::Guild(guild));
                }
            }
        } else {
            for guild in &self.guilds {
                items.push(RootItem::Guild(guild));
            }
        }

        for item in items {
            let children_base_indent = "";

            match item {
                RootItem::Dm => {
                    self.render_dm_node(
                        &mut nodes,
                        state,
                        width,
                        style,
                        children_base_indent,
                        use_display_name,
                    );
                }
                RootItem::Folder(folder) => {
                    self.render_folder_node(
                        &mut nodes,
                        folder,
                        state,
                        width,
                        style,
                        children_base_indent,
                    );
                }
                RootItem::Guild(guild) => {
                    self.render_guild_node(
                        &mut nodes,
                        guild,
                        state,
                        width,
                        style,
                        "",
                        children_base_indent,
                    );
                }
            }
        }

        nodes
    }

    pub fn update_unread_status(
        &mut self,
        read_states: &std::collections::HashMap<ChannelId, ReadState>,
    ) {
        for sorted in self.channels_by_guild.values_mut() {
            let update_channel = |channel: &mut Channel| {
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
                    channel.set_mention_count(read_state.mention_count);
                } else {
                    channel.set_unread(false);
                    channel.set_mention_count(0);
                }
            };

            for channel in &mut sorted.orphans {
                update_channel(channel);
            }
            for channel in &mut sorted.threads {
                update_channel(channel);
            }
            for cat in &mut sorted.categories {
                update_channel(&mut cat.category);
                for channel in &mut cat.children {
                    update_channel(channel);
                }
            }
        }

        for guild in &mut self.guilds {
            if let Some(sorted) = self.channels_by_guild.get(&guild.id()) {
                let mut guild_mentions = 0;
                let mut guild_unread = false;

                let mut check_channel = |channel: &Channel| {
                    guild_mentions += channel.mention_count();
                    if channel.has_unread() {
                        guild_unread = true;
                    }
                };

                for channel in &sorted.orphans {
                    check_channel(channel);
                }
                for channel in &sorted.threads {
                    check_channel(channel);
                }
                for cat in &sorted.categories {
                    check_channel(&cat.category);
                    for channel in &cat.children {
                        check_channel(channel);
                    }
                }

                guild.set_mention_count(guild_mentions);
                guild.set_unread(guild_unread || guild_mentions > 0);
            }
        }

        for dm in &mut self.dm_users {
            if let Ok(channel_id) = dm.channel_id.parse::<u64>() {
                let channel_id = ChannelId(channel_id);
                if let Some(read_state) = read_states.get(&channel_id) {
                    if let Some(last_msg_id) = dm.last_message_id {
                        let is_unread = if let Some(last_read_id) = read_state.last_read_message_id
                        {
                            last_msg_id.as_u64() > last_read_id.as_u64()
                        } else {
                            true
                        };
                        dm.has_unread = is_unread;
                    } else {
                        dm.has_unread = false;
                    }
                    dm.mention_count = read_state.mention_count;
                } else {
                    dm.has_unread = false;
                    dm.mention_count = 0;
                }
            }
        }
    }

    fn render_dm_node<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        children_base_indent: &'a str,
        use_display_name: bool,
    ) {
        let expanded = state.expanded.contains(&TreeNodeId::DirectMessages);
        let arrow = if expanded { "▾ " } else { "▸ " };

        let total_mentions: u32 = self.dm_users.iter().map(|dm| dm.mention_count).sum();
        let any_unread = self.dm_users.iter().any(|dm| dm.has_unread);

        let mut spans = vec![
            Span::styled(arrow, style.tree_guide_style),
            Span::raw("Direct Messages"),
        ];

        if total_mentions > 0 || any_unread {
            let used_width = 2 + 15;
            let padding_needed = width.saturating_sub(used_width).saturating_sub(2);

            if padding_needed > 0 {
                spans.push(Span::raw(" ".repeat(padding_needed as usize)));
            } else {
                spans.push(Span::raw(" "));
            }

            if total_mentions > 0 {
                spans.push(Span::styled("⦁", style.mention_style));
            } else {
                spans.push(Span::styled("⦁", style.channel_unread_style));
            }
        }

        nodes.push(FlattenedNode {
            id: TreeNodeId::DirectMessages,
            label: Line::from(spans),
            depth: 0,
        });

        if expanded {
            for (i, dm) in self.dm_users.iter().enumerate() {
                let is_last = i == self.dm_users.len() - 1;
                let prefix = if is_last { "└── " } else { "├── " };

                let is_active = self.active_dm_user_id() == Some(&dm.channel_id);
                let current_style = if is_active {
                    style.active_channel_style
                } else if dm.has_unread {
                    style.channel_unread_style
                } else {
                    style.dm_style
                };

                let mut clean_name = clean_text(&dm.recipient_username);

                if let Ok(_channel_id) = dm.channel_id.parse::<u64>() {
                    let user = User::new(
                        dm.recipient_id.clone(),
                        dm.recipient_username.clone(),
                        &dm.recipient_discriminator,
                        None,
                        false,
                        None,
                    )
                    .with_global_name(dm.recipient_global_name.clone().unwrap_or_default());

                    let preferred_name =
                        IdentityResolver::with_preference(use_display_name).resolve(&user);
                    clean_name = clean_text(&preferred_name);
                }

                let mut spans = vec![
                    Span::styled(children_base_indent, style.tree_guide_style),
                    Span::styled(prefix, style.tree_guide_style),
                    Span::styled("@ ", current_style),
                    Span::styled(clean_name.clone(), current_style),
                ];

                if dm.mention_count > 0 || dm.has_unread {
                    let base_indent_width =
                        u16::try_from(unicode_width::UnicodeWidthStr::width(children_base_indent))
                            .unwrap_or(0);
                    let prefix_width =
                        u16::try_from(unicode_width::UnicodeWidthStr::width(prefix)).unwrap_or(0);
                    let at_width = 2;
                    let name_width =
                        u16::try_from(unicode_width::UnicodeWidthStr::width(clean_name.as_str()))
                            .unwrap_or(0);

                    let used_width = base_indent_width + prefix_width + at_width + name_width;
                    let padding_needed = width.saturating_sub(used_width).saturating_sub(2);

                    if padding_needed > 0 {
                        spans.push(Span::raw(" ".repeat(padding_needed as usize)));
                    } else {
                        spans.push(Span::raw(" "));
                    }

                    if dm.mention_count > 0 {
                        spans.push(Span::styled("⦁", style.mention_style));
                    } else {
                        spans.push(Span::styled("⦁", style.channel_unread_style));
                    }
                }

                nodes.push(FlattenedNode {
                    id: TreeNodeId::DirectMessageUser(dm.channel_id.clone()),
                    label: Line::from(spans),
                    depth: 1,
                });
            }
        }
    }

    fn render_folder_node<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        folder: &'a GuildFolder,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        children_base_indent: &'a str,
    ) {
        let expanded = state.expanded.contains(&TreeNodeId::Folder(folder.id));
        let folder_name = folder.name.as_deref().unwrap_or("Folder");
        let folder_icon = " ";

        let mut folder_style = style.folder_style;
        if let Some(color) = folder.color {
            let r = ((color >> 16) & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let b = (color & 0xFF) as u8;
            folder_style = folder_style.fg(Color::Rgb(r, g, b));
        }

        let arrow = if expanded { "▾ " } else { "▸ " };
        nodes.push(FlattenedNode {
            id: TreeNodeId::Folder(folder.id),
            label: Line::from(vec![
                Span::styled(arrow, style.tree_guide_style),
                Span::styled(folder_icon, folder_style),
                Span::styled(folder_name, folder_style),
            ]),
            depth: 0,
        });

        if expanded {
            for (i, guild_id) in folder.guild_ids.iter().enumerate() {
                if let Some(guild) = self.guilds.iter().find(|g| g.id() == *guild_id) {
                    let is_last_in_folder = i == folder.guild_ids.len() - 1;

                    let prefix = if is_last_in_folder {
                        "└── "
                    } else {
                        "├── "
                    };

                    let guild_sibling_indent = if is_last_in_folder { "    " } else { "│   " };

                    self.render_guild_node_nested(
                        nodes,
                        guild,
                        state,
                        width,
                        style,
                        children_base_indent,
                        prefix,
                        guild_sibling_indent,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_guild_node_nested<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        guild: &'a Guild,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        base_indent_1: &'a str,
        connector: &'a str,
        base_indent_2: &'a str,
    ) {
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

        let mut spans = vec![
            Span::styled(base_indent_1, style.tree_guide_style),
            Span::styled(connector, style.tree_guide_style),
            Span::styled(arrow, style.tree_guide_style),
            Span::styled(clean_name.clone(), guild_style),
        ];

        if guild.mention_count() > 0 || guild.has_unread() {
            let used_width = u16::try_from(unicode_width::UnicodeWidthStr::width(base_indent_1))
                .unwrap_or(0)
                .saturating_add(
                    u16::try_from(unicode_width::UnicodeWidthStr::width(connector)).unwrap_or(0),
                )
                .saturating_add(2)
                .saturating_add(
                    u16::try_from(unicode_width::UnicodeWidthStr::width(clean_name.as_str()))
                        .unwrap_or(0),
                );

            let padding_needed = width.saturating_sub(used_width).saturating_sub(2);

            if padding_needed > 0 {
                spans.push(Span::raw(" ".repeat(padding_needed as usize)));
            } else {
                spans.push(Span::raw(" "));
            }

            if guild.mention_count() > 0 {
                spans.push(Span::styled("⦁", style.mention_style));
            }
        }

        nodes.push(FlattenedNode {
            id: TreeNodeId::Guild(guild_id),
            label: Line::from(spans),
            depth: 0,
        });

        if expanded {
            if let Some(channels) = self.channels(guild_id) {
                self.flatten_channels_nested(
                    nodes,
                    channels,
                    state,
                    width,
                    style,
                    base_indent_1,
                    base_indent_2,
                );
            } else {
                nodes.push(FlattenedNode {
                    id: TreeNodeId::Placeholder(guild_id),
                    label: Line::from(vec![
                        Span::styled(base_indent_1, style.tree_guide_style),
                        Span::styled(base_indent_2, style.tree_guide_style),
                        Span::styled("└── ", style.tree_guide_style),
                        Span::styled("Loading...", style.placeholder_style),
                    ]),
                    depth: 1,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_guild_node<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        guild: &'a Guild,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        _prefix_unused: &'a str,
        children_base_indent: &'a str,
    ) {
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

        let mut spans = vec![
            Span::styled(arrow, style.tree_guide_style),
            Span::styled(clean_name.clone(), guild_style),
        ];

        if guild.mention_count() > 0 || guild.has_unread() {
            let used_width =
                2 + u16::try_from(unicode_width::UnicodeWidthStr::width(clean_name.as_str()))
                    .unwrap_or(0);
            let padding_needed = width.saturating_sub(used_width).saturating_sub(2);

            if padding_needed > 0 {
                spans.push(Span::raw(" ".repeat(padding_needed as usize)));
            } else {
                spans.push(Span::raw(" "));
            }

            if guild.mention_count() > 0 {
                spans.push(Span::styled("⦁", style.mention_style));
            }
        }

        nodes.push(FlattenedNode {
            id: TreeNodeId::Guild(guild_id),
            label: Line::from(spans),
            depth: 0,
        });

        if expanded {
            if let Some(channels) = self.channels(guild_id) {
                self.flatten_channels(nodes, channels, state, width, style, children_base_indent);
            } else {
                nodes.push(FlattenedNode {
                    id: TreeNodeId::Placeholder(guild_id),
                    label: Line::from(vec![
                        Span::styled(children_base_indent, style.tree_guide_style),
                        Span::styled("└── ", style.tree_guide_style),
                        Span::styled("Loading...", style.placeholder_style),
                    ]),
                    depth: 1,
                });
            }
        }
    }

    fn flatten_channels<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        channels: &'a SortedGuildChannels,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        base_indent: &'a str,
    ) {
        self.flatten_channels_nested(nodes, channels, state, width, style, base_indent, "");
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn flatten_channels_nested<'a>(
        &'a self,
        nodes: &mut Vec<FlattenedNode<'a>>,
        channels: &'a SortedGuildChannels,
        state: &GuildsTreeState,
        width: u16,
        style: &GuildsTreeStyle,
        base_indent_1: &'a str,
        base_indent_2: &'a str,
    ) {
        for (i, channel) in channels.orphans.iter().enumerate() {
            let is_last = i == channels.orphans.len() - 1 && channels.categories.is_empty();
            let prefix = if is_last { "└── " } else { "├── " };
            if let Some(node) = self.create_channel_node(
                channel,
                1,
                prefix,
                width,
                style,
                base_indent_1,
                base_indent_2,
            ) {
                nodes.push(node);
            }
        }

        for (i, cat_node) in channels.categories.iter().enumerate() {
            let is_last_category = i == channels.categories.len() - 1;
            let cat_prefix = if is_last_category {
                "└── "
            } else {
                "├── "
            };
            let child_indent_comp = if is_last_category { "    " } else { "│   " };

            let category = &cat_node.category;
            let expanded = state
                .expanded
                .contains(&TreeNodeId::Category(category.id()));
            let arrow = if expanded { "▾ " } else { "▸ " };
            let clean_name = clean_text(category.name());

            let mut cat_mentions = 0;
            for child in &cat_node.children {
                cat_mentions += child.mention_count();
            }

            let mut spans = vec![
                Span::styled(base_indent_1, style.tree_guide_style),
                Span::styled(base_indent_2, style.tree_guide_style),
                Span::styled(cat_prefix, style.tree_guide_style),
                Span::styled(arrow, style.tree_guide_style),
                Span::styled(clean_name.to_uppercase(), style.category_style),
            ];

            if cat_mentions > 0 {
                let used_width =
                    u16::try_from(unicode_width::UnicodeWidthStr::width(base_indent_1))
                        .unwrap_or(0)
                        .saturating_add(
                            u16::try_from(unicode_width::UnicodeWidthStr::width(base_indent_2))
                                .unwrap_or(0),
                        )
                        .saturating_add(
                            u16::try_from(unicode_width::UnicodeWidthStr::width(cat_prefix))
                                .unwrap_or(0),
                        )
                        .saturating_add(2)
                        .saturating_add(
                            u16::try_from(unicode_width::UnicodeWidthStr::width(
                                clean_name.to_uppercase().as_str(),
                            ))
                            .unwrap_or(0),
                        );

                let total_available = width.saturating_sub(2);
                let padding_needed = total_available.saturating_sub(used_width);

                if padding_needed > 0 {
                    spans.push(Span::raw(" ".repeat(padding_needed as usize)));
                } else {
                    spans.push(Span::raw(" "));
                }

                spans.push(Span::styled("⦁", style.mention_style));
            }

            nodes.push(FlattenedNode {
                id: TreeNodeId::Category(category.id()),
                label: Line::from(spans),
                depth: 1,
            });

            if expanded {
                for (j, child) in cat_node.children.iter().enumerate() {
                    let is_last_child = j == cat_node.children.len() - 1;
                    let mut prefix = child_indent_comp.to_string();
                    prefix.push_str(if is_last_child {
                        "└── "
                    } else {
                        "├── "
                    });

                    if let Some(node) = self.create_channel_node(
                        child,
                        2,
                        &prefix,
                        width,
                        style,
                        base_indent_1,
                        base_indent_2,
                    ) {
                        nodes.push(node);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_channel_node<'a>(
        &'a self,
        channel: &'a Channel,
        depth: usize,
        prefix: &str,
        width: u16,
        style: &GuildsTreeStyle,
        base_indent_1: &'a str,
        base_indent_2: &'a str,
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

        let mut spans = vec![
            Span::styled(base_indent_1, style.tree_guide_style),
            Span::styled(base_indent_2, style.tree_guide_style),
        ];
        spans.push(Span::styled(prefix.to_string(), style.tree_guide_style));
        spans.push(Span::styled(channel_icon.to_string(), channel_style));
        spans.push(Span::styled(clean_name.clone(), channel_style));

        if channel.mention_count() > 0 || channel.has_unread() {
            let used_width = u16::try_from(unicode_width::UnicodeWidthStr::width(base_indent_1))
                .unwrap_or(0)
                .saturating_add(
                    u16::try_from(unicode_width::UnicodeWidthStr::width(base_indent_2))
                        .unwrap_or(0),
                )
                .saturating_add(prefix_width)
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

            if channel.mention_count() > 0 {
                spans.push(Span::styled("⦁", style.mention_style));
            } else {
                spans.push(Span::styled("⦁", style.channel_unread_style));
            }
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
    use_display_name: bool,
}

impl<'a> GuildsTree<'a> {
    #[must_use]
    pub fn new(data: &'a GuildsTreeData) -> Self {
        Self {
            data,
            style: GuildsTreeStyle::default(),
            title: "Guilds",
            use_display_name: true,
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

    #[must_use]
    pub const fn use_display_name(mut self, use_display_name: bool) -> Self {
        self.use_display_name = use_display_name;
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

        let flattened_nodes =
            self.data
                .flatten(state, inner_area.width, &self.style, self.use_display_name);

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
                        let is_dot = content.contains('⦁');

                        if !is_guide {
                            if !is_dot && let Some(fg) = selected_style.fg {
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
            true,
        );
        let flattened = data.flatten(&state, 100, &style, true);
        state.select(flattened[0].id.clone());

        let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

        assert!(
            state
                .handle_key(key_j, &data, &registry, &style, true)
                .is_none()
        );
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
        let nodes = data.flatten(&state, 100, &style, true);

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
        let start = std::time::Instant::now();
        let nodes = data.flatten(&state, 100, &style, true);
        let duration = start.elapsed();

        println!("Flattened {} nodes in {:?}", nodes.len(), duration);
        assert!(duration < std::time::Duration::from_millis(5));
    }

    #[test]
    fn test_thread_exclusion_from_tree() {
        let mut data = GuildsTreeData::new();
        let guild_id = GuildId(1);
        let guild = Guild::new(guild_id, "Test Guild");
        data.set_guilds(vec![guild]);

        let channels = vec![
            Channel::new(10_u64, "general", ChannelKind::Text).with_guild(guild_id),
            Channel::new(20_u64, "thread", ChannelKind::PublicThread)
                .with_guild(guild_id)
                .with_parent(10_u64),
            Channel::new(30_u64, "voice", ChannelKind::Voice).with_guild(guild_id),
        ];

        data.set_channels(guild_id, channels);

        let sorted = data.channels(guild_id).expect("Should have channels");

        assert!(!sorted.orphans.iter().any(|c| c.kind().is_thread()));

        for cat in &sorted.categories {
            assert!(!cat.category.kind().is_thread());
            assert!(!cat.children.iter().any(|c| c.kind().is_thread()));
        }

        assert!(sorted.threads.iter().any(|c| c.id().as_u64() == 20));

        let mut state = GuildsTreeState::new();
        state.expand(TreeNodeId::Guild(guild_id));

        let style = GuildsTreeStyle::default();
        let flattened = data.flatten(&state, 100, &style, true);

        let has_thread_flattened = flattened.iter().any(|node| {
            if let TreeNodeId::Channel(id) = node.id {
                id.as_u64() == 20
            } else {
                false
            }
        });

        assert!(
            !has_thread_flattened,
            "Thread should not appear in the flattened tree"
        );
    }
}
