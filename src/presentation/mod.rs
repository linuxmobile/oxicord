//! Presentation layer with UI components and event handling.

/// Command handling.
pub mod commands;
/// Event handling.
pub mod events;
/// Presentation services (rendering, highlighting).
pub mod services;
/// Theme logic.
pub mod theme;
/// UI screens.
pub mod ui;
/// Reusable widgets.
pub mod widgets;

pub use theme::Theme;
pub use ui::{App, AppConfig};
