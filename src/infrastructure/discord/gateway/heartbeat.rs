use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, interval_at};
use tracing::{debug, warn};

use super::constants::HEARTBEAT_JITTER_PERCENT;
use super::payloads::GatewayPayload;

pub struct HeartbeatManager {
    interval_ms: u64,
    sequence: Arc<std::sync::atomic::AtomicU64>,
    running: Arc<AtomicBool>,
    ack_received: Arc<AtomicBool>,
}

impl HeartbeatManager {
    #[must_use]
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            ack_received: Arc::new(AtomicBool::new(true)),
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn start(
        &self,
        payload_tx: mpsc::Sender<String>,
        error_tx: mpsc::Sender<()>,
    ) -> tokio::task::JoinHandle<()> {
        let interval_ms = self.interval_ms;
        let sequence = self.sequence.clone();
        let running = self.running.clone();
        let ack_received = self.ack_received.clone();

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let jitter = (interval_ms as f64 * HEARTBEAT_JITTER_PERCENT) as u64;
            let first_delay = Duration::from_millis(interval_ms - jitter);
            let mut ticker = interval_at(
                Instant::now() + first_delay,
                Duration::from_millis(interval_ms),
            );

            while running.load(Ordering::SeqCst) {
                ticker.tick().await;

                if !running.load(Ordering::SeqCst) {
                    break;
                }

                if !ack_received.load(Ordering::SeqCst) {
                    warn!("Heartbeat ACK not received, connection may be dead");
                    if error_tx.send(()).await.is_err() {
                        debug!("Error channel closed");
                    }
                    break;
                }

                let seq = sequence.load(Ordering::SeqCst);
                let seq_opt = if seq == 0 { None } else { Some(seq) };

                let payload = GatewayPayload::heartbeat(seq_opt);
                if let Ok(json) = serde_json::to_string(&payload) {
                    ack_received.store(false, Ordering::SeqCst);
                    if payload_tx.send(json).await.is_err() {
                        debug!("Heartbeat channel closed");
                        break;
                    }
                    debug!(sequence = ?seq_opt, "Sent heartbeat");
                }
            }

            debug!("Heartbeat loop stopped");
        })
    }

    pub fn set_ack_received(&mut self, ack_received: Arc<AtomicBool>) {
        self.ack_received = ack_received;
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for HeartbeatManager {
    fn drop(&mut self) {
        self.stop();
    }
}
