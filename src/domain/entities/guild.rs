//! Discord guild entity.

use serde::{Deserialize, Serialize};

use super::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GuildId(pub u64);

impl GuildId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum VerificationLevel {
    #[default]
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    VeryHigh = 4,
}

impl From<u8> for VerificationLevel {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::VeryHigh,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum NsfwLevel {
    #[default]
    Default = 0,
    Explicit = 1,
    Safe = 2,
    AgeRestricted = 3,
}

impl From<u8> for NsfwLevel {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Explicit,
            2 => Self::Safe,
            3 => Self::AgeRestricted,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum PremiumTier {
    #[default]
    None = 0,
    Tier1 = 1,
    Tier2 = 2,
    Tier3 = 3,
}

impl From<u8> for PremiumTier {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Tier1,
            2 => Self::Tier2,
            3 => Self::Tier3,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Guild {
    id: GuildId,
    name: String,
    icon: Option<String>,
    owner_id: Option<UserId>,
    has_unread: bool,
    #[serde(default)]
    mention_count: u32,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    splash: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    verification_level: VerificationLevel,
    #[serde(default)]
    nsfw_level: NsfwLevel,
    #[serde(default)]
    premium_tier: PremiumTier,
    #[serde(default)]
    premium_subscription_count: u32,
    #[serde(default)]
    vanity_url_code: Option<String>,
    #[serde(default)]
    preferred_locale: Option<String>,
    #[serde(default)]
    approximate_member_count: Option<u32>,
    #[serde(default)]
    approximate_presence_count: Option<u32>,
    #[serde(default, skip)]
    position: i32,
}

impl Guild {
    #[must_use]
    pub fn new(id: impl Into<GuildId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            owner_id: None,
            has_unread: false,
            mention_count: 0,
            description: None,
            splash: None,
            banner: None,
            features: Vec::new(),
            verification_level: VerificationLevel::None,
            nsfw_level: NsfwLevel::Default,
            premium_tier: PremiumTier::None,
            premium_subscription_count: 0,
            vanity_url_code: None,
            preferred_locale: None,
            approximate_member_count: None,
            approximate_presence_count: None,
            position: 0,
        }
    }

    #[must_use]
    pub const fn with_position(mut self, position: i32) -> Self {
        self.position = position;
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn with_owner_id(mut self, owner_id: impl Into<UserId>) -> Self {
        self.owner_id = Some(owner_id.into());
        self
    }

    #[must_use]
    pub const fn with_unread(mut self, has_unread: bool) -> Self {
        self.has_unread = has_unread;
        self
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn with_splash(mut self, splash: impl Into<String>) -> Self {
        self.splash = Some(splash.into());
        self
    }

    #[must_use]
    pub fn with_banner(mut self, banner: impl Into<String>) -> Self {
        self.banner = Some(banner.into());
        self
    }

    #[must_use]
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    #[must_use]
    pub const fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    #[must_use]
    pub const fn with_nsfw_level(mut self, level: NsfwLevel) -> Self {
        self.nsfw_level = level;
        self
    }

    #[must_use]
    pub const fn with_premium_tier(mut self, tier: PremiumTier) -> Self {
        self.premium_tier = tier;
        self
    }

    #[must_use]
    pub const fn with_premium_subscription_count(mut self, count: u32) -> Self {
        self.premium_subscription_count = count;
        self
    }

    #[must_use]
    pub fn with_vanity_url_code(mut self, code: impl Into<String>) -> Self {
        self.vanity_url_code = Some(code.into());
        self
    }

    #[must_use]
    pub fn with_preferred_locale(mut self, locale: impl Into<String>) -> Self {
        self.preferred_locale = Some(locale.into());
        self
    }

    #[must_use]
    pub const fn with_approximate_member_count(mut self, count: u32) -> Self {
        self.approximate_member_count = Some(count);
        self
    }

    #[must_use]
    pub const fn with_approximate_presence_count(mut self, count: u32) -> Self {
        self.approximate_presence_count = Some(count);
        self
    }

    #[must_use]
    pub const fn id(&self) -> GuildId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    #[must_use]
    pub const fn owner_id(&self) -> Option<UserId> {
        self.owner_id
    }

    #[must_use]
    pub const fn has_unread(&self) -> bool {
        self.has_unread
    }

    #[must_use]
    pub const fn mention_count(&self) -> u32 {
        self.mention_count
    }

    pub fn set_mention_count(&mut self, count: u32) {
        self.mention_count = count;
    }

    pub const fn set_unread(&mut self, has_unread: bool) {
        self.has_unread = has_unread;
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn splash(&self) -> Option<&str> {
        self.splash.as_deref()
    }

    #[must_use]
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    #[must_use]
    pub fn features(&self) -> &[String] {
        &self.features
    }

    #[must_use]
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    #[must_use]
    pub const fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    #[must_use]
    pub const fn nsfw_level(&self) -> NsfwLevel {
        self.nsfw_level
    }

    #[must_use]
    pub const fn premium_tier(&self) -> PremiumTier {
        self.premium_tier
    }

    #[must_use]
    pub const fn premium_subscription_count(&self) -> u32 {
        self.premium_subscription_count
    }

    #[must_use]
    pub fn vanity_url_code(&self) -> Option<&str> {
        self.vanity_url_code.as_deref()
    }

    #[must_use]
    pub fn preferred_locale(&self) -> Option<&str> {
        self.preferred_locale.as_deref()
    }

    #[must_use]
    pub const fn approximate_member_count(&self) -> Option<u32> {
        self.approximate_member_count
    }

    #[must_use]
    pub const fn approximate_presence_count(&self) -> Option<u32> {
        self.approximate_presence_count
    }

    #[must_use]
    pub const fn position(&self) -> i32 {
        self.position
    }

    pub fn set_position(&mut self, position: i32) {
        self.position = position;
    }

    #[must_use]
    pub fn is_community(&self) -> bool {
        self.has_feature("COMMUNITY")
    }

    #[must_use]
    pub fn is_partnered(&self) -> bool {
        self.has_feature("PARTNERED")
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.has_feature("VERIFIED")
    }

    #[must_use]
    pub fn is_discoverable(&self) -> bool {
        self.has_feature("DISCOVERABLE")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuildFolder {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub color: Option<u64>,
    pub guild_ids: Vec<GuildId>,
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
    fn test_guild_with_features() {
        let guild = Guild::new(123_u64, "Community Server")
            .with_features(vec!["COMMUNITY".to_string(), "PARTNERED".to_string()]);

        assert!(guild.is_community());
        assert!(guild.is_partnered());
        assert!(!guild.is_verified());
    }
}
