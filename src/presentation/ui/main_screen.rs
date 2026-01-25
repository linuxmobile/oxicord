//! Main screen after login.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::domain::entities::User;
use crate::presentation::widgets::{StatusBar, StatusLevel};

/// Main application screen.
pub struct MainScreen {
    user: User,
    status: StatusBar,
    use_display_name: bool,
}

impl MainScreen {
    /// Creates new main screen.
    #[must_use]
    pub fn new(user: User) -> Self {
        let status = StatusBar::new()
            .left(format!("Logged in as: {}", user.display_name()))
            .right("Press 'q' to quit")
            .level(StatusLevel::Success);

        Self {
            user,
            status,
            use_display_name: false,
        }
    }

    pub fn set_use_display_name(&mut self, enabled: bool) {
        use crate::application::services::identity_service::IdentityService;
        self.use_display_name = enabled;
        let name = IdentityService::get_preferred_name(&self.user, enabled);
        self.status = self.status.clone().left(format!("Logged in as: {name}"));
    }

    /// Returns current user.
    #[must_use]
    pub const fn user(&self) -> &User {
        &self.user
    }

    /// Sets status bar.
    pub fn set_status(&mut self, status: StatusBar) {
        self.status = status;
    }
}

impl Widget for &MainScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);
        let [content_area, status_area] = layout.areas(area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Oxicord ");

        let inner = block.inner(content_area);
        block.render(content_area, buf);

        let welcome_layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(5),
            Constraint::Fill(1),
        ]);
        let [_, center, _] = welcome_layout.areas(inner);

        let horizontal = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Min(40),
            Constraint::Fill(1),
        ]);
        let [_, message_area, _] = horizontal.areas(center);

        let lines = vec![
            Line::from(vec![Span::styled(
                "Welcome to Oxicord!",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Logged in as: "),
                Span::styled(
                    crate::application::services::identity_service::IdentityService::get_preferred_name(
                        &self.user,
                        self.use_display_name,
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("User ID: "),
                Span::styled(self.user.id_str(), Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "The full Discord client is under development...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            )),
        ];

        let paragraph = Paragraph::new(lines);
        paragraph.render(message_area, buf);

        (&self.status).render(status_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_screen_creation() {
        let user = User::new("123", "testuser", "0", None, false, None);
        let screen = MainScreen::new(user);

        assert_eq!(screen.user().username(), "testuser");
    }
}
