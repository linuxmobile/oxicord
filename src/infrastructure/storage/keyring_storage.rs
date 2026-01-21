//! Keyring-based token storage.

use async_trait::async_trait;
use keyring::Entry;
use tracing::{debug, warn};

use crate::domain::entities::AuthToken;
use crate::domain::errors::{AuthError, SecretError};
use crate::domain::ports::TokenStoragePort;

const KEYRING_SERVICE: &str = "oxicord";
const KEYRING_USER: &str = "default";

/// System keyring token storage adapter.
pub struct KeyringTokenStorage {
    service: String,
    user: String,
}

impl KeyringTokenStorage {
    /// Creates new storage with default names.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: KEYRING_SERVICE.to_string(),
            user: KEYRING_USER.to_string(),
        }
    }

    /// Creates storage with custom names.
    #[must_use]
    pub fn with_names(service: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            user: user.into(),
        }
    }
}

impl Default for KeyringTokenStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenStoragePort for KeyringTokenStorage {
    async fn get_token(&self) -> Result<Option<AuthToken>, AuthError> {
        debug!(service = %self.service, "Retrieving token from keyring");

        let service = self.service.clone();
        let user = self.user.clone();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &user)
                .map_err(|e| SecretError::AccessFailed(e.to_string()))?;

            match entry.get_password() {
                Ok(password) => {
                    debug!("Token found in keyring");
                    Ok(AuthToken::new(&password))
                }
                Err(keyring::Error::NoEntry) => {
                    debug!("No token stored in keyring");
                    Ok(None)
                }
                Err(e) => {
                    warn!(error = %e, "Failed to retrieve token from keyring");
                    Err(SecretError::RetrievalFailed(e.to_string()).into())
                }
            }
        })
        .await
        .map_err(|e| AuthError::Unexpected {
            message: e.to_string(),
        })?
    }

    async fn store_token(&self, token: &AuthToken) -> Result<(), AuthError> {
        debug!(service = %self.service, "Storing token in keyring");

        let service = self.service.clone();
        let user = self.user.clone();
        let token = token.clone();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &user)
                .map_err(|e| SecretError::AccessFailed(e.to_string()))?;

            entry.set_password(token.as_str()).map_err(|e| {
                warn!(error = %e, "Failed to store token in keyring");
                SecretError::StorageFailed(e.to_string())
            })?;

            debug!("Token stored successfully");
            Ok(())
        })
        .await
        .map_err(|e| AuthError::Unexpected {
            message: e.to_string(),
        })?
    }

    async fn delete_token(&self) -> Result<(), AuthError> {
        debug!(service = %self.service, "Deleting token from keyring");

        let service = self.service.clone();
        let user = self.user.clone();

        tokio::task::spawn_blocking(move || {
            let entry = Entry::new(&service, &user)
                .map_err(|e| SecretError::AccessFailed(e.to_string()))?;

            match entry.delete_credential() {
                Ok(()) => {
                    debug!("Token deleted from keyring");
                    Ok(())
                }
                Err(keyring::Error::NoEntry) => {
                    debug!("No token to delete");
                    Ok(())
                }
                Err(e) => {
                    warn!(error = %e, "Failed to delete token from keyring");
                    Err(SecretError::DeletionFailed(e.to_string()).into())
                }
            }
        })
        .await
        .map_err(|e| AuthError::Unexpected {
            message: e.to_string(),
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires system keyring"]
    async fn test_store_and_retrieve_token() {
        let storage = KeyringTokenStorage::with_names("oxicord-test", "test-token");
        let token = AuthToken::new_unchecked(
            "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.XXXXXX.YYYYYYYYYYYYYYYYYYYYYYYYYYYY",
        );

        storage.store_token(&token).await.unwrap();

        let retrieved = storage.get_token().await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().as_str(), token.as_str());

        storage.delete_token().await.unwrap();
    }
}
