//! Discord guild entity.

use serde::{Deserialize, Serialize};

/// Unique identifier for a Discord guild (server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GuildId(pub u64);

impl GuildId {
    /// Returns the underlying u64 value.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for GuildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for GuildId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<&str> for GuildId {
    fn from(value: &str) -> Self {
        Self(value.parse().unwrap_or(0))
    }
}

/// Discord guild (server) information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Guild {
    id: GuildId,
    name: String,
    icon: Option<String>,
    owner_id: Option<String>,
    has_unread: bool,
}

impl Guild {
    /// Creates a new guild with the given ID and name.
    #[must_use]
    pub fn new(id: impl Into<GuildId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            owner_id: None,
            has_unread: false,
        }
    }

    /// Sets the guild icon hash.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets whether the guild has unread messages.
    #[must_use]
    pub fn with_unread(mut self, has_unread: bool) -> Self {
        self.has_unread = has_unread;
        self
    }

    /// Returns the guild ID.
    #[must_use]
    pub fn id(&self) -> GuildId {
        self.id
    }

    /// Returns the guild name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the guild icon hash.
    #[must_use]
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Returns whether the guild has unread messages.
    #[must_use]
    pub fn has_unread(&self) -> bool {
        self.has_unread
    }

    /// Sets whether the guild has unread messages.
    pub fn set_unread(&mut self, has_unread: bool) {
        self.has_unread = has_unread;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guild_creation() {
        let guild = Guild::new(123_u64, "Test Server");

        assert_eq!(guild.id().as_u64(), 123);
        assert_eq!(guild.name(), "Test Server");
        assert!(!guild.has_unread());
    }

    #[test]
    fn test_guild_with_unread() {
        let guild = Guild::new(456_u64, "Busy Server").with_unread(true);

        assert!(guild.has_unread());
    }

    #[test]
    fn test_guild_id_display() {
        let id = GuildId(123_456_789);
        assert_eq!(format!("{id}"), "123456789");
    }
}
