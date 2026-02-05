use crate::domain::keybinding::{Action, Keybind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use tracing::warn;

#[derive(Clone)]
pub struct CommandRegistry {
    display_bindings: HashMap<Action, Vec<KeyEvent>>,
    input_bindings: Vec<(KeyEvent, Action)>,
}

#[allow(clippy::too_many_lines)]
impl Default for CommandRegistry {
    fn default() -> Self {
        let mut display_bindings: HashMap<Action, Vec<KeyEvent>> = HashMap::new();
        let mut input_bindings = Vec::new();

        let mut register = |action: Action, key: KeyEvent, is_display: bool| {
            if is_display {
                display_bindings.entry(action).or_default().push(key);
            }
            input_bindings.push((key, action));
        };

        register(
            Action::Quit,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::Logout,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::SecureLogout,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT),
            true,
        );
        register(
            Action::ToggleHelp,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::ToggleHelp,
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            false,
        );
        register(
            Action::ToggleHelp,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
            false,
        );

        register(
            Action::FocusGuilds,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::FocusMessages,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::FocusInput,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::FocusInput,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            false,
        );
        register(
            Action::FocusNext,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::FocusPrevious,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::FocusPrevious,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            true,
        );
        register(
            Action::FocusPrevious,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE),
            false,
        );
        register(
            Action::FocusPrevious,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT),
            false,
        );
        register(
            Action::ToggleGuildsTree,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::ToggleFileExplorer,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::ToggleHiddenFiles,
            KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE),
            false,
        );
        register(
            Action::ToggleHiddenFiles,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            false,
        );
        register(
            Action::NextTab,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            true,
        );

        register(
            Action::NavigateUp,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateUp,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateDown,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateDown,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateLeft,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateLeft,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateRight,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateRight,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            true,
        );

        register(
            Action::Select,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            true,
        );
        register(
            Action::Select,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            false,
        );

        register(
            Action::SelectFirst,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::SelectLast,
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            true,
        );

        register(
            Action::Collapse,
            KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::MoveToParent,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
            true,
        );

        register(
            Action::ScrollDown,
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT),
            true,
        );
        register(
            Action::ScrollUp,
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
            true,
        );
        register(
            Action::ScrollToTop,
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            true,
        );
        register(
            Action::ScrollToBottom,
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            true,
        );
        register(
            Action::Cancel,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            true,
        );

        register(
            Action::Reply,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::ReplyNoMention,
            KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT),
            true,
        );
        register(
            Action::EditMessage,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::DeleteMessage,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::CopyContent,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::CopyImage,
            KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
            true,
        );
        register(
            Action::YankId,
            KeyEvent::new(
                KeyCode::Char('I'),
                KeyModifiers::SHIFT | KeyModifiers::CONTROL,
            ),
            true,
        );
        register(
            Action::OpenAttachments,
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
            true,
        );
        register(
            Action::JumpToReply,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            true,
        );

        register(
            Action::SendMessage,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            true,
        );
        register(
            Action::OpenEditor,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::ClearInput,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::Cancel,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            true,
        );
        register(
            Action::Paste,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
            true,
        );
        register(
            Action::Paste,
            KeyEvent::new(
                KeyCode::Char('V'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            false,
        );
        register(
            Action::Paste,
            KeyEvent::new(
                KeyCode::Char('v'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            false,
        );
        register(
            Action::Paste,
            KeyEvent::new(KeyCode::Char('V'), KeyModifiers::CONTROL),
            false,
        );
        register(
            Action::Paste,
            KeyEvent::new(KeyCode::Insert, KeyModifiers::SHIFT),
            false,
        );

        register(
            Action::ToggleDisplayName,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            true,
        );

        register(
            Action::ToggleQuickSwitcher,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            true,
        );

        Self {
            display_bindings,
            input_bindings,
        }
    }
}

impl CommandRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, action: Action) -> Option<&Vec<KeyEvent>> {
        self.display_bindings.get(&action)
    }

    #[must_use]
    pub fn get_first(&self, action: Action) -> Option<KeyEvent> {
        self.display_bindings
            .get(&action)
            .and_then(|v| v.first().copied())
    }

    #[must_use]
    pub fn find_action(&self, key: KeyEvent) -> Option<Action> {
        self.input_bindings
            .iter()
            .find(|(k, _)| k.code == key.code && k.modifiers == key.modifiers)
            .map(|(_, a)| *a)
    }

    pub fn apply_overrides(&mut self, overrides: &HashMap<String, Action>) {
        for (key_str, action) in overrides {
            if let Some(key_event) = parse_key_event(key_str) {
                self.input_bindings.retain(|(k, _)| *k != key_event);

                self.input_bindings.insert(0, (key_event, *action));

                // Update display bindings
                self.display_bindings
                    .entry(*action)
                    .or_default()
                    .insert(0, key_event);
            } else {
                warn!("Failed to parse keybinding: {}", key_str);
            }
        }
    }
}

fn parse_key_event(s: &str) -> Option<KeyEvent> {
    let mut parts: Vec<&str> = s.split('+').collect();
    let mut modifiers = KeyModifiers::NONE;
    let mut code = KeyCode::Null;

    if parts.len() >= 2
        && parts.last().is_some_and(|p| p.is_empty())
        && parts[parts.len() - 2].is_empty()
    {
        code = KeyCode::Char('+');
        while let Some(last) = parts.last() {
            if last.is_empty() {
                parts.pop();
            } else {
                break;
            }
        }
    } else if s == "+" {
        code = KeyCode::Char('+');
        parts.clear();
    }

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers.insert(KeyModifiers::CONTROL),
            "alt" => modifiers.insert(KeyModifiers::ALT),
            "shift" => modifiers.insert(KeyModifiers::SHIFT),
            key => {
                code = match key {
                    "esc" | "escape" => KeyCode::Esc,
                    "enter" | "return" => KeyCode::Enter,
                    "tab" => KeyCode::Tab,
                    "backtab" => KeyCode::BackTab,
                    "backspace" => KeyCode::Backspace,
                    "delete" | "del" => KeyCode::Delete,
                    "insert" | "ins" => KeyCode::Insert,
                    "home" => KeyCode::Home,
                    "end" => KeyCode::End,
                    "pageup" | "pgup" => KeyCode::PageUp,
                    "pagedown" | "pgdn" => KeyCode::PageDown,
                    "up" => KeyCode::Up,
                    "down" => KeyCode::Down,
                    "left" => KeyCode::Left,
                    "right" => KeyCode::Right,
                    "space" => KeyCode::Char(' '),
                    c if c.len() == 1 => {
                        let char_code = c.chars().next().unwrap();
                        if modifiers.contains(KeyModifiers::SHIFT) && char_code.is_ascii_lowercase()
                        {
                            KeyCode::Char(char_code.to_ascii_uppercase())
                        } else if part.len() == 1 {
                            let original_char = part.chars().next().unwrap();
                            if original_char.is_ascii_uppercase() {
                                modifiers.insert(KeyModifiers::SHIFT);
                            }
                            KeyCode::Char(original_char)
                        } else {
                            KeyCode::Char(char_code)
                        }
                    }
                    f if f.starts_with('f') => {
                        if let Ok(n) = f[1..].parse::<u8>() {
                            KeyCode::F(n)
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                };
            }
        }
    }

    if code == KeyCode::Null {
        return None;
    }

    Some(KeyEvent::new(code, modifiers))
}

pub trait HasCommands {
    fn get_commands(&self, registry: &CommandRegistry) -> Vec<Keybind>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use test_case::test_case;

    #[test_case("a", KeyCode::Char('a'), KeyModifiers::NONE ; "simple char")]
    #[test_case("A", KeyCode::Char('A'), KeyModifiers::SHIFT ; "uppercase char")]
    #[test_case("Ctrl+c", KeyCode::Char('c'), KeyModifiers::CONTROL ; "ctrl char")]
    #[test_case("Shift+a", KeyCode::Char('A'), KeyModifiers::SHIFT ; "shift char")]
    #[test_case("+", KeyCode::Char('+'), KeyModifiers::NONE ; "plus key")]
    #[test_case("Ctrl++", KeyCode::Char('+'), KeyModifiers::CONTROL ; "ctrl plus")]
    #[test_case("F1", KeyCode::F(1), KeyModifiers::NONE ; "function key")]
    fn test_key_parsing(input: &str, expected_code: KeyCode, expected_modifiers: KeyModifiers) {
        assert_eq!(
            parse_key_event(input),
            Some(KeyEvent::new(expected_code, expected_modifiers))
        );
    }
}
