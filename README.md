<img width="2560" height="1120" alt="oxicordbanner" src="https://github.com/user-attachments/assets/a66b4fe1-2270-459d-957c-d9620365044b" />

# 錆緒 (Oxicord)

> A tether forged in oxide, weaving the digital void.

**Oxicord** (錆緒, _sabio_) translates to "Oxide Cord." It is a high-performance, memory-safe Discord TUI client crafted in Rust. Born from the philosophy of _Sabi_ (錆)—where beauty is found in the weathered and the enduring—it honors the strength of its codebase and the stillness of the terminal.

It is an _atelier_ for the modern power user, designed for those who demand the precision of a forged blade and the quietude of a sanctuary.

### ⚠ <sup><sub><samp>The use of unofficial clients resides outside the established paths of Discord's Terms of Service. Journey at your own risk. <strong>The sanctity of your token must be guarded.</samp></sub></sup>

## The Philosophy of Stillness

Oxicord is not merely built; it is curated. It adheres to a strict architecture where the logic is pure and the interface recedes to let the dialogue breathe.

- **Oxidized Strength:** Leverages the safety and speed of Rust to ensure a footprint as light as fallen snow.
- **Architectural Intent:** Follows Clean Architecture principles, ensuring the core logic remains untainted by external noise.
- **Visual Harmony:** Utilizes `ratatui` and _TachyonFX_ to provide a fluid, artifact-free experience that respects the terminal's aesthetic.

## The Atelier of Features

A comprehensive collection of intentional tools, forged through continuous refinement.

<p align="center">
  <img src="https://github.com/user-attachments/assets/4cd1909c-fc0f-419b-8e1f-ec2c0322d1d6" alt="final_showcase">
  <br>
  <sub><b>a glimpse of the atelier</b></sub>
</p>

### 織 | The Evolving Loom (Performance & Logic)

- **O(1) Message Geometry:** A revolutionary layout strategy that ensures appending messages remains instant, regardless of the channel's history weight.
- **Stealth Transport:** Implements strict identity enforcement to mimic official client behavior, moving silently through the network.
- **Permission-Based Clarity:** The interface honors your Discord permissions, showing only the paths you are permitted to walk.
- **Concurrent Integrity:** Powered by `tokio`, ensuring a non-blocking, responsive experience even in the busiest environments.

### 幽 | Visual Fidelity (Interface & Rendering)

- **Inline Image Mastery:** High-resolution previews via `ratatui-image` and `chafa`, supporting Sixel, Kitty, iTerm2, and high-fidelity ANSI approximations.
- **Visual Grouping:** Consecutive messages from the same author are visually gathered, creating a rhythmic and clean dialogue flow.
- **Dynamic Theming:** A theme engine with HSL support and role-based coloring for usernames, ensuring every voice is distinct.
- **Animated Rituals:** Subtle, non-intrusive animations powered by _TachyonFX_ for splash screens and state transitions.

### 間 | Spatial Awareness (Navigation & Search)

- **Enhanced Quick Switcher:** A fuzzy-search powerhouse with dynamic sorting that remembers your most frequent paths across sessions.
- **Server Tree Ritual:** A native custom tree implementation for guilds and channels, designed for spatial intuition without emoji clutter.
- **Vim-like Fluidity:** Navigation via `j`/`k` and `g`/`G` feels as natural as breathing.
- **Integrated Explorer:** A modal file picker with fuzzy search for attaching files and media without leaving the terminal.

### 緒 | The Social Cord (Communication)

- **Rich Typography:** Full Markdown support with syntax highlighting for code blocks, rendered with precision.
- **Intelligent Autocomplete:** A context-aware system for `@mentions` that anticipates your intent.
- **External Reflection:** Use the `o` keybinding to open links and images, or invoke your `$EDITOR` for long-form composition.
- **Infinite Scroll:** History fetches automatically as you ascend, removing the friction of manual loading.
- **Presence Indicators:** Real-time typing feedback and unread markers keep you anchored to the present.

## Fair Play

We honor those who paved the way. Oxicord is a single thread in a larger tapestry of terminal excellence.

- **Regarding Endcord:** A venerable beast of features and extensibility. While it offers a vast toolkit, Oxicord chooses a different path—prioritizing the raw performance and "crash-proof" reliability of the Rust language.
- **Regarding Discordo:** The original spark. Discordo pioneered the TUI layout we admire. Oxicord seeks to evolve this vision through a deeper architectural rigor and the rendering precision of the Ratatui ecosystem.
- **Regarding Rivet:** A fellow traveler in the Rust landscape. Oxicord distinguishes itself through a focus on high-density information and specific optimizations for those who live within tiling window managers.

## Installation

### The Arch Way

Oxicord can be installed from the AUR.
You can choose between:
- the latest release version package: [oxicord](https://aur.archlinux.org/packages/oxicord)
- the git package (to build the latest commit on main branch): [oxicord-git](https://aur.archlinux.org/packages/oxicord-git)
- the bin package (to avoid compiling): [oxicord-bin](https://aur.archlinux.org/packages/oxicord-bin)

For example:
```bash
paru -S oxicord
```

### The Nix Ritual

```bash
nix run github:linuxmobile/oxicord
```

### Forging from Source

**Prerequisites:** pkg-config, dbus, chafa, glib2, mold, clang.

```bash
git clone https://github.com/linuxmobile/oxicord
cd oxicord
cargo build --release
./target/release/oxicord
```

## Configuration

Oxicord honors the XDG Base Directory specification. Your configuration resides in `~/.config/oxicord/config.toml`.

```toml
# A fragment of the Oxicord config.toml
log_level = "info"
mouse = true
quick_switcher_order = "recents"

[ui]
group_guilds = false
use_display_name = true
image_preview = true
timestamp_format = "%H:%M"
enable_animations = true

[notifications]
desktop = true
internal = true

[theme]
accent_color = "Yellow"
mode = "dark"
```

## Authentication

Authentication is a sacred link. Oxicord provides two paths, but first, you must embark on a ritual of discovery to retrieve your unique token.

### Ritual of Discovery

To anchor yourself to the Discord network, you must retrieve your unique signature—your token.

1.  **Enter the Web Interface:** Log in to [Discord](https://discord.com/app) via a web browser.
2.  **Open the Developer Console:** Press `F12` or `Ctrl+Shift+I` to reveal the inner workings.
3.  **The Network Path:** Select the **Network** tab and filter for `/api`.
4.  **A Moment of Interaction:** Click on any channel or trigger a message to create a pulse in the network.
5.  **Retrieve the Essence:** Inspect any request (e.g., `messages`) and find the `authorization` header. Copy this value—it is your key to the void.

### The Path to Connection

1.  **Keyring Storage (Recommended):** Run `oxicord` and provide your token when prompted. It will be securely guarded by your system's keyring (Keychain/libsecret).
2.  **Environment Variable:** For temporary sessions, use `export OXICORD_TOKEN="..."`.

## Credits

Oxicord is a full rewrite of [Discordo](https://github.com/ayn2op/discordo). We extend our deepest gratitude to the original maintainers; their work provided the foundation upon which this sanctuary was built.

---

_One thread, weathered by time, binding the digital horizon._
