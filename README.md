<img width="2560" height="1120" alt="oxicordbanner" src="https://github.com/user-attachments/assets/a66b4fe1-2270-459d-957c-d9620365044b" />

Oxicord is a high-performance, memory-safe Discord TUI client written in Rust. It is a spiritual successor to [Discordo](https://github.com/ayn2op/discordo), rewritten from scratch to leverage the performance and safety guarantees of Rust and the Ratatui ecosystem.

Designed for power users on Linux who demand speed, minimal resource usage, and terminal aesthetics, Oxicord aims to be the definitive CLI experience for Discord.

<p align="center" style="background-color: #2b0000; padding: 20px; border-left: 5px solid #ff0000; border-radius: 5px; color: #ffcccc;">
  <strong>⚠️ WARNING ⚠️</strong><br><br>
  Using unofficial clients is against Discord's Terms of Service. Use this client at your own risk. <strong>Never share your token with anyone.</strong>
</p>

## Why?
Static binary can work everywhere and it is faster.

## How?
1. Feature Flags System 
Made components optional for static builds:
- keyring - Optional system token storage
- notify - Optional desktop notifications  
- image - Optional image rendering
- clipboard - Always enabled but uses shell commands (no arboard/glib)

2. Static Build Support
- Switched from rustls to native-tls (OpenSSL/LibreSSL)
- Removed arboard dependency (required glib)
- Created shell-based clipboard using xclip/wl-copy
- Successfully built fully static binary on Alpine Linux

3. Clipboard Rewrite
- Before: Used arboard crate (glib dependency, hard to static link)
- After: Shell command wrapper around xclip/wl-copy
- set_text() / get_text() for text
- set_binary() / get_binary() for images/files
- No compile-time dependencies, minimal runtime deps

4. Conditional Compilation
- Added #[cfg(feature = "...")] throughout:
- Image loading code only compiled with image feature
- Keyring storage stub when disabled
- Image entity stubs when disabled

5. Merged Upstream Changes
- Fetched latest from original repo
- Resolved merge conflicts (kept both your features and upstream's new modules)
- Updated clipboard integration to match new upstream APIs

6. New Files Created
- src/domain/entities/image_stub.rs - Stub when image disabled
- src/presentation/widgets/image_state_stub.rs - Stub implementations
- src/infrastructure/storage/keyring_storage_stub.rs - Stub storage
- Updated README.md - Minimal build instructions

7. Files Modified
- Cargo.toml - Feature flags, removed arboard, changed TLS
- src/infrastructure/clipboard.rs - Complete rewrite
- src/infrastructure/mod.rs - Conditional exports
- src/presentation/ui/app.rs - Updated clipboard calls
- Multiple files with #[cfg(...)] attributes added

## Installation & Configuration

### Using release

### Using dbin package manager

```bash
   dbin install oxicord 
```

### Building from Source

**Prerequisites**

Ensure you have the latest stable Rust toolchain installed. You will also need the following system dependencies:

- **Alpine:**
  ```bash
  doas apk add -S pkgconf chafa-dev glib-static musl-dev clang rustup libressl-dev xclip wl-clipboard
  ```

  ```bash
  rustup-init
  ```

- **Build Steps**

```bash
git clone https://github.com/tecknian/oxicord-static
export RUSTFLAGS="-C link-arg=-lgcc"
cd oxicord-static
```

- **For fully static binary with image support (chafa needed)**

```bash
cargo build --release --no-default-features --features="image"
```

- **For fully static binary without image support**

```bash
cargo build --release --no-default-features
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
