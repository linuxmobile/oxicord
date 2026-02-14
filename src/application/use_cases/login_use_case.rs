//! Login use case implementation.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::application::dto::{LoginRequest, LoginResponse};
use crate::domain::entities::AuthToken;
use crate::domain::errors::AuthError;
use crate::domain::ports::{AuthPort, TokenStoragePort};

/// Handles user authentication workflow.
#[derive(Clone)]
pub struct LoginUseCase {
    auth_port: Arc<dyn AuthPort>,
    storage_port: Arc<dyn TokenStoragePort>,
}

impl LoginUseCase {
    /// Creates new login use case.
    #[must_use]
    pub const fn new(
        auth_port: Arc<dyn AuthPort>,
        storage_port: Arc<dyn TokenStoragePort>,
    ) -> Self {
        Self {
            auth_port,
            storage_port,
        }
    }

    /// Executes login with provided request.
    ///
    /// # Errors
    /// Returns error if token is invalid or rejected.
    pub async fn execute(&self, request: LoginRequest) -> Result<LoginResponse, AuthError> {
        debug!(source = %request.source, "Attempting login");

        let token = AuthToken::new(&request.token).ok_or_else(|| {
            warn!("Invalid token format provided");
            AuthError::invalid_format("token does not match expected Discord token format")
        })?;

        debug!("Token format validated, checking with Discord API");

        let user = self.auth_port.validate_token(&token).await.map_err(|e| {
            warn!(error = %e, "Token validation failed");
            e
        })?;

        info!(
            user_id = %user.id(),
            username = %user.username(),
            "Successfully authenticated"
        );

        let token_persisted = if request.persist_token {
            match self.storage_port.store_token(&token).await {
                Ok(()) => {
                    info!("Token persisted to secure storage");
                    true
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to persist token to secure storage");
                    false
                }
            }
        } else {
            debug!("Token persistence disabled, skipping storage");
            false
        };

        Ok(LoginResponse::new(user, request.source, token_persisted))
    }

    /// Deletes the stored token.
    ///
    /// # Errors
    /// Returns error if deletion fails.
    pub async fn delete_token(&self) -> Result<(), AuthError> {
        debug!("Deleting token from secure storage");
        match self.storage_port.delete_token().await {
            Ok(()) => {
                info!("Token deleted from secure storage");
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to delete token from secure storage");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::dto::TokenSource;
    use crate::domain::ports::mocks::{MockAuthPort, MockTokenStorage};

    fn make_valid_token() -> String {
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.XXXXXX.YYYYYYYYYYYYYYYYYYYYYYYYYYYY".to_string()
    }

    #[tokio::test]
    async fn test_successful_login() {
        let auth_port = Arc::new(MockAuthPort::new(true));
        let storage_port = Arc::new(MockTokenStorage::new());

        let use_case = LoginUseCase::new(auth_port, storage_port.clone());
        let request = LoginRequest::new(make_valid_token(), TokenSource::Environment);

        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.user.username(), "testuser");
        assert!(response.token_persisted);

        assert!(storage_port.has_token().await.unwrap());
    }

    #[tokio::test]
    async fn test_invalid_token_format() {
        let auth_port = Arc::new(MockAuthPort::new(true));
        let storage_port = Arc::new(MockTokenStorage::new());

        let use_case = LoginUseCase::new(auth_port, storage_port);
        let request = LoginRequest::new("invalid".to_string(), TokenSource::UserInput);

        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(AuthError::InvalidTokenFormat { .. })));
    }

    #[tokio::test]
    async fn test_rejected_token() {
        let auth_port = Arc::new(MockAuthPort::new(false));
        let storage_port = Arc::new(MockTokenStorage::new());

        let use_case = LoginUseCase::new(auth_port, storage_port);
        let request = LoginRequest::new(make_valid_token(), TokenSource::UserInput);

        let result = use_case.execute(request).await;

        assert!(matches!(result, Err(AuthError::TokenRejected { .. })));
    }

    #[tokio::test]
    async fn test_login_without_persistence() {
        let auth_port = Arc::new(MockAuthPort::new(true));
        let storage_port = Arc::new(MockTokenStorage::new());

        let use_case = LoginUseCase::new(auth_port, storage_port.clone());
        let request =
            LoginRequest::new(make_valid_token(), TokenSource::Environment).without_persistence();

        let result = use_case.execute(request).await;

        assert!(result.is_ok());
        assert!(!result.unwrap().token_persisted);
        assert!(!storage_port.has_token().await.unwrap());
    }
}
