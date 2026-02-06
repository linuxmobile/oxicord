use crate::domain::search::{RecentItem, SearchKind, SearchResult};
use crate::infrastructure::config::app_config::QuickSwitcherSortMode;
use crate::presentation::theme::Theme;
use crate::presentation::ui::utils::sanitize_channel_name;
use crate::presentation::widgets::FooterBarStyle;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget,
    },
};

pub struct QuickSwitcher {
    pub input: String,
    pub results: Vec<SearchResult>,
    pub list_state: ListState,
    pub sort_mode: QuickSwitcherSortMode,
    pub recents: Vec<RecentItem>,
}

impl Default for QuickSwitcher {
    fn default() -> Self {
        Self::new(QuickSwitcherSortMode::default())
    }
}

impl QuickSwitcher {
    #[must_use]
    pub fn new(sort_mode: QuickSwitcherSortMode) -> Self {
        Self {
            input: String::new(),
            results: Vec::new(),
            list_state: ListState::default(),
            sort_mode,
            recents: Vec::new(),
        }
    }

    pub fn set_sort_mode(&mut self, mode: QuickSwitcherSortMode) {
        self.sort_mode = mode;
        self.apply_sort();
    }

    pub fn toggle_sort_mode(&mut self) {
        self.sort_mode = match self.sort_mode {
            QuickSwitcherSortMode::Recents => QuickSwitcherSortMode::Mixed,
            QuickSwitcherSortMode::Mixed => QuickSwitcherSortMode::Recents,
        };
        self.apply_sort();
    }

    pub fn set_recents(&mut self, recents: Vec<RecentItem>) {
        self.recents = recents;
        self.apply_sort();
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.results.clear();
        self.list_state.select(None);
    }

    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        self.apply_sort();

        if self.results.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn apply_sort(&mut self) {
        tracing::debug!(
            "Applying sort: {:?} with {} recents",
            self.sort_mode,
            self.recents.len()
        );

        let get_timestamp = |res: &SearchResult, recents: &[RecentItem]| -> i64 {
            let ts = recents
                .iter()
                .find(|r| r.id == res.id && r.kind == res.kind)
                .map_or(0, |r| r.timestamp);

            if ts > 0 {
                tracing::debug!("Matched recent: {} ({:?}) -> {}", res.name, res.kind, ts);
            }
            ts
        };

        match self.sort_mode {
            QuickSwitcherSortMode::Recents => {
                self.results.sort_by(|a, b| {
                    let time_a = get_timestamp(a, &self.recents);
                    let time_b = get_timestamp(b, &self.recents);
                    time_b
                        .cmp(&time_a)
                        .then_with(|| a.score.cmp(&b.score).reverse())
                });
            }
            QuickSwitcherSortMode::Mixed => {
                let mut results_with_time: Vec<(SearchResult, i64)> = self
                    .results
                    .iter()
                    .map(|r| (r.clone(), get_timestamp(r, &self.recents)))
                    .collect();

                results_with_time.sort_by(|a, b| b.1.cmp(&a.1));

                let top_recents_ids: Vec<(String, SearchKind)> = results_with_time
                    .iter()
                    .filter(|(_, t)| *t > 0)
                    .take(3)
                    .map(|(r, _)| (r.id.clone(), r.kind.clone()))
                    .collect();

                self.results.sort_by(|a, b| {
                    let a_is_top = top_recents_ids.contains(&(a.id.clone(), a.kind.clone()));
                    let b_is_top = top_recents_ids.contains(&(b.id.clone(), b.kind.clone()));

                    if a_is_top && b_is_top {
                        let time_a = get_timestamp(a, &self.recents);
                        let time_b = get_timestamp(b, &self.recents);
                        time_b.cmp(&time_a)
                    } else if a_is_top {
                        std::cmp::Ordering::Less
                    } else if b_is_top {
                        std::cmp::Ordering::Greater
                    } else {
                        let kind_priority = |k: &SearchKind| match k {
                            SearchKind::DM => 0,
                            SearchKind::Guild => 1,
                            _ => 2,
                        };

                        let prio_a = kind_priority(&a.kind);
                        let prio_b = kind_priority(&b.kind);

                        if prio_a != prio_b {
                            return prio_a.cmp(&prio_b);
                        }

                        match b.score.cmp(&a.score) {
                            std::cmp::Ordering::Equal => a.name.cmp(&b.name),
                            other => other,
                        }
                    }
                });
            }
        }
    }

    #[must_use]
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.list_state.selected().and_then(|i| self.results.get(i))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> QuickSwitcherAction {
        match key.code {
            KeyCode::Esc => QuickSwitcherAction::Close,
            KeyCode::Enter => {
                if let Some(result) = self.selected_result() {
                    QuickSwitcherAction::Select(result.clone())
                } else {
                    QuickSwitcherAction::None
                }
            }
            KeyCode::Up => {
                self.select_previous();
                QuickSwitcherAction::None
            }
            KeyCode::Down => {
                self.select_next();
                QuickSwitcherAction::None
            }
            KeyCode::Tab => QuickSwitcherAction::ToggleSortMode,
            KeyCode::Char('h' | 'w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(last_space_idx) = self.input.trim_end().rfind(' ') {
                    self.input.truncate(last_space_idx + 1);
                } else {
                    self.input.clear();
                }
                QuickSwitcherAction::UpdateSearch(self.input.clone())
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                QuickSwitcherAction::UpdateSearch(self.input.clone())
            }
            KeyCode::Backspace => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    if let Some(last_space_idx) = self.input.trim_end().rfind(' ') {
                        self.input.truncate(last_space_idx + 1);
                    } else {
                        self.input.clear();
                    }
                } else {
                    self.input.pop();
                }
                QuickSwitcherAction::UpdateSearch(self.input.clone())
            }
            _ => QuickSwitcherAction::None,
        }
    }

    pub fn select_next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.results.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        if !self.results.is_empty() {
            self.list_state.select(Some(i));
        }
    }

    pub fn select_previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.results.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        if !self.results.is_empty() {
            self.list_state.select(Some(i));
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum QuickSwitcherAction {
    None,
    Close,
    Select(SearchResult),
    UpdateSearch(String),
    ToggleSortMode,
}

pub struct QuickSwitcherWidget<'a> {
    switcher: &'a QuickSwitcher,
    theme: &'a Theme,
}

impl<'a> QuickSwitcherWidget<'a> {
    #[must_use]
    pub fn new(switcher: &'a QuickSwitcher, theme: &'a Theme) -> Self {
        Self { switcher, theme }
    }

    fn render_results_list(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        Block::default().render(area, buf);

        let list_width = area.width as usize;

        let items: Vec<ListItem> = self
            .switcher
            .results
            .iter()
            .map(|res| {
                let (type_label, icon) = match res.kind {
                    SearchKind::DM => ("(user)", ""),
                    SearchKind::Channel => ("(channel)", "󰆈"),
                    SearchKind::Voice => ("(voice)", "󰕾"),
                    SearchKind::Forum => ("(forum)", ""),
                    SearchKind::Thread => ("(thread)", ""),
                    SearchKind::Guild => ("(server)", ""),
                };

                let name = sanitize_channel_name(&res.name);

                let left_part_1 = format!(" {type_label:<9} ");
                let left_part_2 = format!(" {icon} ");
                let left_part_3 = format!(" {name} ");

                let mut left_len =
                    left_part_1.len() + left_part_2.chars().count() + left_part_3.len();

                let mut spans = vec![
                    Span::styled(left_part_1, Style::default().fg(Color::DarkGray)),
                    Span::styled(left_part_2, Style::default().fg(self.theme.accent)),
                    Span::styled(left_part_3, Style::default().fg(Color::White)),
                ];

                if let Some(parent) = &res.parent_name {
                    let parent_text = format!("({}) ", sanitize_channel_name(parent));
                    left_len += parent_text.len();
                    spans.push(Span::styled(
                        parent_text,
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                if let Some(guild_name) = &res.guild_name {
                    let right_part = format!(" {guild_name} ");
                    let right_len = right_part.len();

                    if list_width > left_len + right_len {
                        let padding = list_width - left_len - right_len;
                        spans.push(Span::raw(" ".repeat(padding)));
                        spans.push(Span::styled(
                            right_part,
                            Style::default().fg(self.theme.accent),
                        ));
                    } else {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            right_part,
                            Style::default().fg(self.theme.accent),
                        ));
                    }
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default().bg(self.theme.selection_style.bg.unwrap_or(Color::DarkGray)),
        );

        let mut state = self.switcher.list_state;
        StatefulWidget::render(list, area, buf, &mut state);
    }
}

impl Widget for QuickSwitcherWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let area = centered_rect(60, 30, area);

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.accent))
            .title(" Quick Switcher ");

        let inner_area = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner_area);

        let search_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(layout[0]);

        let search_label = Paragraph::new(" Search ")
            .style(Style::default().bg(self.theme.accent).fg(Color::Black));
        search_label.render(search_layout[0], buf);

        let input =
            Paragraph::new(self.switcher.input.as_str()).style(Style::default().fg(Color::White));
        input.render(search_layout[2], buf);

        self.render_results_list(layout[1], buf);

        let footer_style = FooterBarStyle::from_theme(self.theme);

        let prefixes = [
            ("*", "Servers"),
            ("#", "Channels"),
            ("!", "Voice"),
            ("@", "Users"),
            ("^", "Threads"),
        ];

        let mut footer_spans = Vec::new();
        for (i, (prefix, label)) in prefixes.iter().enumerate() {
            if i > 0 {
                footer_spans.push(Span::raw(" "));
            }
            footer_spans.push(Span::styled(
                format!(" {prefix} "),
                footer_style.label_style,
            ));
            footer_spans.push(Span::styled(format!(" {label} "), footer_style.key_style));
        }

        let footer = Paragraph::new(Line::from(footer_spans));
        for x in layout[3].left()..layout[3].right() {
            buf[(x, layout[3].y)].set_style(footer_style.background);
        }
        footer.render(layout[3], buf);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::search::{RecentItem, SearchKind};
    use test_case::test_case;

    #[test]
    fn test_quick_switcher_initialization() {
        let switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        assert!(switcher.input.is_empty());
        assert!(switcher.results.is_empty());
        assert!(switcher.list_state.selected().is_none());
        assert_eq!(switcher.sort_mode, QuickSwitcherSortMode::Recents);
    }

    #[test]
    fn test_quick_switcher_set_results() {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        let results = vec![
            SearchResult::new("1", "Channel 1", SearchKind::Channel),
            SearchResult::new("2", "Channel 2", SearchKind::Channel),
        ];

        switcher.set_results(results.clone());
        assert_eq!(switcher.results.len(), 2);
        assert_eq!(switcher.list_state.selected(), Some(0));

        switcher.set_results(vec![]);
        assert!(switcher.results.is_empty());
        assert!(switcher.list_state.selected().is_none());
    }

    #[test_case(KeyCode::Char('a'), "", "a", QuickSwitcherAction::UpdateSearch("a".to_string()) ; "type_char")]
    #[test_case(KeyCode::Backspace, "a", "", QuickSwitcherAction::UpdateSearch("".to_string()) ; "backspace_char")]
    #[test_case(KeyCode::Backspace, "", "", QuickSwitcherAction::UpdateSearch("".to_string()) ; "backspace_empty")]
    #[test_case(KeyCode::Esc, "abc", "abc", QuickSwitcherAction::Close ; "escape_preserves_input")]
    #[test_case(KeyCode::Tab, "abc", "abc", QuickSwitcherAction::ToggleSortMode ; "tab_toggles_mode")]
    fn test_handle_key_input(
        key_code: KeyCode,
        initial_input: &str,
        expected_input: &str,
        expected_action: QuickSwitcherAction,
    ) {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        switcher.input = initial_input.to_string();

        let action = switcher.handle_key(KeyEvent::from(key_code));

        assert_eq!(switcher.input, expected_input);

        match (action, expected_action) {
            (QuickSwitcherAction::UpdateSearch(a), QuickSwitcherAction::UpdateSearch(b)) => {
                assert_eq!(a, b)
            }
            (QuickSwitcherAction::Close, QuickSwitcherAction::Close) => {}
            (QuickSwitcherAction::ToggleSortMode, QuickSwitcherAction::ToggleSortMode) => {}
            (a, b) => panic!("Action mismatch: got {:?}, expected {:?}", a, b),
        }
    }

    #[test_case(KeyCode::Down, Some(0), Some(1) ; "down_from_0")]
    #[test_case(KeyCode::Down, Some(1), Some(2) ; "down_from_1")]
    #[test_case(KeyCode::Down, Some(2), Some(0) ; "down_wrap_around")]
    #[test_case(KeyCode::Up, Some(0), Some(2) ; "up_wrap_around")]
    #[test_case(KeyCode::Up, Some(2), Some(1) ; "up_from_2")]
    #[test_case(KeyCode::Up, Some(1), Some(0) ; "up_from_1")]
    fn test_navigation(key_code: KeyCode, start_idx: Option<usize>, expected_idx: Option<usize>) {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        let results = vec![
            SearchResult::new("1", "1", SearchKind::Channel),
            SearchResult::new("2", "2", SearchKind::Channel),
            SearchResult::new("3", "3", SearchKind::Channel),
        ];
        switcher.set_results(results);
        switcher.list_state.select(start_idx);

        switcher.handle_key(KeyEvent::from(key_code));

        assert_eq!(switcher.list_state.selected(), expected_idx);
    }

    #[test]
    fn test_select_action() {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        let results = vec![SearchResult::new("1", "1", SearchKind::Channel)];
        switcher.set_results(results);
        switcher.list_state.select(Some(0));

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Enter));

        if let QuickSwitcherAction::Select(res) = action {
            assert_eq!(res.id, "1");
        } else {
            panic!("Expected Select action");
        }
    }

    #[test]
    fn test_select_none() {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::default());
        let results = vec![SearchResult::new("1", "1", SearchKind::Channel)];
        switcher.set_results(results);
        switcher.list_state.select(None);

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Enter));

        assert_eq!(action, QuickSwitcherAction::None);
    }

    #[test]
    fn test_quick_switcher_sorting_recents() {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::Recents);
        let mut r1 = SearchResult::new("1", "1", SearchKind::Channel);
        r1.score = 10;
        let mut r2 = SearchResult::new("2", "2", SearchKind::Channel);
        r2.score = 20;

        let recents = vec![
            RecentItem {
                id: "1".to_string(),
                name: "1".to_string(),
                kind: SearchKind::Channel,
                guild_id: None,
                timestamp: 100,
            },
            RecentItem {
                id: "2".to_string(),
                name: "2".to_string(),
                kind: SearchKind::Channel,
                guild_id: None,
                timestamp: 50,
            },
        ];
        switcher.set_recents(recents);

        switcher.set_results(vec![r1.clone(), r2.clone()]);

        assert_eq!(switcher.results[0].id, "1");
        assert_eq!(switcher.results[1].id, "2");
    }

    #[test]
    fn test_quick_switcher_sorting_mixed() {
        let mut switcher = QuickSwitcher::new(QuickSwitcherSortMode::Mixed);
        let items: Vec<SearchResult> = (1..=5)
            .map(|i| {
                SearchResult::new(i.to_string(), i.to_string(), SearchKind::Channel)
                    .with_score(i * 10)
            })
            .collect();

        let recents = vec![
            RecentItem {
                id: "1".to_string(),
                name: "1".to_string(),
                kind: SearchKind::Channel,
                guild_id: None,
                timestamp: 300,
            },
            RecentItem {
                id: "2".to_string(),
                name: "2".to_string(),
                kind: SearchKind::Channel,
                guild_id: None,
                timestamp: 200,
            },
            RecentItem {
                id: "3".to_string(),
                name: "3".to_string(),
                kind: SearchKind::Channel,
                guild_id: None,
                timestamp: 100,
            },
        ];
        switcher.set_recents(recents);
        switcher.set_results(items);

        assert_eq!(switcher.results[0].id, "1");
        assert_eq!(switcher.results[1].id, "2");
        assert_eq!(switcher.results[2].id, "3");
        assert_eq!(switcher.results[3].id, "5");
        assert_eq!(switcher.results[4].id, "4");
    }
}
