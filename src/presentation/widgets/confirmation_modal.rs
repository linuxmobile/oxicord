use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::presentation::theme::Theme;

pub struct ConfirmationModal {
    title: String,
    message: String,
    theme: Theme,
}

impl ConfirmationModal {
    #[must_use]
    pub fn new(title: impl Into<String>, message: impl Into<String>, theme: Theme) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            theme,
        }
    }

    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

        Layout::horizontal([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
    }
}

impl Widget for ConfirmationModal {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::centered_rect(40, 20, area);

        Clear.render(area, buf);

        let block = Block::default()
            .title(self.title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.accent));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let layout = Layout::vertical([Constraint::Min(2), Constraint::Length(1)])
            .margin(1)
            .split(inner_area);

        let message = Paragraph::new(self.message)
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });

        message.render(layout[0], buf);

        let buttons = Line::from(vec![
            Span::styled(
                "Confirm (Enter)",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Cancel (Esc)", Style::default().fg(Color::Gray)),
        ]);

        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        buttons_para.render(layout[1], buf);
    }
}
