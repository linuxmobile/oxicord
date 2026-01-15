//! Guilds tree widget for server/channel navigation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, StatefulWidget},
};
use tui_tree_widget::{Tree, TreeItem, TreeState};

use crate::domain::entities::{Channel, ChannelId, Guild, GuildId};

/// Unique identifier for nodes in the guilds tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    /// Root node for direct messages.
    DirectMessages,
    /// A direct message conversation with a user.
    DirectMessageUser(String),
    /// A guild (server) node.
    Guild(GuildId),
    /// A category (channel group) node.
    Category(ChannelId),
    /// A channel node.
    Channel(ChannelId),
    /// Placeholder node for unloaded content.
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
    /// A channel was selected.
    SelectChannel(ChannelId),
    /// A guild was selected.
    SelectGuild(GuildId),
    /// A direct message conversation was selected.
    SelectDirectMessage(String),
    /// An ID was yanked (copied).
    YankId(String),
    /// Request to load channels for a guild (lazy loading).
    LoadGuildChannels(GuildId),
}

/// State for the guilds tree widget.
pub struct GuildsTreeState {
    tree_state: TreeState<TreeNodeId>,
    focused: bool,
}

impl GuildsTreeState {
    /// Creates a new guilds tree state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tree_state: TreeState::default(),
            focused: false,
        }
    }

    /// Sets whether the tree is focused.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns whether the tree is focused.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Selects the next item in the tree.
    pub fn select_next(&mut self) {
        self.tree_state.key_down();
    }

    /// Selects the previous item in the tree.
    pub fn select_previous(&mut self) {
        self.tree_state.key_up();
    }

    /// Selects the first item in the tree.
    pub fn select_first(&mut self) {
        self.tree_state.select_first();
    }

    /// Selects the last item in the tree.
    pub fn select_last(&mut self) {
        self.tree_state.select_last();
    }

    /// Toggles expansion of the current node using the full selection path.
    pub fn toggle_current(&mut self) {
        self.tree_state.toggle_selected();
    }

    /// Expands the current node using the full selection path.
    pub fn expand_current(&mut self) {
        let selected_path = self.tree_state.selected().to_vec();
        if !selected_path.is_empty() {
            self.tree_state.open(selected_path);
        }
    }

    /// Collapses the current node using the full selection path.
    pub fn collapse_current(&mut self) {
        let selected_path = self.tree_state.selected().to_vec();
        if !selected_path.is_empty() {
            self.tree_state.close(&selected_path);
        }
    }

    /// Collapses the parent of the current node.
    pub fn collapse_parent(&mut self) {
        let selected = self.tree_state.selected().to_vec();
        if selected.len() > 1 {
            let parent_path: Vec<_> = selected[..selected.len() - 1].to_vec();
            self.tree_state.close(&parent_path);
        }
    }

    /// Moves selection to the parent node.
    pub fn move_to_parent(&mut self) {
        let selected = self.tree_state.selected().to_vec();
        if selected.len() > 1 {
            let parent_path: Vec<_> = selected[..selected.len() - 1].to_vec();
            self.tree_state.select(parent_path);
        }
    }

    /// Returns the currently selected path.
    #[must_use]
    pub fn selected(&self) -> &[TreeNodeId] {
        self.tree_state.selected()
    }

    /// Returns the currently selected node.
    #[must_use]
    pub fn current_selection(&self) -> Option<&TreeNodeId> {
        self.tree_state.selected().last()
    }

    /// Handles a key event and returns an optional action.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<GuildsTreeAction> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => {
                self.select_next();
                None
            }
            (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => {
                self.select_previous();
                None
            }
            (KeyCode::Char('h') | KeyCode::Left, KeyModifiers::NONE) => {
                self.tree_state.key_left();
                None
            }
            (KeyCode::Char('l') | KeyCode::Right, KeyModifiers::NONE) => {
                self.tree_state.key_right();
                None
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.select_first();
                None
            }
            (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                self.select_last();
                None
            }
            (KeyCode::Enter | KeyCode::Char(' '), KeyModifiers::NONE) => {
                self.toggle_current();
                self.get_selection_action()
            }
            (KeyCode::Char('-'), KeyModifiers::NONE) => {
                self.collapse_parent();
                None
            }
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                self.move_to_parent();
                None
            }
            (KeyCode::Char('i'), KeyModifiers::NONE) => self.current_selection().map(|node| {
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
        self.current_selection().and_then(|node| match node {
            TreeNodeId::Channel(id) => Some(GuildsTreeAction::SelectChannel(*id)),
            TreeNodeId::Guild(id) => Some(GuildsTreeAction::SelectGuild(*id)),
            TreeNodeId::DirectMessageUser(id) => {
                Some(GuildsTreeAction::SelectDirectMessage(id.clone()))
            }
            _ => None,
        })
    }

    /// Returns a mutable reference to the underlying tree state.
    pub fn tree_state_mut(&mut self) -> &mut TreeState<TreeNodeId> {
        &mut self.tree_state
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
            channel_unread_style: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            category_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            dm_style: Style::default().fg(Color::Magenta),
            placeholder_style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        }
    }
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
    /// Creates a new empty data container.
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

    /// Sets the list of guilds.
    pub fn set_guilds(&mut self, guilds: Vec<Guild>) {
        self.guilds = guilds;
    }

    /// Sets the channels for a specific guild.
    pub fn set_channels(&mut self, guild_id: GuildId, channels: Vec<Channel>) {
        tracing::debug!(
            guild_id = %guild_id,
            guild_id_raw = guild_id.as_u64(),
            channel_count = channels.len(),
            "Storing channels for guild"
        );
        self.channels_by_guild.insert(guild_id, channels);
    }

    /// Sets the list of DM users.
    pub fn set_dm_users(&mut self, users: Vec<(String, String)>) {
        self.dm_users = users;
    }

    /// Returns the list of guilds.
    #[must_use]
    pub fn guilds(&self) -> &[Guild] {
        &self.guilds
    }

    /// Returns the channels for a specific guild.
    #[must_use]
    pub fn channels(&self, guild_id: GuildId) -> Option<&Vec<Channel>> {
        self.channels_by_guild.get(&guild_id)
    }

    /// Returns the list of DM users.
    #[must_use]
    pub fn dm_users(&self) -> &[(String, String)] {
        &self.dm_users
    }

    /// Sets the active guild.
    pub fn set_active_guild(&mut self, guild_id: Option<GuildId>) {
        self.active_guild_id = guild_id;
    }

    /// Sets the active channel.
    pub fn set_active_channel(&mut self, channel_id: Option<ChannelId>) {
        self.active_channel_id = channel_id;
    }

    /// Sets the active DM user.
    pub fn set_active_dm_user(&mut self, user_id: Option<String>) {
        self.active_dm_user_id = user_id;
    }

    /// Returns the active guild ID.
    #[must_use]
    pub fn active_guild_id(&self) -> Option<GuildId> {
        self.active_guild_id
    }

    /// Returns the active channel ID.
    #[must_use]
    pub fn active_channel_id(&self) -> Option<ChannelId> {
        self.active_channel_id
    }

    /// Returns the active DM user ID.
    #[must_use]
    pub fn active_dm_user_id(&self) -> Option<&str> {
        self.active_dm_user_id.as_deref()
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
    /// Creates a new guilds tree widget.
    #[must_use]
    pub fn new(data: &'a GuildsTreeData) -> Self {
        Self {
            data,
            style: GuildsTreeStyle::default(),
            title: "Guilds",
        }
    }

    /// Sets the style configuration.
    #[must_use]
    pub fn style(mut self, style: GuildsTreeStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the title.
    #[must_use]
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    fn build_tree_items(&self) -> Vec<TreeItem<'static, TreeNodeId>> {
        let mut items = Vec::with_capacity(self.data.guilds.len() + 1);

        items.push(self.build_dm_node());

        for guild in &self.data.guilds {
            items.push(self.build_guild_node(guild));
        }

        items
    }

    fn build_dm_node(&self) -> TreeItem<'static, TreeNodeId> {
        let dm_children: Vec<TreeItem<'static, TreeNodeId>> = self
            .data
            .dm_users
            .iter()
            .map(|(id, name)| {
                let is_active = self.data.active_dm_user_id() == Some(id.as_str());
                let style = if is_active {
                    self.style.active_channel_style
                } else {
                    self.style.dm_style
                };
                let text = Line::from(vec![
                    Span::styled("@", style),
                    Span::styled(name.clone(), style),
                ]);
                TreeItem::new_leaf(TreeNodeId::DirectMessageUser(id.clone()), text)
            })
            .collect();

        let dm_text = Line::from(Span::styled("Direct Messages", self.style.dm_style));

        TreeItem::new(TreeNodeId::DirectMessages, dm_text, dm_children)
            .expect("DM node should have unique children")
    }

    fn build_guild_node(&self, guild: &Guild) -> TreeItem<'static, TreeNodeId> {
        let is_active = self.data.active_guild_id() == Some(guild.id());
        let guild_style = if is_active {
            self.style.active_guild_style
        } else if guild.has_unread() {
            self.style.guild_unread_style
        } else {
            self.style.guild_style
        };

        let guild_text = Line::from(Span::styled(guild.name().to_string(), guild_style));

        let channel_items = match self.data.channels(guild.id()) {
            Some(channels) => self.build_channel_nodes(channels),
            None => vec![self.build_placeholder_node(guild.id())],
        };

        TreeItem::new(TreeNodeId::Guild(guild.id()), guild_text, channel_items)
            .expect("Guild node should have unique children")
    }

    fn build_placeholder_node(&self, guild_id: GuildId) -> TreeItem<'static, TreeNodeId> {
        let text = Line::from(Span::styled("Loading...", self.style.placeholder_style));
        TreeItem::new_leaf(TreeNodeId::Placeholder(guild_id), text)
    }

    fn build_channel_nodes(&self, channels: &[Channel]) -> Vec<TreeItem<'static, TreeNodeId>> {
        let mut result = Vec::new();
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
        for channel in orphan_channels {
            if let Some(item) = self.build_channel_leaf(channel) {
                result.push(item);
            }
        }

        category_channels.sort_by_key(|c| c.position());
        for category in category_channels {
            let children_channels = categories.get(&category.id()).cloned().unwrap_or_default();
            result.push(self.build_category_node(category, &children_channels));
        }

        result
    }

    fn build_category_node(
        &self,
        category: &Channel,
        children: &[&Channel],
    ) -> TreeItem<'static, TreeNodeId> {
        let text = Line::from(Span::styled(
            category.name().to_uppercase(),
            self.style.category_style,
        ));

        let mut sorted_children: Vec<&Channel> = children.to_vec();
        sorted_children.sort_by_key(|c| c.position());

        let child_items: Vec<TreeItem<'static, TreeNodeId>> = sorted_children
            .iter()
            .filter_map(|ch| self.build_channel_leaf(ch))
            .collect();

        TreeItem::new(TreeNodeId::Category(category.id()), text, child_items)
            .expect("Category should have unique children")
    }

    fn build_channel_leaf(&self, channel: &Channel) -> Option<TreeItem<'static, TreeNodeId>> {
        if !channel.kind().is_text_based() && !channel.kind().is_voice() {
            return None;
        }

        let is_active = self.data.active_channel_id() == Some(channel.id());
        let style = if is_active {
            self.style.active_channel_style
        } else if channel.has_unread() {
            self.style.channel_unread_style
        } else {
            self.style.channel_style
        };

        let text = Line::from(Span::styled(channel.display_name(), style));

        Some(TreeItem::new_leaf(TreeNodeId::Channel(channel.id()), text))
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

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                format!(" {} ", self.title),
                self.style.title_style,
            ));

        let items = self.build_tree_items();

        let tree = Tree::new(&items)
            .expect("Tree items should be valid")
            .block(block)
            .highlight_style(self.style.selected_style);

        StatefulWidget::render(tree, area, buf, state.tree_state_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guilds_tree_state_creation() {
        let state = GuildsTreeState::new();
        assert!(!state.is_focused());
        assert!(state.selected().is_empty());
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

        let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(state.handle_key(key_j).is_none());

        let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert!(state.handle_key(key_k).is_none());
    }

    #[test]
    fn test_all_guilds_included_in_tree_items() {
        let mut data = GuildsTreeData::new();
        let guilds = vec![
            Guild::new(1_u64, "Nix/NixOS (unofficial)"),
            Guild::new(2_u64, "r/Unixporn"),
            Guild::new(3_u64, "RespriteApp"),
            Guild::new(4_u64, "Discordo"),
            Guild::new(5_u64, "Noctalia"),
            Guild::new(6_u64, "L I N U X's TEST SERVER"),
            Guild::new(7_u64, "OpenCode"),
            Guild::new(8_u64, "OpenCode Antigravity Auth"),
        ];

        data.set_guilds(guilds.clone());

        let tree = GuildsTree::new(&data);
        let items = tree.build_tree_items();

        assert_eq!(
            items.len(),
            guilds.len() + 1,
            "Tree should have DM node + all guilds"
        );

        for (i, guild) in guilds.iter().enumerate() {
            let expected_id = TreeNodeId::Guild(guild.id());
            let item = &items[i + 1];
            assert_eq!(
                item.identifier(),
                &expected_id,
                "Guild at index {} should be {}",
                i,
                guild.name()
            );
        }
    }

    #[test]
    fn test_large_guild_list() {
        let mut data = GuildsTreeData::new();
        let guilds: Vec<Guild> = (0u64..100)
            .map(|i| Guild::new(i, format!("Guild {i}")))
            .collect();

        data.set_guilds(guilds.clone());

        let tree = GuildsTree::new(&data);
        let items = tree.build_tree_items();

        assert_eq!(items.len(), 101, "Should have DM node + 100 guilds");
    }
}
