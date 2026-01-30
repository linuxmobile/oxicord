//! Application configuration.

use clap::Parser;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const APP_NAME: &str = "oxicord";
const APP_QUALIFIER: &str = "com";
const APP_ORGANIZATION: &str = "ayn2op";

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
#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(
    name = "oxicord",
    version,
    about = "A lightweight, secure Discord terminal client",
    long_about = None
)]
pub struct AppConfig {
    /// Discord authentication token.
    #[arg(short, long, env = "OXICORD_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// Configuration file path.
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Log file path.
    #[arg(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,

    /// Log verbosity level.
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Enable mouse support.
    #[arg(long, default_value_t = true)]
    pub mouse: bool,

    /// Enable desktop notifications.
    #[arg(long, default_value_t = true)]
    #[serde(default = "default_true")]
    pub enable_desktop_notifications: bool,

    /// Disable user colors (monochrome mode).
    #[arg(long, default_value_t = false)]
    pub disable_user_colors: bool,

    /// UI configuration.
    #[command(flatten)]
    #[serde(default)]
    pub ui: UiConfig,

    /// Theme configuration.
    #[command(flatten)]
    pub theme: ThemeConfig,
}

/// UI configuration.
#[derive(Debug, Clone, Default, clap::Args, Serialize, Deserialize)]
pub struct UiConfig {
    /// Group guilds into folders.
    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub group_guilds: bool,

    /// Use display name (Global Name) instead of username where available.
    #[arg(long, default_value_t = true)]
    #[serde(default = "default_true")]
    pub use_display_name: bool,
}

/// Theme configuration.
#[derive(Debug, Clone, clap::Args, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Accent color (name or hex code).
    #[arg(long, default_value = "Yellow")]
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
}

fn default_accent_color() -> String {
    "Yellow".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent_color: default_accent_color(),
        }
    }
}

impl AppConfig {
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
            token: None,
            config: None,
            log_path: None,
            log_level: LogLevel::Info,
            mouse: true,
            enable_desktop_notifications: true,
            disable_user_colors: false,
            ui: UiConfig::default(),
            theme: ThemeConfig::default(),
        }
    }
}
