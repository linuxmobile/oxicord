use crate::domain::keybinding::Keybind;
use crate::presentation::theme::Theme;
use crossterm::event::{KeyCode, KeyModifiers};
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
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::GuildsTree => "GUILDS",
            Self::MessagesList => "MESSAGES",
            Self::MessageInput => "INPUT",
        }
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

impl FooterBarStyle {
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            key: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            focus_indicator: Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            ..Self::default()
        }
    }
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
    keybindings: &'a [Keybind],
    focus_context: Option<FocusContext>,
    right_info: Option<&'a str>,
    style: FooterBarStyle,
}

impl<'a> FooterBar<'a> {
    #[must_use]
    pub fn new(keybindings: &'a [Keybind]) -> Self {
        Self {
            keybindings,
            focus_context: None,
            right_info: None,
            style: FooterBarStyle::default(),
        }
    }

    #[must_use]
    pub fn focus_context(mut self, context: FocusContext) -> Self {
        self.focus_context = Some(context);
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

    fn format_key(key: &crossterm::event::KeyEvent) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            s.push_str("C-");
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            s.push_str("A-");
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) && !matches!(key.code, KeyCode::Char(_)) {
            s.push_str("S-");
        }

        match key.code {
            KeyCode::Char(c) => s.push(c),
            KeyCode::Enter => s.push_str("Enter"),
            KeyCode::Esc => s.push_str("Esc"),
            KeyCode::Tab => s.push_str("Tab"),
            KeyCode::Backspace => s.push_str("Bksp"),
            KeyCode::Up => s.push('↑'),
            KeyCode::Down => s.push('↓'),
            KeyCode::Left => s.push('←'),
            KeyCode::Right => s.push('→'),
            KeyCode::F(n) => {
                let _ = write!(s, "F{n}");
            }
            _ => {
                let _ = write!(s, "{:?}", key.code);
            }
        }
        s
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

        for (i, binding) in self
            .keybindings
            .iter()
            .filter(|k| k.visible_in_bar)
            .enumerate()
        {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled("[", self.style.key_bracket));
            spans.push(Span::styled(Self::format_key(&binding.key), self.style.key));
            spans.push(Span::styled("]", self.style.key_bracket));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(binding.label.as_ref(), self.style.action));
        }

        spans
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
        let right_width = self.right_info.map_or(0, |s| s.len() as u16);
        let left_width = area.width.saturating_sub(right_width + 1);

        let left_area = Rect::new(area.x, area.y, left_width, 1);
        left_para.render(left_area, buf);

        if let Some(info) = self.right_info {
            let right_spans = vec![Span::styled(info, self.style.info)];
            let right_line = Line::from(right_spans);

            if right_width < area.width {
                let right_x = area.right().saturating_sub(right_width);
                let right_area = Rect::new(right_x, area.y, right_width, 1);
                let right_para = Paragraph::new(right_line);
                right_para.render(right_area, buf);
            }
        }
    }
}
