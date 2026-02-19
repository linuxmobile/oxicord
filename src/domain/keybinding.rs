use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Quit,
    Logout,
    ToggleHelp,
    ToggleGuildsTree,
    ToggleFileExplorer,
    ToggleHiddenFiles,

    FocusGuilds,
    FocusMessages,
    FocusInput,
    FocusNext,
    FocusPrevious,
    NextTab,
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Select,
    SelectFirst,
    SelectLast,
    Collapse,
    MoveToParent,

    ScrollDown,
    ScrollUp,
    ScrollToTop,
    ScrollToBottom,
    LoadHistory,
    ClearSelection,

    SendMessage,
    Reply,
    ReplyNoMention,
    EditMessage,
    DeleteMessage,
    CopyContent,
    CopyImage,
    YankId,
    YankUrl,
    OpenAttachments,
    JumpToReply,

    OpenEditor,
    ClearInput,
    Cancel,
    Paste,
    NewLine,
    SecureLogout,
    ToggleDisplayName,
    ToggleQuickSwitcher,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybind {
    pub key: KeyEvent,
    pub action: Action,
    pub label: Cow<'static, str>,
    pub visible_in_bar: bool,
    pub key_display: Option<String>,
}

impl Keybind {
    pub fn new(key: KeyEvent, action: Action, label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key,
            action,
            label: label.into(),
            visible_in_bar: true,
            key_display: None,
        }
    }

    #[must_use]
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.key_display = Some(display.into());
        self
    }

    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.visible_in_bar = false;
        self
    }
}

pub struct KeyDefinition {
    pub key: KeyEvent,
    pub action: Action,
}
