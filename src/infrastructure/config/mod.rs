//! Application configuration.

mod app_config;
pub mod args;
pub mod state_config;
pub mod storage;

pub use app_config::{AppConfig, LogLevel, NotificationsConfig, ThemeConfig, UiConfig};
pub use args::CliArgs;
pub use state_config::StateConfig;
pub use storage::StorageManager;
