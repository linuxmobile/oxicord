//! Text input widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};
use zeroize::Zeroize;

/// Text input field widget.
#[derive(Debug, Clone)]
pub struct TextInput {
    value: String,
    cursor: usize,
    focused: bool,
    masked: bool,
    placeholder: String,
    label: String,
}

impl Drop for TextInput {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

impl TextInput {
    /// Creates new input with label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            focused: false,
            masked: false,
            placeholder: String::new(),
            label: label.into(),
        }
    }

    /// Enables password masking.
    #[must_use]
    pub const fn password(mut self) -> Self {
        self.masked = true;
        self
    }

    /// Sets placeholder text.
    #[must_use]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Sets focus state.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns focus state.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Returns current value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Sets value.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.len();
    }

    /// Clears value.
    pub fn clear(&mut self) {
        self.value.zeroize();
        self.value.clear();
        self.cursor = 0;
    }

    /// Inserts character at cursor.
    pub fn input_char(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Inserts string at cursor.
    pub fn insert_str(&mut self, text: &str) {
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    /// Deletes character before cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.value.remove(self.cursor);
        }
    }

    /// Deletes character at cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    /// Moves cursor left.
    pub const fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Moves cursor right.
    pub const fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor += 1;
        }
    }

    /// Moves cursor to start.
    pub const fn move_start(&mut self) {
        self.cursor = 0;
    }

    /// Moves cursor to end.
    pub const fn move_end(&mut self) {
        self.cursor = self.value.len();
    }

    fn display_text(&self) -> String {
        if self.value.is_empty() {
            self.placeholder.clone()
        } else if self.masked {
            "•".repeat(self.value.len())
        } else {
            self.value.clone()
        }
    }
}

impl Widget for &TextInput {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };

        let text_style = if self.value.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(self.label.as_str());

        let inner = block.inner(area);

        let display = self.display_text();
        let paragraph = Paragraph::new(display).style(text_style);

        block.render(area, buf);
        paragraph.render(inner, buf);

        if self.focused && inner.width > 0 {
            #[allow(clippy::cast_possible_truncation)]
            let cursor_x = inner.x + self.cursor as u16;
            if cursor_x < inner.x + inner.width {
                buf[(cursor_x, inner.y)]
                    .set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_basic() {
        let mut input = TextInput::new("Test");
        assert!(input.value().is_empty());

        input.input_char('a');
        input.input_char('b');
        assert_eq!(input.value(), "ab");

        input.backspace();
        assert_eq!(input.value(), "a");
    }

    #[test]
    fn test_masked_display() {
        let mut input = TextInput::new("Password").password();
        input.set_value("secret");

        assert_eq!(input.display_text(), "••••••");
    }
}
