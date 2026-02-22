use super::app_config::LogLevel;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "oxicord",
    version,
    about = "A lightweight, secure Discord terminal client",
    long_about = None
)]
pub struct CliArgs {
    /// Configuration file path.
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Log file path.
    #[arg(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,

    /// Log verbosity level.
    #[arg(long, value_enum)]
    pub log_level: Option<LogLevel>,

    /// Enable mouse support.
    #[arg(long)]
    pub mouse: Option<bool>,

    /// Enable desktop notifications.
    #[arg(long)]
    pub enable_desktop_notifications: Option<bool>,

    /// Disable user colors (monochrome mode).
    #[arg(long)]
    pub disable_user_colors: Option<bool>,

    /// Group guilds into folders.
    #[arg(long)]
    pub group_guilds: Option<bool>,

    /// Use display name (Global Name) instead of username where available.
    #[arg(long)]
    pub use_display_name: Option<bool>,

    /// Notification duration in seconds.
    #[arg(long)]
    pub notification_duration: Option<u64>,

    /// Show typing indicators.
    #[arg(long)]
    pub show_typing: Option<bool>,

    /// Enable internal TUI notifications.
    #[arg(long)]
    pub internal_notifications: Option<bool>,

    /// Enable `TachyonFX` animations.
    #[arg(long)]
    pub enable_animations: Option<bool>,

    /// Accent color (name or hex code).
    #[arg(long)]
    pub accent_color: Option<String>,
}
