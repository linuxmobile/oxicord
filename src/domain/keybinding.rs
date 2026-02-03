use crossterm::event::KeyEvent;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    SecureLogout,
    ToggleDisplayName,
    ToggleQuickSwitcher,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybind {
    pub key: KeyEvent,
    pub action: Action,
    pub label: Cow<'static, str>,
    pub visible_in_bar: bool,
}

impl Keybind {
    pub fn new(key: KeyEvent, action: Action, label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key,
            action,
            label: label.into(),
            visible_in_bar: true,
        }
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
