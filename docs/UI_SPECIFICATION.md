# Discordo UI Specification for Ratatui Implementation

## Executive Summary

This document provides a comprehensive specification for implementing the Discordo terminal user interface using ratatui (Rust). It is based on a detailed analysis of the existing Go/tview implementation and follows clean architecture principles.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Screen Hierarchy](#2-screen-hierarchy)
3. [Component Specifications](#3-component-specifications)
4. [Keybinding Reference](#4-keybinding-reference)
5. [ASCII Mockups](#5-ascii-mockups)
6. [State Management](#6-state-management)
7. [Navigation Flow](#7-navigation-flow)
8. [Ratatui Widget Mapping](#8-ratatui-widget-mapping)
9. [Implementation Guidelines](#9-implementation-guidelines)

---

## 1. Architecture Overview

### 1.1 Layer Separation

```
presentation/
    events/           # Event handling (keyboard, mouse)
    screens/          # Full-screen views (Login, Chat)
    widgets/          # Reusable UI components
    theme/            # Theming and styling

application/          # Use cases and DTOs
domain/               # Entities, ports, errors
infrastructure/       # Discord client, storage, config
```

### 1.2 Core Design Principles

| Principle              | Application                                           |
| ---------------------- | ----------------------------------------------------- |
| Single Responsibility  | Each widget handles one concern                       |
| Dependency Inversion   | UI depends on traits, not concrete implementations    |
| Separation of Concerns | Rendering separate from state management              |
| Clean Architecture     | Presentation layer knows nothing about infrastructure |
| Testability            | All components unit-testable in isolation             |

### 1.3 Data Flow

```
User Input -> EventHandler -> Screen/Widget -> Use Case -> Domain -> Response
                                    |
                                    v
                              Render Update
```

---

## 2. Screen Hierarchy

### 2.1 Application States

```rust
enum AppState {
    Login,      // Authentication flow
    Chat,       // Main Discord interface
    Exiting,    // Graceful shutdown
}
```

### 2.2 Screen Transition Diagram

```
                    +------------------+
                    |                  |
                    v                  |
    +---------+  token exists   +------------+
    |  Start  |---------------->|    Chat    |
    +---------+                 +------------+
         |                            |
         | no token                   | logout
         v                            v
    +---------+  login success  +------------+
    |  Login  |---------------->|    Chat    |
    +---------+                 +------------+
         |                            |
         | Ctrl+C                     | Ctrl+C
         v                            v
    +---------+                 +---------+
    |  Exit   |                 |  Exit   |
    +---------+                 +---------+
```

---

## 3. Component Specifications

### 3.1 Login Screen

#### Components

- `LoginForm`: Email/password/2FA authentication
- `QrLoginView`: QR code-based mobile authentication
- `ErrorModal`: Centered modal for error display

#### Variants

| Variant | Description                       |
| ------- | --------------------------------- |
| Form    | Standard email/password login     |
| QR      | QR code displayed for mobile scan |
| Error   | Error modal overlaying form       |

#### State Machine

```rust
enum LoginFormState {
    Ready,           // Awaiting input
    Validating,      // API call in progress
    AwaitingMfa,     // 2FA code required
    Error(String),   // Display error message
    Success,         // Transition to Chat
}

enum QrLoginState {
    Connecting,      // WebSocket handshake
    Displaying,      // QR code visible
    Scanned,         // User scanned, awaiting confirm
    Confirmed,       // Mobile approved
    Error(String),   // Connection failed
}
```

### 3.2 Chat Screen (Main View)

#### Layout Structure

```
+------------------------------------------------------------------+
|                        Chat Screen                                |
+------------------+-----------------------------------------------+
|                  |                                               |
|   GuildsTree     |              MessagesPane                     |
|   (collapsible)  |    +-------------------------------------+    |
|                  |    |          MessagesList               |    |
|   - DMs          |    |    (scrollable, selectable)         |    |
|   - Servers      |    |                                     |    |
|   - Channels     |    |                                     |    |
|                  |    +-------------------------------------+    |
|                  |    |          MessageInput               |    |
|                  |    |    (multi-line, autocomplete)       |    |
+------------------+-----------------------------------------------+
                   |
                   +-- MentionsList (floating, conditional)
                   +-- AttachmentsList (floating, conditional)
                   +-- ConfirmModal (centered, conditional)
```

#### Primary Components

| Component         | Responsibility                         |
| ----------------- | -------------------------------------- |
| `GuildsTree`      | Server/channel navigation hierarchy    |
| `MessagesList`    | Display and selection of messages      |
| `MessageInput`    | Message composition with rich features |
| `MentionsList`    | Floating autocomplete for @mentions    |
| `AttachmentsList` | URL/file selection popup               |
| `ConfirmModal`    | Confirmation dialogs                   |

---

## 4. Keybinding Reference

### 4.1 Global Keybindings

| Key      | Action                | Description                    |
| -------- | --------------------- | ------------------------------ |
| `Ctrl+C` | `quit`                | Exit application               |
| `Ctrl+D` | `logout`              | Log out and clear stored token |
| `Ctrl+G` | `focus_guilds_tree`   | Focus guild/channel tree       |
| `Ctrl+T` | `focus_messages_list` | Focus messages view            |
| `Ctrl+I` | `focus_message_input` | Focus message input            |
| `Ctrl+H` | `focus_previous`      | Cycle focus backward           |
| `Ctrl+L` | `focus_next`          | Cycle focus forward            |
| `Ctrl+B` | `toggle_guilds_tree`  | Show/hide guilds tree          |

### 4.2 Guilds Tree Keybindings

| Key     | Action                 | Description           |
| ------- | ---------------------- | --------------------- |
| `j`     | `select_next`          | Move selection down   |
| `k`     | `select_previous`      | Move selection up     |
| `g`     | `select_first`         | Jump to first item    |
| `G`     | `select_last`          | Jump to last item     |
| `Enter` | `select_current`       | Expand/select node    |
| `i`     | `yank_id`              | Copy guild/channel ID |
| `-`     | `collapse_parent_node` | Collapse parent       |
| `p`     | `move_to_parent_node`  | Navigate to parent    |

### 4.3 Messages List Keybindings

| Key    | Action            | Description              |
| ------ | ----------------- | ------------------------ |
| `j`    | `select_next`     | Select newer message     |
| `k`    | `select_previous` | Select older message     |
| `g`    | `select_first`    | Jump to oldest message   |
| `G`    | `select_last`     | Jump to newest message   |
| `J`    | `scroll_down`     | Scroll without selection |
| `K`    | `scroll_up`       | Scroll without selection |
| `Home` | `scroll_top`      | Scroll to top            |
| `End`  | `scroll_bottom`   | Scroll to bottom         |
| `s`    | `select_reply`    | Jump to replied message  |
| `r`    | `reply_mention`   | Reply with @mention      |
| `R`    | `reply`           | Reply without @mention   |
| `e`    | `edit`            | Edit own message         |
| `d`    | `delete_confirm`  | Delete with confirmation |
| `D`    | `delete`          | Delete immediately       |
| `o`    | `open`            | Open attachments/URLs    |
| `y`    | `yank_content`    | Copy message content     |
| `u`    | `yank_url`        | Copy message URL         |
| `i`    | `yank_id`         | Copy message ID          |
| `Esc`  | `cancel`          | Clear selection          |

### 4.4 Message Input Keybindings

| Key      | Action             | Description              |
| -------- | ------------------ | ------------------------ |
| `Enter`  | `send`             | Send message             |
| `Esc`    | `cancel`           | Clear input/cancel reply |
| `Tab`    | `tab_complete`     | Autocomplete @mention    |
| `Ctrl+V` | `paste`            | Paste text/image         |
| `Ctrl+E` | `open_editor`      | Open external editor     |
| `Ctrl+\` | `open_file_picker` | Attach files             |

### 4.5 Mentions List Keybindings

| Key      | Action   | Description         |
| -------- | -------- | ------------------- |
| `Ctrl+P` | `up`     | Previous suggestion |
| `Ctrl+N` | `down`   | Next suggestion     |
| `Enter`  | `select` | Accept suggestion   |
| `Esc`    | `close`  | Close suggestions   |

---

## 5. ASCII Mockups

### 5.1 Login Screen - Form View

```
+--------------------------------------------------------------------------------+
|                                                                                |
|                                                                                |
|                                                                                |
|                       +-----------------------------------------+              |
|                       |            Discordo Login              |              |
|                       +-----------------------------------------+              |
|                       |                                         |              |
|                       |  Email                                  |              |
|                       |  +-----------------------------------+  |              |
|                       |  | user@example.com                 |  |              |
|                       |  +-----------------------------------+  |              |
|                       |                                         |              |
|                       |  Password                               |              |
|                       |  +-----------------------------------+  |              |
|                       |  | ••••••••••••                      |  |              |
|                       |  +-----------------------------------+  |              |
|                       |                                         |              |
|                       |  Code (optional)                        |              |
|                       |  +-----------------------------------+  |              |
|                       |  |                                   |  |              |
|                       |  +-----------------------------------+  |              |
|                       |                                         |              |
|                       |  [ Login ]      [ Login with QR ]       |              |
|                       |                                         |              |
|                       +-----------------------------------------+              |
|                                                                                |
+--------------------------------------------------------------------------------+
```

### 5.2 Login Screen - QR View

```
+--------------------------------------------------------------------------------+
|                                                                                |
|                       +-----------------------------------------+              |
|                       |            Login with QR               |              |
|                       +-----------------------------------------+              |
|                       |                                         |              |
|                       |         ██████████████████████          |              |
|                       |         ██                  ██          |              |
|                       |         ██  ████████████    ██          |              |
|                       |         ██  ██        ██    ██          |              |
|                       |         ██  ██  ████  ██    ██          |              |
|                       |         ██  ██  ████  ██    ██          |              |
|                       |         ██  ██        ██    ██          |              |
|                       |         ██  ████████████    ██          |              |
|                       |         ██                  ██          |              |
|                       |         ██████████████████████          |              |
|                       |                                         |              |
|                       |     Scan with Discord mobile app        |              |
|                       |                                         |              |
|                       |           Press Esc to cancel           |              |
|                       |                                         |              |
|                       +-----------------------------------------+              |
|                                                                                |
+--------------------------------------------------------------------------------+
```

### 5.3 Login Screen - Token Input (Current Rust)

```
+--------------------------------------------------------------------------------+
|                                                                                |
|                                                                                |
|                       +-----------------------------------------+              |
|                       |           Discordo Login               |              |
|                       +-----------------------------------------+              |
|                       |                                         |              |
|                       | Enter your Discord token to login       |              |
|                       |                                         |              |
|                       | Discord Token                           |              |
|                       | +-------------------------------------+ |              |
|                       | | ••••••••••••••••••••••••••••••••••• | |              |
|                       | +-------------------------------------+ |              |
|                       |                                         |              |
|                       | [x] Remember token (Tab to toggle)      |              |
|                       |                                         |              |
|                       | Press Enter to login, Esc to quit       |              |
|                       |                                         |              |
|                       +-----------------------------------------+              |
|                                                                                |
+--------------------------------------------------------------------------------+
```

### 5.4 Chat Screen - Main View (Full)

```
+--------------------------------------------------------------------------------+
| Guilds                          | #general - Welcome to the server!            |
+-------------------------------+ +----------------------------------------------+
| > Direct Messages              | | 10:30 AM Alice                              |
|   > @friend1                   | | Hello everyone! How's it going?             |
|   > @friend2                   | |                                             |
| v My Server                    | | 10:31 AM Bob                                |
|   v general                    | | > Alice: Hello everyone!                    |
|   | #general                   | | Pretty good! Working on some code.          |
|   | #random                    | |                                             |
|   v dev                        | | 10:32 AM Charlie                            |
|     #frontend                  | | Check out this image:                       |
|     #backend                   | | [screenshot.png]                            |
| > Another Server               | |                                             |
|                                | | 10:35 AM Alice                              |
|                                | | @Bob nice! What are you working on?         |
|                                | |                                             |
|                                | |                                             |
|                                | |                                             |
+-------------------------------+ +----------------------------------------------+
                                  | Message...                                   |
                                  +----------------------------------------------+
```

### 5.5 Chat Screen - With Selection Highlight

```
+--------------------------------------------------------------------------------+
| Guilds                          | #general - Welcome to the server!            |
+-------------------------------+ +----------------------------------------------+
| > Direct Messages              | | 10:30 AM Alice                              |
|   > @friend1                   | | Hello everyone! How's it going?             |
|                                | |                                             |
| v My Server                    | | [SELECTED]==================[SELECTED]      |
|   v general                    | | 10:31 AM Bob                                |
|   | #general <                 | | > Alice: Hello everyone!                    |
|   | #random                    | | Pretty good! Working on some code.          |
|                                | | [SELECTED]==================[SELECTED]      |
|                                | |                                             |
|                                | | 10:32 AM Charlie                            |
|                                | | Check out this image:                       |
|                                | | [screenshot.png]                            |
|                                | |                                             |
+-------------------------------+ +----------------------------------------------+
                                  | Replying to Bob                              |
                                  +----------------------------------------------+
                                  | @Bob that sounds interesting!                |
                                  +----------------------------------------------+
```

### 5.6 Chat Screen - Mentions Autocomplete

```
+--------------------------------------------------------------------------------+
| Guilds                          | #general                                     |
+-------------------------------+ +----------------------------------------------+
| > Direct Messages              | | 10:30 AM Alice                              |
|   > @friend1                   | | Hello everyone!                             |
|                                | |                                             |
| v My Server                    | | 10:31 AM Bob                                |
|   #general                     | | Pretty good!                                |
|   #random                      | |                                             |
|                                | +----------------------------------------------+
|                                | | Mentions                                    |
|                                | +----------------------------------------------+
|                                | | > Alice                                     |
|                                | |   Bob                                       |
|                                | |   Charlie                                   |
|                                | +----------------------------------------------+
|                                | | Hey @ali|                                   |
+-------------------------------+ +----------------------------------------------+
```

### 5.7 Chat Screen - Guilds Tree Hidden

```
+--------------------------------------------------------------------------------+
| #general - Welcome to the server!                                              |
+--------------------------------------------------------------------------------+
| 10:30 AM Alice                                                                 |
| Hello everyone! How's it going?                                                |
|                                                                                |
| 10:31 AM Bob                                                                   |
| > Alice: Hello everyone!                                                       |
| Pretty good! Working on some code.                                             |
|                                                                                |
| 10:32 AM Charlie                                                               |
| Check out this image:                                                          |
| [screenshot.png]                                                               |
|                                                                                |
| 10:35 AM Alice                                                                 |
| @Bob nice! What are you working on?                                            |
|                                                                                |
+--------------------------------------------------------------------------------+
| Message...                                                                     |
+--------------------------------------------------------------------------------+
```

### 5.8 Confirmation Modal

```
+--------------------------------------------------------------------------------+
| Guilds                          | #general                                     |
+-------------------------------+ +----------------------------------------------+
| > Direct Messages              | | 10:30 AM Alice                              |
|                                | | Hello everyone!                             |
| v My Server           +-------------------------------------+                  |
|   #general            |                                     |                  |
|   #random             | Are you sure you want to delete     |                  |
|                       | this message?                       |                  |
|                       |                                     |                  |
|                       |      [ Yes ]        [ No ]          |                  |
|                       |                                     |                  |
|                       +-------------------------------------+                  |
|                                | |                                             |
+-------------------------------+ +----------------------------------------------+
                                  | Message...                                   |
                                  +----------------------------------------------+
```

### 5.9 Attachments List Popup

```
+--------------------------------------------------------------------------------+
| Guilds                          | #general                                     |
+-------------------------------+ +----------------------------------------------+
| > Direct Messages              | | 10:32 AM Charlie                            |
|                                | | Check out these files:                      |
| v My Server                    | |                                             |
|   #general                     | +----------------------------+                |
|   #random                      | | a) screenshot.png          |                |
|                                | | b) document.pdf            |                |
|                                | | 1) https://example.com     |                |
|                                | | 2) https://github.com/...  |                |
|                                | +----------------------------+                |
|                                | |                                             |
+-------------------------------+ +----------------------------------------------+
                                  | Message...                                   |
                                  +----------------------------------------------+
```

### 5.10 Error Modal

```
+--------------------------------------------------------------------------------+
|                                                                                |
|                                                                                |
|                       +-----------------------------------------+              |
|                       |                                         |              |
|                       |  Failed to send message:                |              |
|                       |  Rate limited. Try again in 5 seconds.  |              |
|                       |                                         |              |
|                       |       [ Copy ]       [ Close ]          |              |
|                       |                                         |              |
|                       +-----------------------------------------+              |
|                                                                                |
+--------------------------------------------------------------------------------+
```

---

## 6. State Management

### 6.1 Application State

```rust
pub struct AppState {
    current_screen: Screen,
    discord_state: Option<DiscordState>,
    config: AppConfig,
    focus: FocusTarget,
}

pub enum Screen {
    Login(LoginScreenState),
    Chat(ChatScreenState),
}

pub enum FocusTarget {
    GuildsTree,
    MessagesList,
    MessageInput,
    Modal(ModalType),
}
```

### 6.2 Discord State

```rust
pub struct DiscordState {
    user: User,
    guilds: Vec<Guild>,
    channels: HashMap<GuildId, Vec<Channel>>,
    messages: HashMap<ChannelId, Vec<Message>>,
    members: HashMap<GuildId, Vec<Member>>,
    read_states: HashMap<ChannelId, ReadState>,
    presences: HashMap<UserId, Presence>,
}
```

### 6.3 Chat Screen State

```rust
pub struct ChatScreenState {
    selected_guild: Option<GuildId>,
    selected_channel: Option<Channel>,
    selected_message: Option<MessageId>,
    guilds_tree_visible: bool,
    guilds_tree: GuildsTreeState,
    messages_list: MessagesListState,
    message_input: MessageInputState,
    active_modal: Option<ModalState>,
}
```

### 6.4 Component States

```rust
pub struct GuildsTreeState {
    root_nodes: Vec<TreeNode>,
    current_node: Option<NodeId>,
    expanded_nodes: HashSet<NodeId>,
    scroll_offset: usize,
}

pub struct MessagesListState {
    messages: Vec<Message>,
    highlighted_id: Option<MessageId>,
    scroll_offset: usize,
    image_cache: HashMap<String, ImageCacheEntry>,
    is_fetching_members: bool,
}

pub struct MessageInputState {
    text: String,
    cursor_position: usize,
    mode: InputMode,
    reply_to: Option<MessageReference>,
    attached_files: Vec<AttachedFile>,
    mentions_list: Option<MentionsListState>,
}

pub enum InputMode {
    Normal,
    Editing(MessageId),
    Replying { mention: bool },
}

pub struct MentionsListState {
    suggestions: Vec<MentionSuggestion>,
    selected_index: usize,
    search_query: String,
}
```

### 6.5 State Update Pattern

```rust
pub enum AppAction {
    // Navigation
    FocusNext,
    FocusPrevious,
    FocusWidget(FocusTarget),
    ToggleGuildsTree,

    // Guild tree
    SelectGuild(GuildId),
    SelectChannel(ChannelId),
    ExpandNode(NodeId),
    CollapseNode(NodeId),

    // Messages
    SelectMessage(MessageId),
    ClearSelection,
    StartReply { mention: bool },
    StartEdit(MessageId),
    DeleteMessage(MessageId),

    // Input
    InputChar(char),
    InputBackspace,
    InputSubmit,
    InputCancel,
    ShowMentions(Vec<MentionSuggestion>),
    SelectMention(usize),

    // Discord events
    MessageReceived(Message),
    MessageUpdated(Message),
    MessageDeleted(MessageId),
    ReadStateUpdated(ChannelId, MessageId),
    MembersChunkReceived(Vec<Member>),

    // Modals
    ShowConfirmModal(String, Vec<String>),
    CloseModal,
    ModalButtonPressed(usize),
}
```

---

## 7. Navigation Flow

### 7.1 Focus Cycle

```
                     Ctrl+L
    +--------+    -------->    +-------------+
    | Guilds |                 |  Messages   |
    |  Tree  |                 |    List     |
    +--------+    <--------    +-------------+
        ^          Ctrl+H            |
        |                            |
   Ctrl+H                        Ctrl+L
        |                            v
        |                      +-----------+
        +-------- Ctrl+H ----- |  Message  |
                               |   Input   |
                               +-----------+
                                     ^
                                     |
                                 Ctrl+I
                            (direct focus)
```

### 7.2 Guild Tree Navigation

```
Root
├── Direct Messages          <- Enter: expand/collapse
│   ├── @friend1            <- Enter: load DM messages
│   └── @friend2            <- j/k: navigate up/down
├── Server A (bold=unread)   <- Enter: expand, load channels
│   ├── #general            <- Enter: load messages
│   └── #random             <- g/G: jump to first/last
└── Server B
    └── Category
        └── #channel        <- p: go to parent, -: collapse parent
```

### 7.3 Message Selection Flow

```
No selection
    |
    | k or j
    v
Message selected (highlighted)
    |
    +-- r -> Reply mode (input focused, title shows "Replying to...")
    |
    +-- R -> Reply w/ mention (input focused)
    |
    +-- e -> Edit mode (own messages only, content copied to input)
    |
    +-- d -> Confirm modal appears
    |        |
    |        +-- Yes -> Message deleted
    |        +-- No  -> Return to selection
    |
    +-- D -> Immediate delete (no confirmation)
    |
    +-- o -> Open attachments/URLs
    |        |
    |        +-- Single item: opens directly
    |        +-- Multiple: shows AttachmentsList
    |
    +-- Esc -> Clear selection
```

### 7.4 Mentions Autocomplete Flow

```
Typing in message input
    |
    | @ typed
    v
Start autocomplete
    |
    +-- Type characters -> Filter suggestions
    |
    +-- Tab -> Insert selected mention
    |
    +-- Ctrl+P/N -> Navigate suggestions
    |
    +-- Enter -> Accept selected
    |
    +-- Esc -> Cancel autocomplete
```

---

## 8. Ratatui Widget Mapping

### 8.1 tview to ratatui Equivalents

| tview Component    | ratatui Equivalent          | Notes                                |
| ------------------ | --------------------------- | ------------------------------------ |
| `tview.Box`        | `Block`                     | Base container with borders          |
| `tview.Flex`       | `Layout`                    | Use `Direction::Horizontal/Vertical` |
| `tview.Grid`       | `Layout` + constraints      | Nested layouts                       |
| `tview.Pages`      | State machine + conditional | Manage in App state                  |
| `tview.TreeView`   | Custom `TreeWidget`         | Build from `List` + state            |
| `tview.TextView`   | `Paragraph`                 | Use `wrap: Wrap::Word`               |
| `tview.TextArea`   | Custom `TextArea`           | Use `tui-textarea` or custom         |
| `tview.List`       | `List`                      | Stateful with `ListState`            |
| `tview.Form`       | Custom composite            | Multiple `TextInput` widgets         |
| `tview.Modal`      | `Popup` + `Clear`           | Centered overlay                     |
| `tview.InputField` | Custom `TextInput`          | Already implemented                  |

### 8.2 Custom Widget Requirements

```rust
// Tree widget for guild/channel navigation
pub struct TreeWidget<'a> {
    items: &'a [TreeItem],
    state: &'a TreeState,
    style: TreeStyle,
}

// Message display widget
pub struct MessageWidget<'a> {
    message: &'a Message,
    highlighted: bool,
    show_timestamp: bool,
    style: MessageStyle,
}

// Multi-line text input with autocomplete
pub struct MessageInputWidget<'a> {
    state: &'a MessageInputState,
    placeholder: &'a str,
    style: InputStyle,
}

// Floating popup list
pub struct PopupList<'a> {
    items: &'a [String],
    selected: usize,
    title: &'a str,
    style: PopupStyle,
}
```

### 8.3 Layout Construction

```rust
fn render_chat_screen(frame: &mut Frame, state: &ChatScreenState) {
    let main_layout = Layout::horizontal([
        Constraint::Percentage(if state.guilds_tree_visible { 20 } else { 0 }),
        Constraint::Min(0),
    ]);

    let [guilds_area, right_area] = main_layout.areas(frame.area());

    if state.guilds_tree_visible {
        render_guilds_tree(frame, guilds_area, &state.guilds_tree);
    }

    let right_layout = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(3),
    ]);

    let [messages_area, input_area] = right_layout.areas(right_area);

    render_messages_list(frame, messages_area, &state.messages_list);
    render_message_input(frame, input_area, &state.message_input);

    // Render floating elements last (on top)
    if let Some(ref mentions) = state.message_input.mentions_list {
        render_mentions_popup(frame, input_area, mentions);
    }

    if let Some(ref modal) = state.active_modal {
        render_modal(frame, frame.area(), modal);
    }
}
```

---

## 9. Implementation Guidelines

### 9.1 Widget Implementation Pattern

```rust
pub struct MyWidget {
    // Configuration (immutable after creation)
    style: MyStyle,
}

pub struct MyWidgetState {
    // Mutable state
    selected: usize,
    scroll_offset: usize,
}

impl StatefulWidget for MyWidget {
    type State = MyWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Pure rendering, no side effects
    }
}

impl MyWidgetState {
    // State mutation methods
    pub fn select_next(&mut self) { ... }
    pub fn select_previous(&mut self) { ... }
}
```

### 9.2 Event Handling Pattern

```rust
pub trait HandleEvent {
    type Action;

    fn handle_key(&mut self, key: KeyEvent) -> Option<Self::Action>;
}

impl HandleEvent for GuildsTreeState {
    type Action = GuildsTreeAction;

    fn handle_key(&mut self, key: KeyEvent) -> Option<Self::Action> {
        match key.code {
            KeyCode::Char('j') => {
                self.select_next();
                None
            }
            KeyCode::Enter => {
                self.current_node.map(GuildsTreeAction::Select)
            }
            _ => None,
        }
    }
}
```

### 9.3 Theming System

```rust
pub struct Theme {
    pub border: BorderTheme,
    pub title: TitleTheme,
    pub guilds_tree: GuildsTreeTheme,
    pub messages_list: MessagesListTheme,
    pub message_input: MessageInputTheme,
}

pub struct BorderTheme {
    pub enabled: bool,
    pub normal_style: Style,
    pub active_style: Style,
    pub border_type: BorderType,
    pub padding: [u16; 4],
}

impl Theme {
    pub fn from_config(config: &ThemeConfig) -> Self {
        // Convert config values to ratatui styles
    }
}
```

### 9.4 Focus Management

```rust
pub struct FocusManager {
    current: FocusTarget,
    previous: Option<FocusTarget>,
}

impl FocusManager {
    pub fn focus(&mut self, target: FocusTarget) {
        self.previous = Some(self.current);
        self.current = target;
    }

    pub fn focus_next(&mut self, available: &[FocusTarget]) {
        let idx = available.iter().position(|t| *t == self.current)
            .map(|i| (i + 1) % available.len())
            .unwrap_or(0);
        self.focus(available[idx]);
    }

    pub fn restore_previous(&mut self) {
        if let Some(prev) = self.previous.take() {
            self.focus(prev);
        }
    }
}
```

### 9.5 Async Event Integration

```rust
pub struct EventHandler {
    discord_rx: mpsc::Receiver<DiscordEvent>,
    tick_rate: Duration,
}

impl EventHandler {
    pub async fn next(&mut self) -> Event {
        tokio::select! {
            Some(discord_event) = self.discord_rx.recv() => {
                Event::Discord(discord_event)
            }
            _ = tokio::time::sleep(self.tick_rate) => {
                Event::Tick
            }
            Ok(true) = crossterm::event::poll(Duration::ZERO) => {
                if let Ok(event) = crossterm::event::read() {
                    Event::Terminal(event)
                } else {
                    Event::Tick
                }
            }
        }
    }
}
```

### 9.6 Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("Rendering failed: {0}")]
    RenderError(String),

    #[error("Terminal error: {0}")]
    TerminalError(#[from] std::io::Error),

    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition { from: AppState, to: AppState },
}

// Use Result types, avoid unwrap()
fn render_widget(&self, area: Rect, buf: &mut Buffer) -> Result<(), UiError> {
    // ...
}
```

### 9.7 Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_guilds_tree_navigation() {
        let mut state = GuildsTreeState::new(mock_guilds());

        state.select_next();
        assert_eq!(state.selected_index(), 1);

        state.select_previous();
        assert_eq!(state.selected_index(), 0);
    }

    #[test]
    fn test_widget_render() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| {
            let widget = MyWidget::new();
            let mut state = MyWidgetState::default();
            frame.render_stateful_widget(widget, frame.area(), &mut state);
        }).unwrap();

        // Assert buffer contents
        let buffer = terminal.backend().buffer();
        assert!(buffer.get(0, 0).symbol() == "x");
    }
}
```

---

## Appendix A: Configuration Schema

```toml
# Discordo Configuration (config.toml)

auto_focus = true
mouse = true
editor = "default"
status = "default"
markdown = true
hide_blocked_users = true
show_attachment_links = true
autocomplete_limit = 20
messages_limit = 50

[timestamps]
enabled = true
format = "3:04PM"

[notifications]
enabled = true
duration = 0
[notifications.sound]
enabled = true
only_on_ping = true

[image_previews]
enabled = false
type = "auto"
max_height = 10
max_width = 40

[keys]
# See Section 4 for complete keybinding reference

[theme]
# See theme.go for complete theme options
```

---

## Appendix B: File Structure

```
src/presentation/
├── mod.rs
├── events/
│   ├── mod.rs
│   └── handler.rs
├── screens/
│   ├── mod.rs
│   ├── login/
│   │   ├── mod.rs
│   │   ├── form.rs
│   │   ├── qr.rs
│   │   └── state.rs
│   └── chat/
│       ├── mod.rs
│       ├── screen.rs
│       └── state.rs
├── widgets/
│   ├── mod.rs
│   ├── guilds_tree.rs
│   ├── messages_list.rs
│   ├── message_input.rs
│   ├── mentions_list.rs
│   ├── popup.rs
│   ├── modal.rs
│   └── text_input.rs
└── theme/
    ├── mod.rs
    ├── config.rs
    └── styles.rs
```

---

## Appendix C: Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
thiserror = "2.0"
tracing = "0.1"

# Optional but recommended
tui-textarea = "0.7"     # For MessageInput
unicode-width = "0.2"    # For text width calculations
textwrap = "0.16"        # For message wrapping
```

---

_Document Version: 1.0_
_Generated from Discordo Go implementation analysis_
_Target: ratatui 0.29+ with async runtime_
