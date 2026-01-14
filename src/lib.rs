//! Discordo - A lightweight Discord terminal client.
//!
//! This crate provides a terminal-based Discord client with clean architecture,
//! implementing authentication, token management, and a TUI interface.

#![allow(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

/// Application layer containing use cases and DTOs.
pub mod application;
/// Domain layer containing entities, errors, and port definitions.
pub mod domain;
/// Infrastructure layer containing adapters for external services.
pub mod infrastructure;
/// Presentation layer containing UI components and event handling.
pub mod presentation;

/// Current version of the application.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name.
pub const NAME: &str = "discordo";
