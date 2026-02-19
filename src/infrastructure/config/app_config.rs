//! Application configuration.

use crate::domain::keybinding::Action;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const APP_NAME: &str = "oxicord";
const APP_QUALIFIER: &str = "com";
const APP_ORGANIZATION: &str = "linuxmobile";

/// Log level configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Trace level.
    Trace,
    /// Debug level.
    Debug,
    /// Info level.
    #[default]
    Info,
    /// Warning level.
    Warn,
    /// Error level.
    Error,
}

impl LogLevel {
    /// Converts to tracing level.
    #[must_use]
    pub const fn to_tracing_level(self) -> tracing::Level {
        match self {
            Self::Trace => tracing::Level::TRACE,
            Self::Debug => tracing::Level::DEBUG,
            Self::Info => tracing::Level::INFO,
            Self::Warn => tracing::Level::WARN,
            Self::Error => tracing::Level::ERROR,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Application configuration from CLI.
#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration file path.
    #[serde(skip)]
    pub config: Option<PathBuf>,

    /// Log file path.
    #[serde(skip)]
    pub log_path: Option<PathBuf>,

    /// Log verbosity level.
    #[serde(default)]
    pub log_level: LogLevel,

    /// Enable mouse support.
    #[serde(default = "default_true")]
    pub mouse: bool,

    /// Disable user colors (monochrome mode).
    #[serde(default)]
    pub disable_user_colors: bool,

    /// Editor command to use for file viewing/editing.
    /// Overrides $EDITOR environment variable.
    #[serde(default)]
    pub editor: Option<String>,

    /// Custom keybindings.
    #[serde(default)]
    pub keybindings: HashMap<String, Action>,

    /// UI configuration.
    #[serde(default)]
    pub ui: UiConfig,

    /// Notification configuration.
    #[serde(default)]
    pub notifications: NotificationsConfig,

    /// Quick Switcher sort mode (Recents, Mixed).
    #[serde(default)]
    pub quick_switcher_order: QuickSwitcherSortMode,

    /// Theme configuration.
    #[serde(default)]
    pub theme: ThemeConfig,
}

/// UI configuration.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Group guilds into folders.
    #[serde(default)]
    pub group_guilds: bool,

    /// Use display name (Global Name) instead of username where available.
    #[serde(default = "default_true")]
    pub use_display_name: bool,

    /// Show image previews in chat.
    #[serde(default = "default_true")]
    pub image_preview: bool,

    /// Timestamp format string (chrono format).
    #[serde(default = "default_timestamp_format")]
    pub timestamp_format: String,

    /// Show typing indicators.
    #[serde(default = "default_true")]
    pub show_typing: bool,

    /// Enable `TachyonFX` animations.
    #[serde(default = "default_true")]
    pub enable_animations: bool,

    /// Notification duration in seconds.
    #[serde(default = "default_notification_duration")]
    pub notification_duration: u64,

    /// If true, hide messages from blocked users completely instead of showing a placeholder.
    #[serde(default)]
    pub hide_blocked_completely: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            group_guilds: false,
            use_display_name: true,
            image_preview: true,
            timestamp_format: default_timestamp_format(),
            show_typing: true,
            enable_animations: true,
            notification_duration: 5,
            hide_blocked_completely: false,
        }
    }
}

/// Notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    /// Enable desktop notifications.
    #[serde(default = "default_true", alias = "enabled")]
    pub desktop: bool,

    /// Enable internal TUI notifications.
    #[serde(default = "default_true", alias = "internal_notifications")]
    pub internal: bool,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            desktop: true,
            internal: true,
        }
    }
}

/// Quick Switcher sorting strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuickSwitcherSortMode {
    #[default]
    Recents,
    Mixed,
}

impl std::fmt::Display for QuickSwitcherSortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recents => write!(f, "Recents"),
            Self::Mixed => write!(f, "Mixed"),
        }
    }
}

/// Theme mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Dark mode (default).
    #[default]
    Dark,
    /// Light mode.
    Light,
    /// Auto detect from system.
    Auto,
}

/// Theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Accent color (name or hex code).
    #[serde(default = "default_accent_color")]
    pub accent_color: String,

    /// Mention background color (name or hex code).
    #[serde(default)]
    pub mention_color: Option<String>,

    /// Theme mode (Dark, Light, Auto).
    #[serde(default)]
    pub mode: ThemeMode,
}

fn default_accent_color() -> String {
    "Yellow".to_string()
}

fn default_timestamp_format() -> String {
    "%H:%M".to_string()
}

fn default_true() -> bool {
    true
}

fn default_notification_duration() -> u64 {
    5
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent_color: default_accent_color(),
            mention_color: None,
            mode: ThemeMode::default(),
        }
    }
}

use super::args::CliArgs;

impl AppConfig {
    /// Merges CLI arguments into the configuration.
    pub fn merge_with_args(&mut self, args: CliArgs) {
        if let Some(config_path) = args.config {
            self.config = Some(config_path);
        }
        if let Some(log_path) = args.log_path {
            self.log_path = Some(log_path);
        }
        if let Some(log_level) = args.log_level {
            self.log_level = log_level;
        }
        if let Some(mouse) = args.mouse {
            self.mouse = mouse;
        }
        if let Some(notifications) = args.enable_desktop_notifications {
            self.notifications.desktop = notifications;
        }
        if let Some(disable_colors) = args.disable_user_colors {
            self.disable_user_colors = disable_colors;
        }
        if let Some(group_guilds) = args.group_guilds {
            self.ui.group_guilds = group_guilds;
        }
        if let Some(use_display_name) = args.use_display_name {
            self.ui.use_display_name = use_display_name;
        }
        if let Some(notification_duration) = args.notification_duration {
            self.ui.notification_duration = notification_duration;
        }
        if let Some(accent_color) = args.accent_color {
            self.theme.accent_color = accent_color;
        }
        if let Some(show_typing) = args.show_typing {
            self.ui.show_typing = show_typing;
        }
        if let Some(internal_notifications) = args.internal_notifications {
            self.notifications.internal = internal_notifications;
        }
        if let Some(enable_animations) = args.enable_animations {
            self.ui.enable_animations = enable_animations;
        }
    }

    /// Returns default config directory.
    #[must_use]
    pub fn default_config_dir() -> Option<PathBuf> {
        ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Returns default config file path.
    #[must_use]
    pub fn default_config_path() -> Option<PathBuf> {
        Self::default_config_dir().map(|dir| dir.join("config.toml"))
    }

    /// Returns default log file path.
    #[must_use]
    pub fn default_log_path() -> Option<PathBuf> {
        ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .map(|dirs| dirs.data_dir().join("oxicord.log"))
    }

    /// Returns effective config path.
    #[must_use]
    pub fn effective_config_path(&self) -> Option<PathBuf> {
        self.config.clone().or_else(Self::default_config_path)
    }

    /// Returns effective log path.
    #[must_use]
    pub fn effective_log_path(&self) -> Option<PathBuf> {
        self.log_path.clone().or_else(Self::default_log_path)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config: None,
            log_path: None,
            log_level: LogLevel::Info,
            mouse: true,
            disable_user_colors: false,
            editor: None,
            keybindings: HashMap::new(),
            ui: UiConfig::default(),
            notifications: NotificationsConfig::default(),
            quick_switcher_order: QuickSwitcherSortMode::default(),
            theme: ThemeConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::keybinding::Action;

    #[test]
    fn test_parse_config_with_new_fields() {
        let toml_content = r#"
            editor = "nvim"

            [ui]
            enable_animations = false

            [notifications]
            internal_notifications = false

            [keybindings]
            "Ctrl+q" = "Quit"
            "Alt+Enter" = "SendMessage"
        "#;

        let config: AppConfig = toml::from_str(toml_content).expect("Failed to parse config");

        assert_eq!(config.editor, Some("nvim".to_string()));
        assert!(!config.ui.enable_animations);
        assert!(!config.notifications.internal);
        assert_eq!(
            config.quick_switcher_order,
            QuickSwitcherSortMode::default()
        );

        assert_eq!(config.keybindings.len(), 2);
        assert_eq!(config.keybindings.get("Ctrl+q"), Some(&Action::Quit));
        assert_eq!(
            config.keybindings.get("Alt+Enter"),
            Some(&Action::SendMessage)
        );
    }

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();

        assert_eq!(config.editor, None);
        assert!(config.keybindings.is_empty());
        assert!(config.ui.enable_animations); // default_true
        assert!(config.notifications.internal); // default_true
    }
}
