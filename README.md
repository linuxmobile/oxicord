# Oxicord

Oxicord is a high-performance, memory-safe Discord TUI client written in Rust. It is a spiritual successor to [Discordo](https://github.com/ayn2op/discordo), rewritten from scratch to leverage the performance and safety guarantees of Rust and the Ratatui ecosystem.

Designed for power users on Linux who demand speed, minimal resource usage, and terminal aesthetics, Oxicord aims to be the definitive CLI experience for Discord.

## Why Oxicord?

Oxicord distinguishes itself through a commitment to modern engineering principles and user experience:

- **Uncompromising Performance:** Built with Rust, Oxicord delivers instant startup times and negligible memory footprint compared to Electron or Go-based alternatives.
- **Clean Architecture:** The codebase follows strict Clean Architecture principles (Domain, Application, Infrastructure), making it robust, testable, and approachable for contributors.
- **TUI Fidelity:** Utilizing `ratatui` and `tachyonfx`, Oxicord provides a fluid, glitch-free interface with full mouse support and responsive layouts.

## Features

Oxicord implements a focused set of "real" features designed for daily drivers, prioritizing stability over bloat:

### Core Experience

- **Vim-like Navigation:** Native `j`/`k` navigation, `g`/`G` scrolling, and intuitive focus management.
- **Infinite Scrolling:** Seamless history loading. Scroll up, and history fetches automatically without manual "load more" buttons.
- **Smart "Follow" Mode:** The view automatically snaps to new messages but respects your position when reading history.

### Visual Precision

- **Rich Text Rendering:** Full Markdown support with syntax highlighting (via `syntect`) for code blocks.
- **Precision Timestamps:** 6-character timestamps (e.g., `14:05:32`) with automatic **Local Timezone** conversion—no more UTC mental math.
- **Visual Indicators:**
  - **Unread Markers:** Bold text and bullet indicators (`●`) for unread channels and guilds.
  - **Typing Indicators:** Real-time feedback when others are typing.
  - **Full-Width Selection:** Messages are selected across the full width of the pane for superior readability.

### System Integration

- **Built-in File Explorer:** Integrated TUI file picker for attaching files without leaving the terminal.
- **Secure Authentication:** Options for ephemeral Token login or secure storage using system keyrings (`libsecret`/`keyring`).
- **Clipboard Integration:** One-key copying of message content or IDs to your system clipboard.

## Fair Play Comparison

We stand on the shoulders of giants. Here is how Oxicord compares to existing terminal clients:

- **Endcord (Python):** Endcord is a feature beast (Voice, Plugins, Image previews). However, as a Python application, it carries the runtime overhead of an interpreted language. **Oxicord (Rust)** prioritizes raw performance, memory safety, and type-safe reliability, aiming for a "crash-proof" experience rather than feature parity at the cost of stability.
- **Discordo (Go):** The original inspiration. While Discordo pioneered this TUI layout, it uses the `tview` library and a flatter Go architecture. **Oxicord** moves to `ratatui` for superior rendering control (no artifacts/flickering) and adopts a strict "Clean Architecture" to prevent the "spaghetti code" issues common in long-lived TUI projects.
- **Rivet (Rust):** A fellow Rust client. While Rivet offers a solid experience, **Oxicord** specifically targets the "Power User" workflow with deeper Vim integration, specific optimizations for tiling window managers, and a visual style that favors information density (6-char timestamps, full selection) over standard layouts.

## Installation & Configuration

### Nix

```bash
nix run github:linuxmobile/oxicord
```

For development:

```bash
nix develop
```

### Building from Source

```bash
git clone https://github.com/linuxmobile/oxicord
cd oxicord
cargo build --release
./target/release/oxicord
```

### Configuration

Oxicord is currently configured via command-line arguments. Full support for a persistent `config.toml` file adhering to the XDG Base Directory specification is **in development**:

- **Linux:** `~/.config/oxicord/config.toml` (Planned)

## Roadmap

### Core Features & Stability

- [x] ~~Infinite scrolling / Auto-loading history~~
- [x] ~~Configurable keybindings~~
- [x] ~~Edit messages support~~
- [x] ~~Smart selection behavior on new messages~~
- [x] ~~Auto-focus message pane on channel selection~~
- [x] ~~Streamlined authentication (Token/Libsecret only)~~
- [x] ~~Connection status indicator fixes~~
- [x] ~~Performance optimizations (reduce CPU spikes)~~
- [ ] Forum channel support

### Visuals & UI

- [x] ~~Rich Markdown rendering in message pane~~
- [x] ~~Message reply previews~~
- [x] ~~Animated loading screen (TachyonFX)~~
- [x] ~~Unread indicators for guilds and channels~~
- [x] ~~Compact file picker UI~~
- [ ] Mention indicators for servers/channels and DMs
- [ ] UI Animations (Guild tree, Typing indicators via TachyonFX)
- [ ] Image modal viewer ('o' binding)
- [ ] Image previews (Ratatui-image integration) _(Monitoring for performance impact)_

### System & Documentation

- [x] ~~Native file explorer for attachments~~
- [x] ~~User mention support (@)~~
- [x] ~~Comprehensive documentation update~~
- [ ] XDG-compliant configuration support (`~/.config/oxicord/config.toml`)

## Credits

Oxicord is a fork and full rewrite of [Discordo](https://github.com/ayn2op/discordo). We express our sincere gratitude to the original maintainers for their work, which served as the foundation and inspiration for this project.
