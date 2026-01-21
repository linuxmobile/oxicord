use crate::domain::ConnectionStatus;
use crate::presentation::theme::Theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

trait ConnectionStatusExt {
    fn display_text(self) -> &'static str;
    fn indicator(self) -> &'static str;
}

impl ConnectionStatusExt for ConnectionStatus {
    fn display_text(self) -> &'static str {
        match self {
            Self::Disconnected => "DISCONNECTED",
            Self::Connecting => "CONNECTING",
            Self::Connected => "CONNECTED",
            Self::Reconnecting => "RECONNECTING",
            Self::Error => "ERROR",
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            Self::Connected => "●",
            Self::Connecting | Self::Reconnecting => "◐",
            Self::Disconnected | Self::Error => "○",
        }
    }
}

pub struct HeaderBarStyle {
    pub background: Style,
    pub app_name: Style,
    pub version: Style,
    pub status_connected: Style,
    pub status_disconnected: Style,
    pub status_connecting: Style,
    pub status_error: Style,
}

impl HeaderBarStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            app_name: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            ..Self::default()
        }
    }
}

impl Default for HeaderBarStyle {
    fn default() -> Self {
        Self {
            background: Style::default(),
            app_name: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            version: Style::default().fg(Color::DarkGray),
            status_connected: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            status_disconnected: Style::default().fg(Color::Red),
            status_connecting: Style::default().fg(Color::Yellow),
            status_error: Style::default().fg(Color::Red),
        }
    }
}

pub struct HeaderBar<'a> {
    app_name: &'a str,
    version: &'a str,
    connection_status: ConnectionStatus,
    style: HeaderBarStyle,
}

impl<'a> HeaderBar<'a> {
    #[must_use]
    pub fn new(app_name: &'a str, version: &'a str) -> Self {
        Self {
            app_name,
            version,
            connection_status: ConnectionStatus::default(),
            style: HeaderBarStyle::default(),
        }
    }

    #[must_use]
    pub const fn connection_status(mut self, status: ConnectionStatus) -> Self {
        self.connection_status = status;
        self
    }

    #[must_use]
    pub const fn style(mut self, style: HeaderBarStyle) -> Self {
        self.style = style;
        self
    }

    const fn status_style(&self) -> Style {
        match self.connection_status {
            ConnectionStatus::Connected => self.style.status_connected,
            ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => {
                self.style.status_connecting
            }
            ConnectionStatus::Disconnected => self.style.status_disconnected,
            ConnectionStatus::Error => self.style.status_error,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn build_status_spans(&self) -> (Vec<Span<'static>>, u16) {
        let status_style = self.status_style();
        let indicator = self.connection_status.indicator().to_string();
        let status_text = self.connection_status.display_text().to_string();

        let spans = vec![
            Span::styled(indicator.clone(), status_style),
            Span::raw(" "),
            Span::styled(status_text.clone(), status_style),
        ];

        let width = (indicator.chars().count() + 1 + status_text.chars().count()) as u16;
        (spans, width)
    }
}

impl Widget for HeaderBar<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        for x in area.left()..area.right() {
            buf[(x, area.y)]
                .set_char(' ')
                .set_style(self.style.background);
        }

        let left_spans = vec![
            Span::styled(self.app_name.to_uppercase(), self.style.app_name),
            Span::raw("  "),
            Span::styled(format!("v{}", self.version), self.style.version),
        ];

        let left_line = Line::from(left_spans);
        let left_width = (self.app_name.len() + 2 + self.version.len() + 1) as u16;
        let left_area = Rect::new(area.x, area.y, left_width.min(area.width), 1);
        Paragraph::new(left_line).render(left_area, buf);

        let (status_spans, status_width) = self.build_status_spans();

        if status_width < area.width.saturating_sub(left_width) {
            let right_x = area.right().saturating_sub(status_width);
            let right_area = Rect::new(right_x, area.y, status_width, 1);
            let right_line = Line::from(status_spans);
            Paragraph::new(right_line).render(right_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Connected.display_text(), "CONNECTED");
        assert_eq!(
            ConnectionStatus::Disconnected.display_text(),
            "DISCONNECTED"
        );
        assert_eq!(ConnectionStatus::Connecting.display_text(), "CONNECTING");
    }

    #[test]
    fn test_connection_status_indicator() {
        assert_eq!(ConnectionStatus::Connected.indicator(), "●");
        assert_eq!(ConnectionStatus::Disconnected.indicator(), "○");
    }

    #[test]
    fn test_header_bar_creation() {
        let header =
            HeaderBar::new("oxicord", "0.0.1").connection_status(ConnectionStatus::Connected);

        assert_eq!(header.app_name, "oxicord");
        assert_eq!(header.version, "0.0.1");
    }
}
