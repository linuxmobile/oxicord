use notify_rust::Notification;

#[derive(Debug, Clone)]
pub struct NotificationService {
    enabled: bool,
}

impl NotificationService {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn send(&self, title: String, body: String) {
        if !self.enabled {
            return;
        }

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
