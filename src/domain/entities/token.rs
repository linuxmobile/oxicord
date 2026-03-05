//! Discord authentication token value object.

use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Discord authentication token with validation and masking.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct AuthToken {
    value: String,
}

impl AuthToken {
    const MIN_TOKEN_LENGTH: usize = 50;

    /// Creates new token with format validation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();

        if value.len() < Self::MIN_TOKEN_LENGTH {
            return None;
        }

        if value.split('.').count() != 3 {
            return None;
        }

        Some(Self { value })
    }

    /// Creates token without validation.
    #[must_use]
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// Returns token as string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Consumes token and returns inner string.
    ///
    /// # Safety
    /// The returned `String` is NOT zeroized on drop.
    /// Callers should ensure the string is short-lived or explicitly zeroed if possible.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.value.clone()
    }

    /// Returns dummy token for testing.
    #[cfg(test)]
    #[must_use]
    pub fn dummy() -> String {
        "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.XXXXXX.YYYYYYYYYYYYYYYYYYYYYYYYYYYY".to_string()
    }

    /// Returns masked token for display.
    #[must_use]
    pub fn masked(&self) -> String {
        if self.value.len() <= 10 {
            return "*".repeat(self.value.len());
        }

        let visible_prefix = &self.value[..4];
        let visible_suffix = &self.value[self.value.len() - 4..];
        format!("{visible_prefix}...{visible_suffix}")
    }
}

impl fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthToken")
            .field("value", &self.masked())
            .finish()
    }
}

impl fmt::Display for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_token_creation() {
        let token = AuthToken::new(AuthToken::dummy());
        assert!(token.is_some());
    }

    #[test]
    fn test_invalid_token_too_short() {
        let token = AuthToken::new("short");
        assert!(token.is_none());
    }

    #[test]
    fn test_invalid_token_wrong_format() {
        let token = AuthToken::new("a".repeat(60));
        assert!(token.is_none());
    }

    #[test]
    fn test_token_masking() {
        let dummy = AuthToken::dummy();
        let token = AuthToken::new_unchecked(AuthToken::dummy());
        let masked = token.masked();

        assert!(masked.contains("..."));
        assert!(!masked.contains(&dummy));
    }

    #[test]
    fn test_debug_does_not_leak_token() {
        let dummy = AuthToken::dummy();
        let token = AuthToken::new_unchecked(AuthToken::dummy());
        let debug_output = format!("{token:?}");

        assert!(!debug_output.contains(&dummy));
    }
}
