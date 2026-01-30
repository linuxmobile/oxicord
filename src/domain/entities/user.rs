//! Discord user entity.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub u64);

impl UserId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for UserId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<&str> for UserId {
    fn from(value: &str) -> Self {
        Self(value.parse().unwrap_or(0))
    }
}

impl From<String> for UserId {
    fn from(value: String) -> Self {
        Self(value.parse().unwrap_or(0))
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct UserFlags: u64 {
        const STAFF = 1 << 0;
        const PARTNER = 1 << 1;
        const HYPESQUAD = 1 << 2;
        const BUG_HUNTER_LEVEL_1 = 1 << 3;
        const HYPESQUAD_BRAVERY = 1 << 6;
        const HYPESQUAD_BRILLIANCE = 1 << 7;
        const HYPESQUAD_BALANCE = 1 << 8;
        const PREMIUM_EARLY_SUPPORTER = 1 << 9;
        const TEAM_PSEUDO_USER = 1 << 10;
        const SYSTEM = 1 << 12;
        const BUG_HUNTER_LEVEL_2 = 1 << 14;
        const VERIFIED_BOT = 1 << 16;
        const VERIFIED_DEVELOPER = 1 << 17;
        const CERTIFIED_MODERATOR = 1 << 18;
        const BOT_HTTP_INTERACTIONS = 1 << 19;
        const ACTIVE_DEVELOPER = 1 << 22;
        const QUARANTINED = 1 << 44;
        const COLLABORATOR = 1 << 50;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum PremiumType {
    #[default]
    None = 0,
    NitroClassic = 1,
    Nitro = 2,
    NitroBasic = 3,
}

impl From<u8> for PremiumType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::NitroClassic,
            2 => Self::Nitro,
            3 => Self::NitroBasic,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    id: UserId,
    username: String,
    discriminator: String,
    global_name: Option<String>,
    avatar: Option<String>,
    bot: bool,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    accent_color: Option<u32>,
    #[serde(default)]
    flags: UserFlags,
    #[serde(default)]
    public_flags: UserFlags,
    #[serde(default)]
    premium_type: PremiumType,
    color: Option<u32>,
}

impl User {
    #[must_use]
    pub fn new(
        id: impl Into<UserId>,
        username: impl Into<String>,
        discriminator: impl Into<String>,
        avatar: Option<String>,
        bot: bool,
        color: Option<u32>,
    ) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            discriminator: discriminator.into(),
            global_name: None,
            avatar,
            bot,
            system: false,
            banner: None,
            accent_color: None,
            flags: UserFlags::empty(),
            public_flags: UserFlags::empty(),
            premium_type: PremiumType::None,
            color,
        }
    }

    #[must_use]
    pub fn with_global_name(mut self, global_name: impl Into<String>) -> Self {
        self.global_name = Some(global_name.into());
        self
    }

    #[must_use]
    pub fn with_banner(mut self, banner: impl Into<String>) -> Self {
        self.banner = Some(banner.into());
        self
    }

    #[must_use]
    pub const fn with_accent_color(mut self, color: u32) -> Self {
        self.accent_color = Some(color);
        self
    }

    #[must_use]
    pub const fn with_flags(mut self, flags: UserFlags) -> Self {
        self.flags = flags;
        self
    }

    #[must_use]
    pub const fn with_public_flags(mut self, flags: UserFlags) -> Self {
        self.public_flags = flags;
        self
    }

    #[must_use]
    pub const fn with_premium_type(mut self, premium_type: PremiumType) -> Self {
        self.premium_type = premium_type;
        self
    }

    #[must_use]
    pub const fn with_system(mut self, system: bool) -> Self {
        self.system = system;
        self
    }

    #[must_use]
    pub const fn id(&self) -> UserId {
        self.id
    }

    #[must_use]
    pub fn id_str(&self) -> String {
        self.id.to_string()
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    #[must_use]
    pub fn discriminator(&self) -> &str {
        &self.discriminator
    }

    #[must_use]
    pub fn global_name(&self) -> Option<&str> {
        self.global_name.as_deref()
    }

    #[must_use]
    pub fn avatar(&self) -> Option<&str> {
        self.avatar.as_deref()
    }

    #[must_use]
    pub const fn is_bot(&self) -> bool {
        self.bot
    }

    #[must_use]
    pub const fn is_system(&self) -> bool {
        self.system
    }

    #[must_use]
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    #[must_use]
    pub const fn accent_color(&self) -> Option<u32> {
        self.accent_color
    }

    #[must_use]
    pub const fn flags(&self) -> UserFlags {
        self.flags
    }

    #[must_use]
    pub const fn public_flags(&self) -> UserFlags {
        self.public_flags
    }

    #[must_use]
    pub const fn premium_type(&self) -> PremiumType {
        self.premium_type
    }

    #[must_use]
    pub const fn color(&self) -> Option<u32> {
        self.color
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        if let Some(ref global_name) = self.global_name {
            global_name.clone()
        } else if self.discriminator == "0" {
            self.username.clone()
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }

    #[must_use]
    pub fn is_migrated(&self) -> bool {
        self.discriminator == "0"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new(
            123_456_789_u64,
            "testuser",
            "1234",
            Some("abc123".into()),
            false,
            None,
        );

        assert_eq!(user.id().as_u64(), 123_456_789);
        assert_eq!(user.username(), "testuser");
        assert_eq!(user.discriminator(), "1234");
        assert_eq!(user.avatar(), Some("abc123"));
        assert!(!user.is_bot());
    }

    #[test]
    fn test_display_name_legacy() {
        let user = User::new(123_u64, "olduser", "1234", None, false, None);
        assert_eq!(user.display_name(), "olduser#1234");
        assert!(!user.is_migrated());
    }

    #[test]
    fn test_display_name_new_style() {
        let user = User::new(123_u64, "newuser", "0", None, false, None);
        assert_eq!(user.display_name(), "newuser");
        assert!(user.is_migrated());
    }

    #[test]
    fn test_display_name_with_global_name() {
        let user =
            User::new(123_u64, "handle", "0", None, false, None).with_global_name("Display Name");
        assert_eq!(user.display_name(), "Display Name");
    }
}
