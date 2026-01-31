use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxicord::domain::entities::{User, UserCache};
use oxicord::presentation::services::markdown_renderer::MarkdownRenderer;
use oxicord::presentation::theme::Theme;
use oxicord::presentation::ui::{ChatFocus, ChatScreenState};
use std::sync::Arc;

fn create_test_user() -> User {
    User::new("123", "testuser", "0", None, false, None)
}

#[test]
fn test_backtab_reverse_navigation() {
    let mut state = ChatScreenState::new(
        create_test_user(),
        Arc::new(MarkdownRenderer::new()),
        UserCache::new(),
        false,
        true,
        Theme::new("Orange"),
    );

    assert_eq!(state.focus(), ChatFocus::GuildsTree);

    state.focus_next();
    state.focus_next();
    assert_eq!(state.focus(), ChatFocus::MessageInput);

    let backtab_event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    state.handle_key(backtab_event);

    assert_eq!(
        state.focus(),
        ChatFocus::MessagesList,
        "BackTab should move focus to MessagesList"
    );
}

#[test]
fn test_shift_tab_reverse_navigation() {
    let mut state = ChatScreenState::new(
        create_test_user(),
        Arc::new(MarkdownRenderer::new()),
        UserCache::new(),
        false,
        true,
        Theme::new("Orange"),
    );

    assert_eq!(state.focus(), ChatFocus::GuildsTree);

    state.focus_next();
    state.focus_next();
    assert_eq!(state.focus(), ChatFocus::MessageInput);

    let shift_tab_event = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
    state.handle_key(shift_tab_event);

    assert_eq!(
        state.focus(),
        ChatFocus::MessagesList,
        "Shift+Tab should move focus to MessagesList"
    );
}
