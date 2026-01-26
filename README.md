<img width="2560" height="1120" alt="oxicordbanner" src="https://github.com/user-attachments/assets/a66b4fe1-2270-459d-957c-d9620365044b" />

Oxicord is a high-performance, memory-safe Discord TUI client written in Rust. It is a spiritual successor to [Discordo](https://github.com/ayn2op/discordo), rewritten from scratch to leverage the performance and safety guarantees of Rust and the Ratatui ecosystem.

Designed for power users on Linux who demand speed, minimal resource usage, and terminal aesthetics, Oxicord aims to be the definitive CLI experience for Discord.

<p align="center" style="background-color: #2b0000; padding: 20px; border-left: 5px solid #ff0000; border-radius: 5px; color: #ffcccc;">
  <strong>⚠️ WARNING ⚠️</strong><br><br>
  Using unofficial clients is against Discord's Terms of Service. Use this client at your own risk. <strong>Never share your token with anyone.</strong>
</p>

## Why Oxicord?

Oxicord distinguishes itself through a commitment to modern engineering principles and user experience:

- **Uncompromising Performance:** Built with Rust, Oxicord delivers instant startup times and negligible memory footprint compared to Electron or Go-based alternatives.
- **Clean Architecture:** The codebase follows strict Clean Architecture principles (Domain, Application, Infrastructure), making it robust, testable, and approachable for contributors.
- **TUI Fidelity:** Utilizing `ratatui` and `tachyonfx`, Oxicord provides a fluid, glitch-free interface with full mouse support and responsive layouts.

## Features

Oxicord implements a focused set of "real" features designed for daily drivers, prioritizing stability over bloat:

<p align="center">
  <img src="https://github.com/user-attachments/assets/4cd1909c-fc0f-419b-8e1f-ec2c0322d1d6" alt="final_showcase">
  <br>
  <sub><b>small showcase</b></sub>
</p>

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

**Prerequisites**

Ensure you have the latest stable Rust toolchain installed. You will also need the following system dependencies:

- **Debian/Ubuntu:**
  ```bash
  sudo apt install pkg-config libdbus-1-dev libchafa-dev libglib2.0-dev mold clang
  ```
- **Fedora:**
  ```bash
  sudo dnf install pkgconf-pkg-config dbus-devel chafa-devel glib2-devel mold clang
  ```
- **Arch Linux:**
  ```bash
  sudo pacman -S pkgconf chafa dbus glib2 mold clang
  ```
- **macOS:**
  ```bash
  brew install chafa
  ```

**Build Steps**

```bash
git clone https://github.com/linuxmobile/oxicord
cd oxicord
cargo build --release
./target/release/oxicord
```

### Configuration

Oxicord is currently configured via command-line arguments. Full support for a persistent `config.toml` file adhering to the XDG Base Directory specification is **in development**:

- **Linux:** `~/.config/oxicord/config.toml` (Planned)

## Authentication

Oxicord requires a Discord user token to function.

### Obtaining the Token

1. Log in to [Discord Web](https://discord.com/app) in your browser.
2. Open **Developer Tools** (`F12` or `Ctrl+Shift+I`) and go to the **Network** tab.
3. In the filter box, type `/api`.
4. Click on any channel to trigger a network request.
5. Select a request (e.g., `messages`, `typing`) and scroll to **Request Headers**.
6. Copy the value of the `authorization` header.

### Usage

**1. Secure Keyring (Recommended)**

Run Oxicord without arguments:

```bash
oxicord
```

Paste your token when prompted. Oxicord will verify and securely store it in your system's keyring (using `libsecret` on Linux, Keychain on macOS) for automatic future logins.

**2. Environment Variable**

For temporary sessions, testing, or scripts, you can provide the token via the environment. This takes precedence over the keyring and is **not** saved.

```bash
export OXICORD_TOKEN="your_token_here"
oxicord
```

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
- [x] ~~Forum channel support~~

### Visuals & UI

- [x] ~~Rich Markdown rendering in message pane~~
- [x] ~~Message reply previews~~
- [x] ~~Animated loading screen (TachyonFX)~~
- [x] ~~Unread indicators for guilds and channels~~
- [x] ~~Compact file picker UI~~
- [x] ~~Image previews (Ratatui-image: Sixel/Kitty/iTerm2)~~
- [x] ~~Guild Folders support~~
- [x] ~~Forum Channel support~~
- [ ] ~~Mention indicators for servers/channels and DMs~~
- [ ] UI Animations (Guild tree, Typing indicators via TachyonFX)
- [ ] Image modal viewer ('o' binding)

### System & Documentation

- [x] ~~Native file explorer for attachments~~
- [x] ~~User mention support (@)~~
- [x] ~~Comprehensive documentation update~~
- [x] ~~Desktop Notifications~~
- [ ] XDG-compliant configuration support (`~/.config/oxicord/config.toml`)

## What's Next

- **Navigation:** `Ctrl+K` fuzzy finder for channels and DM users.
- **Configuration:** Full `config.toml` support for custom keybinds and behavior.
- **Privacy:** Option to hide messages from blocked users.
- **Media Interaction:**
  - 'Y' keybinding to copy images to clipboard.
  - 'o' keybinding to open images in a High-Res modal.
- **Bot Support:** Native support for slash commands and bot interactions.
- **Performance:** Targeting a further +20% improvement in API response parsing.

## Credits

Oxicord is a fork and full rewrite of [Discordo](https://github.com/ayn2op/discordo). We express our sincere gratitude to the original maintainers for their work, which served as the foundation and inspiration for this project.
