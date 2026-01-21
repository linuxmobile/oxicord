//! Authentication port definition.

use async_trait::async_trait;

use crate::domain::entities::{AuthToken, User};
use crate::domain::errors::AuthError;

/// Port for Discord authentication operations.
#[async_trait]
pub trait AuthPort: Send + Sync {
    /// Validates token and returns user information.
    async fn validate_token(&self, token: &AuthToken) -> Result<User, AuthError>;

    /// Checks Discord API availability.
    async fn health_check(&self) -> Result<(), AuthError>;
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock authentication port for testing.
    pub struct MockAuthPort {
        should_succeed: Arc<AtomicBool>,
        user: User,
    }

    impl MockAuthPort {
        /// Creates new mock.
        #[must_use]
        pub fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed: Arc::new(AtomicBool::new(should_succeed)),
                user: User::new("123", "testuser", "0", None, false, None),
            }
        }

        /// Sets success behavior.
        pub fn set_should_succeed(&self, value: bool) {
            self.should_succeed.store(value, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl AuthPort for MockAuthPort {
        async fn validate_token(&self, _token: &AuthToken) -> Result<User, AuthError> {
            if self.should_succeed.load(Ordering::SeqCst) {
                Ok(self.user.clone())
            } else {
                Err(AuthError::rejected("mock rejection"))
            }
        }

        async fn health_check(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }
}
