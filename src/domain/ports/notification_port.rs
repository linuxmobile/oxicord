use async_trait::async_trait;

/// Port for system notifications.
#[async_trait]
pub trait NotificationPort: Send + Sync {
    /// Shows a system notification.
    fn send(&self, title: &str, body: &str);
}
