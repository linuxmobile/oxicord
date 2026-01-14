//! Token storage port definition.

use async_trait::async_trait;

use crate::domain::entities::AuthToken;
use crate::domain::errors::AuthError;

/// Port for token persistence operations.
#[async_trait]
pub trait TokenStoragePort: Send + Sync {
    /// Retrieves stored token.
    async fn get_token(&self) -> Result<Option<AuthToken>, AuthError>;

    /// Stores token securely.
    async fn store_token(&self, token: &AuthToken) -> Result<(), AuthError>;

    /// Deletes stored token.
    async fn delete_token(&self) -> Result<(), AuthError>;

    /// Checks if token exists.
    async fn has_token(&self) -> Result<bool, AuthError> {
        Ok(self.get_token().await?.is_some())
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Mock token storage for testing.
    pub struct MockTokenStorage {
        token: Arc<RwLock<Option<AuthToken>>>,
    }

    impl MockTokenStorage {
        /// Creates empty mock storage.
        #[must_use]
        pub fn new() -> Self {
            Self {
                token: Arc::new(RwLock::new(None)),
            }
        }

        /// Creates mock storage with token.
        #[must_use]
        pub fn with_token(token: AuthToken) -> Self {
            Self {
                token: Arc::new(RwLock::new(Some(token))),
            }
        }
    }

    impl Default for MockTokenStorage {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl TokenStoragePort for MockTokenStorage {
        async fn get_token(&self) -> Result<Option<AuthToken>, AuthError> {
            Ok(self.token.read().await.clone())
        }

        async fn store_token(&self, token: &AuthToken) -> Result<(), AuthError> {
            *self.token.write().await = Some(token.clone());
            Ok(())
        }

        async fn delete_token(&self) -> Result<(), AuthError> {
            *self.token.write().await = None;
            Ok(())
        }
    }
}
