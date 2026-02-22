//! Authentication DTOs.

use crate::domain::entities::User;

/// Source of the authentication token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    /// Token from environment variable.
    Environment,
    /// Token from system keyring.
    Keyring,
    /// Token entered by user.
    UserInput,
}

impl TokenSource {
    /// Returns human-readable description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Environment => "environment variable",
            Self::Keyring => "system keyring",
            Self::UserInput => "user input",
        }
    }
}

impl std::fmt::Display for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Login request data.
#[derive(Debug, Clone)]
pub struct LoginRequest {
    /// Authentication token.
    pub token: String,
    /// Token source.
    pub source: TokenSource,
    /// Whether to persist token.
    pub persist_token: bool,
}

impl LoginRequest {
    /// Creates new login request.
    #[must_use]
    pub const fn new(token: String, source: TokenSource) -> Self {
        Self {
            token,
            source,
            persist_token: true,
        }
    }

    /// Disables token persistence.
    #[must_use]
    pub const fn without_persistence(mut self) -> Self {
        self.persist_token = false;
        self
    }
}

/// Login response data.
#[derive(Debug, Clone)]
pub struct LoginResponse {
    /// Authenticated user.
    pub user: User,
    /// Token source used.
    pub token_source: TokenSource,
    /// Whether token was persisted.
    pub token_persisted: bool,
}

impl LoginResponse {
    /// Creates new login response.
    #[must_use]
    pub const fn new(user: User, token_source: TokenSource, token_persisted: bool) -> Self {
        Self {
            user,
            token_source,
            token_persisted,
        }
    }
}
