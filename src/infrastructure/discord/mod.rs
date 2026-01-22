//! Discord API client.

mod client;
mod dto;
pub mod gateway;
pub mod identity;
pub mod scraper;

pub use client::DiscordClient;
pub use gateway::{
    DispatchEvent, GatewayClient, GatewayClientConfig, GatewayCommand, GatewayEventKind,
    GatewayIntents, PresenceStatus, TypingIndicatorManager, TypingIndicatorState, TypingUser,
};
