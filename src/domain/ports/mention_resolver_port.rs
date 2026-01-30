pub trait MentionResolver: Send + Sync {
    fn resolve(&self, user_id: &str) -> Option<String>;
}
