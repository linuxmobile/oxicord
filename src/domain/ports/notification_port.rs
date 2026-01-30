use async_trait::async_trait;

/// Port for system notifications.
#[async_trait]
pub trait NotificationPort: Send + Sync {
    /// Shows a system notification.
    fn send(&self, title: &str, body: &str);
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct MockNotificationPort {
        pub notifications: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl MockNotificationPort {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl NotificationPort for MockNotificationPort {
        fn send(&self, title: &str, body: &str) {
            self.notifications
                .lock()
                .unwrap()
                .push((title.to_string(), body.to_string()));
        }
    }
}
