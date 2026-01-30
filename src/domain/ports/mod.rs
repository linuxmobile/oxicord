mod auth_port;
mod discord_data_port;
mod gateway_port;
mod image_cache_port;
mod mention_resolver_port;
mod notification_port;
mod token_storage_port;

pub use auth_port::AuthPort;
pub use discord_data_port::{
    DirectMessageChannel, DiscordDataPort, EditMessageRequest, FetchMessagesOptions,
    SendMessageRequest,
};
pub use gateway_port::{GatewayEvent, GatewayPort};
pub use image_cache_port::{CacheError, CacheResult, ImageCachePort, ImageLoaderPort};
pub use mention_resolver_port::MentionResolver;
pub use notification_port::NotificationPort;
pub use token_storage_port::TokenStoragePort;

#[cfg(test)]
pub mod mocks {
    pub use super::auth_port::mock::MockAuthPort;
    pub use super::token_storage_port::mock::MockTokenStorage;
}
