use crate::domain::ports::NotificationPort;
use notify_rust::Notification;

#[derive(Debug, Clone, Default)]
pub struct DesktopNotificationService {
    enabled: bool,
}

impl DesktopNotificationService {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl NotificationPort for DesktopNotificationService {
    fn send(&self, title: &str, body: &str) {
        if !self.enabled {
            return;
        }

        let title = title.to_string();
        let body = body.to_string();

        tokio::task::spawn_blocking(move || {
            if let Err(e) = Notification::new()
                .summary(&title)
                .body(&body)
                .appname("Oxicord")
                .show()
            {
                tracing::warn!("Failed to show notification: {}", e);
            }
        });
    }
}
