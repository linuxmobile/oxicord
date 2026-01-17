use crate::domain::keybinding::{Action, Keybind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

pub struct CommandRegistry {
    display_bindings: HashMap<Action, KeyEvent>,
    input_bindings: Vec<(KeyEvent, Action)>,
}

#[allow(clippy::too_many_lines)]
impl Default for CommandRegistry {
    fn default() -> Self {
        let mut display_bindings = HashMap::new();
        let mut input_bindings = Vec::new();

        let mut register = |action: Action, key: KeyEvent, is_primary: bool| {
            if is_primary {
                display_bindings.insert(action, key);
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
            Action::ToggleHelp,
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            true,
        );
        register(
            Action::ToggleHelp,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
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
            Action::FocusInput,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
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
            Action::NextTab,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            false,
        );

        register(
            Action::NavigateUp,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateUp,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            false,
        );
        register(
            Action::NavigateDown,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateDown,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            false,
        );
        register(
            Action::NavigateLeft,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateLeft,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            false,
        );
        register(
            Action::NavigateRight,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            true,
        );
        register(
            Action::NavigateRight,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            false,
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
            Action::YankId,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
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
    pub fn get(&self, action: Action) -> Option<KeyEvent> {
        self.display_bindings.get(&action).copied()
    }

    #[must_use]
    pub fn find_action(&self, key: KeyEvent) -> Option<Action> {
        self.input_bindings
            .iter()
            .find(|(k, _)| k.code == key.code && k.modifiers == key.modifiers)
            .map(|(_, a)| *a)
    }
}

pub trait HasCommands {
    fn get_commands(&self, registry: &CommandRegistry) -> Vec<Keybind>;
}
