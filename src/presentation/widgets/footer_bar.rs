use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusContext {
    #[default]
    GuildsTree,
    MessagesList,
    MessageInput,
}

impl FocusContext {
    #[must_use]
    pub fn keybindings(self) -> Vec<KeyBinding> {
        match self {
            Self::GuildsTree => vec![
                KeyBinding::global_quit(),
                KeyBinding::global_focus_next(),
                KeyBinding::new("j/k", "NAV"),
                KeyBinding::new("Enter", "SELECT"),
                KeyBinding::new("h/l", "COLLAPSE"),
            ],
            Self::MessagesList => vec![
                KeyBinding::global_quit(),
                KeyBinding::global_focus_next(),
                KeyBinding::new("j/k", "NAV"),
                KeyBinding::new("r", "REPLY"),
                KeyBinding::new("y", "YANK"),
            ],
            Self::MessageInput => vec![
                KeyBinding::global_quit(),
                KeyBinding::global_focus_next(),
                KeyBinding::new("Enter", "SEND"),
                KeyBinding::new("Esc", "CANCEL"),
                KeyBinding::new("C-a", "ATTACH"),
            ],
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::GuildsTree => "GUILDS",
            Self::MessagesList => "MESSAGES",
            Self::MessageInput => "INPUT",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    key: String,
    action: String,
}

impl KeyBinding {
    #[must_use]
    pub fn new(key: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            action: action.into(),
        }
    }

    fn global_quit() -> Self {
        Self::new("C-c", "QUIT")
    }

    fn global_focus_next() -> Self {
        Self::new("C-l/h", "FOCUS")
    }
}

pub struct FooterBarStyle {
    pub background: Style,
    pub key_bracket: Style,
    pub key: Style,
    pub action: Style,
    pub info: Style,
    pub focus_indicator: Style,
}

impl Default for FooterBarStyle {
    fn default() -> Self {
        Self {
            background: Style::default(),
            key_bracket: Style::default().fg(Color::DarkGray),
            key: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            action: Style::default().fg(Color::Gray),
            info: Style::default().fg(Color::DarkGray),
            focus_indicator: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }
}

pub struct FooterBar<'a> {
    keybindings: Vec<KeyBinding>,
    focus_context: Option<FocusContext>,
    right_info: Option<&'a str>,
    style: FooterBarStyle,
}

impl<'a> FooterBar<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            keybindings: Vec::new(),
            focus_context: None,
            right_info: None,
            style: FooterBarStyle::default(),
        }
    }

    #[must_use]
    pub fn keybindings(mut self, bindings: Vec<KeyBinding>) -> Self {
        self.keybindings = bindings;
        self
    }

    #[must_use]
    pub fn focus_context(mut self, context: FocusContext) -> Self {
        self.focus_context = Some(context);
        self.keybindings = context.keybindings();
        self
    }

    #[must_use]
    pub const fn right_info(mut self, info: Option<&'a str>) -> Self {
        self.right_info = info;
        self
    }

    #[must_use]
    pub const fn style(mut self, style: FooterBarStyle) -> Self {
        self.style = style;
        self
    }

    fn build_left_spans(&self) -> Vec<Span<'_>> {
        let mut spans = Vec::new();

        if let Some(context) = self.focus_context {
            spans.push(Span::styled(
                context.display_name(),
                self.style.focus_indicator,
            ));
            spans.push(Span::raw("  "));
        }

        for (i, binding) in self.keybindings.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled("[", self.style.key_bracket));
            spans.push(Span::styled(&binding.key, self.style.key));
            spans.push(Span::styled("]", self.style.key_bracket));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(&binding.action, self.style.action));
        }

        spans
    }
}

impl Default for FooterBar<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for FooterBar<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        for x in area.left()..area.right() {
            buf[(x, area.y)]
                .set_char(' ')
                .set_style(self.style.background);
        }

        let left_spans = self.build_left_spans();
        let left_line = Line::from(left_spans);
        let left_para = Paragraph::new(left_line);
        let left_area = Rect::new(area.x, area.y, area.width.saturating_sub(30), 1);
        left_para.render(left_area, buf);

        if let Some(info) = self.right_info {
            let right_spans = vec![Span::styled(info, self.style.info)];
            let right_line = Line::from(right_spans);
            let right_width = info.len() as u16;

            if right_width < area.width {
                let right_x = area.right().saturating_sub(right_width);
                let right_area = Rect::new(right_x, area.y, right_width, 1);
                let right_para = Paragraph::new(right_line);
                right_para.render(right_area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybinding_creation() {
        let binding = KeyBinding::new("F1", "HELP");
        assert_eq!(binding.key, "F1");
        assert_eq!(binding.action, "HELP");
    }

    #[test]
    fn test_focus_context_keybindings() {
        let guilds_bindings = FocusContext::GuildsTree.keybindings();
        assert!(!guilds_bindings.is_empty());

        let messages_bindings = FocusContext::MessagesList.keybindings();
        assert!(!messages_bindings.is_empty());
    }

    #[test]
    fn test_footer_bar_with_context() {
        let footer = FooterBar::new().focus_context(FocusContext::GuildsTree);
        assert!(!footer.keybindings.is_empty());
    }

    #[test]
    fn test_footer_bar_with_info() {
        let footer = FooterBar::new().right_info(Some("UTF-8"));
        assert_eq!(footer.right_info, Some("UTF-8"));
    }
}
