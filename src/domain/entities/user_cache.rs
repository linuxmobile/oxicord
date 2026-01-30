use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::User;
use crate::domain::ports::MentionResolver;

#[derive(Debug, Clone)]
pub struct CachedUser {
    id: String,
    username: String,
    discriminator: String,
    avatar: Option<String>,
    bot: bool,
}

impl CachedUser {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        username: impl Into<String>,
        discriminator: impl Into<String>,
        avatar: Option<String>,
        bot: bool,
    ) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            discriminator: discriminator.into(),
            avatar,
            bot,
        }
    }

    #[must_use]
    pub fn from_user(user: &User) -> Self {
        Self {
            id: user.id().to_string(),
            username: user.username().to_string(),
            discriminator: user.discriminator().to_string(),
            avatar: user.avatar().map(String::from),
            bot: user.is_bot(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
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
    pub fn avatar(&self) -> Option<&str> {
        self.avatar.as_deref()
    }

    #[must_use]
    pub const fn is_bot(&self) -> bool {
        self.bot
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        if self.discriminator == "0" {
            self.username.clone()
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }

    pub fn update_username(&mut self, username: impl Into<String>) {
        self.username = username.into();
    }
}

impl From<User> for CachedUser {
    fn from(user: User) -> Self {
        Self::from_user(&user)
    }
}

impl From<&User> for CachedUser {
    fn from(user: &User) -> Self {
        Self::from_user(user)
    }
}

#[derive(Debug, Clone)]
pub struct UserCache {
    inner: Arc<RwLock<UserCacheInner>>,
}

#[derive(Debug, Default)]
struct UserCacheInner {
    users: HashMap<String, CachedUser>,
}

impl UserCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(UserCacheInner::default())),
        }
    }

    pub fn insert(&self, user: CachedUser) {
        if let Ok(mut inner) = self.inner.write() {
            inner.users.insert(user.id.clone(), user);
        }
    }

    pub fn insert_from_user(&self, user: &User) {
        self.insert(CachedUser::from_user(user));
    }

    pub fn insert_basic(&self, user_id: impl Into<String>, username: impl Into<String>) {
        let user = CachedUser::new(user_id, username, "0", None, false);
        self.insert(user);
    }

    #[must_use]
    pub fn get(&self, user_id: &str) -> Option<CachedUser> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.users.get(user_id).cloned())
    }

    #[must_use]
    pub fn get_display_name(&self, user_id: &str) -> Option<String> {
        self.get(user_id).map(|u| u.display_name())
    }

    #[must_use]
    pub fn contains(&self, user_id: &str) -> bool {
        self.inner
            .read()
            .ok()
            .is_some_and(|inner| inner.users.contains_key(user_id))
    }

    pub fn update_username(&self, user_id: &str, username: impl Into<String>) {
        if let Ok(mut inner) = self.inner.write()
            && let Some(user) = inner.users.get_mut(user_id)
        {
            user.update_username(username);
        }
    }

    pub fn remove(&self, user_id: &str) {
        if let Ok(mut inner) = self.inner.write() {
            inner.users.remove(user_id);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.users.clear();
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().ok().map_or(0, |inner| inner.users.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for UserCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MentionResolver for UserCache {
    fn resolve(&self, user_id: &str) -> Option<String> {
        self.get_display_name(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_user_display_name() {
        let user = CachedUser::new("123", "testuser", "0", None, false);
        assert_eq!(user.display_name(), "testuser");

        let legacy_user = CachedUser::new("456", "legacyuser", "1234", None, false);
        assert_eq!(legacy_user.display_name(), "legacyuser#1234");
    }

    #[test]
    fn test_user_cache_insert_and_get() {
        let cache = UserCache::new();
        cache.insert_basic("123", "testuser");

        let user = cache.get("123").unwrap();
        assert_eq!(user.username(), "testuser");
    }

    #[test]
    fn test_user_cache_mention_resolver() {
        let cache = UserCache::new();
        cache.insert_basic("123", "testuser");

        let result = cache.resolve("123");
        assert_eq!(result, Some("testuser".to_string()));

        let missing = cache.resolve("999");
        assert!(missing.is_none());
    }

    #[test]
    fn test_user_cache_thread_safe() {
        use std::thread;

        let cache = UserCache::new();
        let cache_clone = cache.clone();

        let handle = thread::spawn(move || {
            cache_clone.insert_basic("123", "testuser");
        });

        handle.join().unwrap();
        assert!(cache.contains("123"));
    }
}
