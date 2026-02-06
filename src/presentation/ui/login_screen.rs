//! Login screen.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::presentation::theme::Theme;
use crate::presentation::widgets::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
    Input,
    Validating,
    Error,
    Success,
}

/// Login screen UI.
pub struct LoginScreen {
    token_input: TextInput,
    state: LoginState,
    error_message: Option<String>,
    persist_token: bool,
    theme: Theme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginAction {
    None,
    Submit,
    DeleteToken,
    Paste,
}

impl LoginScreen {
    /// Creates new login screen.
    #[must_use]
    pub fn new() -> Self {
        let mut token_input = TextInput::new("Discord Token")
            .password()
            .placeholder("Paste your Discord token here...");
        token_input.set_focused(true);

        Self {
            token_input,
            state: LoginState::Input,
            error_message: None,
            persist_token: true,
            theme: Theme::default(),
        }
    }

    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Returns current state.
    #[must_use]
    pub const fn state(&self) -> LoginState {
        self.state
    }

    /// Returns entered token.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        let value = self.token_input.value();
        if value.is_empty() { None } else { Some(value) }
    }

    /// Returns persistence preference.
    #[must_use]
    pub const fn should_persist(&self) -> bool {
        self.persist_token
    }

    /// Sets validating state.
    pub fn set_validating(&mut self) {
        self.state = LoginState::Validating;
        self.error_message = None;
    }

    /// Sets success state.
    pub fn set_success(&mut self) {
        self.state = LoginState::Success;
        self.error_message = None;
    }

    /// Sets error state.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.state = LoginState::Error;
        self.error_message = Some(message.into());
    }

    /// Resets to input state.
    pub fn reset(&mut self) {
        self.state = LoginState::Input;
        self.error_message = None;
    }

    /// Sets the token value.
    pub fn set_token(&mut self, token: String) {
        self.token_input.set_value(token);
    }

    /// Inserts text at the current cursor position.
    pub fn paste_token(&mut self, text: &str) {
        self.token_input.insert_str(text);
    }

    /// Handles key event, returns action.
    pub fn handle_key(&mut self, key: KeyEvent) -> LoginAction {
        if self.state == LoginState::Validating {
            return LoginAction::None;
        }

        if self.state == LoginState::Error {
            self.reset();
            return LoginAction::None;
        }

        match key.code {
            KeyCode::Enter => {
                if !self.token_input.value().is_empty() {
                    return LoginAction::Submit;
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) => {
                return LoginAction::DeleteToken;
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.token_input.clear();
            }
            KeyCode::Char('v')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return LoginAction::Paste;
            }
            KeyCode::Char(c) => {
                self.token_input.input_char(c);
            }
            KeyCode::Backspace => {
                self.token_input.backspace();
            }
            KeyCode::Delete => {
                self.token_input.delete();
            }
            KeyCode::Left => {
                self.token_input.move_left();
            }
            KeyCode::Right => {
                self.token_input.move_right();
            }
            KeyCode::Home => {
                self.token_input.move_start();
            }
            KeyCode::End => {
                self.token_input.move_end();
            }
            KeyCode::Tab => {
                self.persist_token = !self.persist_token;
            }
            _ => {}
        }

        LoginAction::None
    }

    fn render_inner(&self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Min(15),
            Constraint::Fill(1),
        ]);
        let [_, center, _] = vertical.areas(area);

        let horizontal = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Min(60),
            Constraint::Fill(1),
        ]);
        let [_, content_area, _] = horizontal.areas(center);

        Clear.render(content_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.accent))
            .title(" Oxicord Login ");

        let inner = block.inner(content_area);
        block.render(content_area, buf);

        let mut constraints = vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ];

        let token_val = self.token_input.value();
        let has_quote = token_val.contains('"') || token_val.contains('\'');
        let too_long = token_val.len() > 72;
        let show_tip = has_quote || too_long;

        if show_tip {
            constraints.push(Constraint::Length(2));
        } else {
            constraints.push(Constraint::Length(1));
        }

        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Min(2));

        let inner_layout = Layout::vertical(constraints);
        let areas = inner_layout.split(inner);

        let title =
            Paragraph::new("Enter your Discord token to login").style(self.theme.base_style);
        title.render(areas[0], buf);

        (&self.token_input).render(areas[2], buf);

        if show_tip {
            let mut tips = Vec::new();
            if has_quote {
                tips.push("Token should not contain quotes");
            }
            if too_long {
                tips.push("Token seems too long");
            }
            let tip_text = format!("Tip! {}", tips.join(", "));
            let tip_para = Paragraph::new(tip_text)
                .style(self.theme.warning_style.add_modifier(Modifier::BOLD));
            tip_para.render(areas[3], buf);
        }

        let checkbox = if self.persist_token { "[x]" } else { "[ ]" };
        let persist_line = Line::from(vec![
            Span::styled(checkbox, Style::default().fg(self.theme.accent)),
            Span::raw(" Remember token (Tab to toggle)"),
        ]);
        let persist_para = Paragraph::new(persist_line);
        persist_para.render(areas[4], buf);

        let status = match self.state {
            LoginState::Input => Line::from(vec![
                Span::styled("Enter: Login", self.theme.dimmed_style),
                Span::raw(" | "),
                Span::styled("Esc: Quit", self.theme.dimmed_style),
                Span::raw(" | "),
                Span::styled("Alt+D: Clear Saved", self.theme.dimmed_style),
            ]),
            LoginState::Validating => Line::from(Span::styled(
                "Validating token...",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::ITALIC),
            )),
            LoginState::Error => {
                let msg = self.error_message.as_deref().unwrap_or("Unknown error");
                Line::from(Span::styled(
                    format!("Error: {msg}"),
                    self.theme.error_style,
                ))
            }
            LoginState::Success => {
                Line::from(Span::styled("Login successful!", self.theme.success_style))
            }
        };

        let status_para = Paragraph::new(status).wrap(ratatui::widgets::Wrap { trim: true });
        status_para.render(areas[6], buf);
    }
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for &LoginScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_inner(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_initial_state() {
        let screen = LoginScreen::new();
        assert_eq!(screen.state(), LoginState::Input);
        assert!(screen.token().is_none());
        assert!(screen.should_persist());
    }

    #[test]
    fn test_typing() {
        let mut screen = LoginScreen::new();
        screen.handle_key(key(KeyCode::Char('t')));
        screen.handle_key(key(KeyCode::Char('e')));
        screen.handle_key(key(KeyCode::Char('s')));
        screen.handle_key(key(KeyCode::Char('t')));

        assert_eq!(screen.token(), Some("test"));
    }

    #[test]
    fn test_toggle_persist() {
        let mut screen = LoginScreen::new();
        assert!(screen.should_persist());

        screen.handle_key(key(KeyCode::Tab));
        assert!(!screen.should_persist());

        screen.handle_key(key(KeyCode::Tab));
        assert!(screen.should_persist());
    }

    #[test]
    fn test_submit_empty_returns_false() {
        let mut screen = LoginScreen::new();
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), LoginAction::None);
    }

    #[test]
    fn test_submit_with_token_returns_true() {
        let mut screen = LoginScreen::new();
        screen.handle_key(key(KeyCode::Char('x')));
        assert_eq!(screen.handle_key(key(KeyCode::Enter)), LoginAction::Submit);
    }

    #[test]
    fn test_delete_token_action() {
        let mut screen = LoginScreen::new();
        let event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT);
        assert_eq!(screen.handle_key(event), LoginAction::DeleteToken);
    }

    #[test]
    fn test_ctrl_u_clears_input() {
        let mut screen = LoginScreen::new();
        screen.handle_key(key(KeyCode::Char('t')));
        screen.handle_key(key(KeyCode::Char('e')));
        assert_eq!(screen.token(), Some("te"));

        screen.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(screen.token().is_none());
    }

    #[test]
    fn test_ctrl_v_returns_paste_action() {
        let mut screen = LoginScreen::new();
        let action = screen.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL));
        assert_eq!(action, LoginAction::Paste);
    }
}
