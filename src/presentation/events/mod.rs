//! Event handling.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

/// Result of event handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventResult {
    /// Continue processing.
    Continue,
    /// Exit application.
    Exit,
    /// Event was consumed.
    Consumed,
    /// Event was ignored.
    Ignored,
    /// Open external editor for message input.
    OpenEditor {
        initial_content: String,
        message_id: Option<crate::domain::entities::MessageId>,
    },
}

/// Terminal event handler.
pub struct EventHandler {
    poll_timeout: Duration,
}

impl EventHandler {
    const DEFAULT_POLL_TIMEOUT_MS: u64 = 100;

    /// Creates new handler with default timeout.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            poll_timeout: Duration::from_millis(Self::DEFAULT_POLL_TIMEOUT_MS),
        }
    }

    /// Creates handler with custom timeout.
    #[must_use]
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self {
            poll_timeout: timeout,
        }
    }

    /// Polls for events.
    ///
    /// # Errors
    /// Returns IO error if polling fails.
    pub fn poll(&self) -> std::io::Result<Option<Event>> {
        if event::poll(self.poll_timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    /// Checks if key is a quit event.
    #[must_use]
    pub const fn is_quit_event(key: &KeyEvent) -> bool {
        matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } | KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            }
        )
    }

    /// Checks if key is a submit event.
    #[must_use]
    pub const fn is_submit_event(key: &KeyEvent) -> bool {
        matches!(
            key,
            KeyEvent {
                code: KeyCode::Enter,
                ..
            }
        )
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn make_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
    }

    #[test]
    fn test_quit_events() {
        assert!(EventHandler::is_quit_event(&make_key_event(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
        assert!(EventHandler::is_quit_event(&make_key_event(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(EventHandler::is_quit_event(&make_key_event(
            KeyCode::Esc,
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_non_quit_events() {
        assert!(!EventHandler::is_quit_event(&make_key_event(
            KeyCode::Char('a'),
            KeyModifiers::NONE
        )));
        assert!(!EventHandler::is_quit_event(&make_key_event(
            KeyCode::Enter,
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn test_submit_event() {
        assert!(EventHandler::is_submit_event(&make_key_event(
            KeyCode::Enter,
            KeyModifiers::NONE
        )));
        assert!(!EventHandler::is_submit_event(&make_key_event(
            KeyCode::Char('a'),
            KeyModifiers::NONE
        )));
    }
}
