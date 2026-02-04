use crate::domain::search::{SearchKind, SearchResult};
use crate::presentation::theme::Theme;
use crate::presentation::ui::utils::clean_text;
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
}

impl Default for QuickSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickSwitcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: String::new(),
            results: Vec::new(),
            list_state: ListState::default(),
        }
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.results.clear();
        self.list_state.select(None);
    }

    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        if self.results.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
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

pub enum QuickSwitcherAction {
    None,
    Close,
    Select(SearchResult),
    UpdateSearch(String),
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

                let name = clean_text(&res.name);

                let left_part_1 = format!(" {:<9} ", type_label);
                let left_part_2 = format!(" {icon} ");
                let left_part_3 = format!(" {name} ");

                let left_len = left_part_1.len() + left_part_2.chars().count() + left_part_3.len();

                let mut spans = vec![
                    Span::styled(left_part_1, Style::default().fg(Color::DarkGray)),
                    Span::styled(left_part_2, Style::default().fg(self.theme.accent)),
                    Span::styled(left_part_3, Style::default().fg(Color::White)),
                ];

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
            ("*", "Search servers"),
            ("#", "Search channels"),
            ("!", "Search voice channels"),
            ("@", "Search users"),
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
    use crate::domain::search::SearchKind;

    #[test]
    fn test_quick_switcher_initialization() {
        let switcher = QuickSwitcher::new();
        assert!(switcher.input.is_empty());
        assert!(switcher.results.is_empty());
        assert!(switcher.list_state.selected().is_none());
    }

    #[test]
    fn test_quick_switcher_set_results() {
        let mut switcher = QuickSwitcher::new();
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

    #[test]
    fn test_quick_switcher_input() {
        let mut switcher = QuickSwitcher::new();

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Char('a')));
        if let QuickSwitcherAction::UpdateSearch(s) = action {
            assert_eq!(s, "a");
        } else {
            panic!("Expected UpdateSearch");
        }
        assert_eq!(switcher.input, "a");

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Char('b')));
        if let QuickSwitcherAction::UpdateSearch(s) = action {
            assert_eq!(s, "ab");
        } else {
            panic!("Expected UpdateSearch");
        }

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Backspace));
        if let QuickSwitcherAction::UpdateSearch(s) = action {
            assert_eq!(s, "a");
        } else {
            panic!("Expected UpdateSearch");
        }
    }

    #[test]
    fn test_quick_switcher_navigation() {
        let mut switcher = QuickSwitcher::new();
        let results = vec![
            SearchResult::new("1", "1", SearchKind::Channel),
            SearchResult::new("2", "2", SearchKind::Channel),
            SearchResult::new("3", "3", SearchKind::Channel),
        ];
        switcher.set_results(results);

        assert_eq!(switcher.list_state.selected(), Some(0));

        switcher.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(switcher.list_state.selected(), Some(1));

        switcher.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(switcher.list_state.selected(), Some(2));

        switcher.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(switcher.list_state.selected(), Some(0));

        switcher.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(switcher.list_state.selected(), Some(2));
    }

    #[test]
    fn test_quick_switcher_select() {
        let mut switcher = QuickSwitcher::new();
        let results = vec![SearchResult::new("1", "1", SearchKind::Channel)];
        switcher.set_results(results);

        let action = switcher.handle_key(KeyEvent::from(KeyCode::Enter));
        if let QuickSwitcherAction::Select(res) = action {
            assert_eq!(res.id, "1");
        } else {
            panic!("Expected Select");
        }
    }
}
