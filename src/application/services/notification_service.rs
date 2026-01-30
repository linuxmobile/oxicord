use std::sync::Arc;

use crate::domain::ports::NotificationPort;

#[derive(Clone)]
pub struct NotificationService {
    port: Arc<dyn NotificationPort>,
}

impl NotificationService {
    #[must_use]
    pub fn new(port: Arc<dyn NotificationPort>) -> Self {
        Self { port }
    }

    pub fn send(&self, title: &str, body: &str) {
        self.port.send(title, body);
    }
}
