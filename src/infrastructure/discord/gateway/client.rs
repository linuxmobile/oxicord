use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use super::connection::{GatewayConnectionHandler, WebSocketConnection};
use super::constants::{
    GatewayIntents, MAX_RECONNECT_ATTEMPTS, RECONNECT_DELAY_BASE, RECONNECT_DELAY_MAX,
    RECONNECT_JITTER_MAX,
};
use super::error::{GatewayCloseCode, GatewayError, GatewayResult};
use super::events::{GatewayCommand, GatewayEventKind};
use super::heartbeat::HeartbeatManager;
use super::payloads::GatewayPayload;
use super::session::SessionInfo;
use crate::infrastructure::discord::identity::ClientIdentity;

pub struct GatewayClientConfig {
    pub intents: GatewayIntents,
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: u32,
}

impl Default for GatewayClientConfig {
    fn default() -> Self {
        Self {
            intents: GatewayIntents::default_client(),
            auto_reconnect: true,
            max_reconnect_attempts: MAX_RECONNECT_ATTEMPTS,
        }
    }
}

impl GatewayClientConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn with_intents(mut self, intents: GatewayIntents) -> Self {
        self.intents = intents;
        self
    }

    #[must_use]
    pub const fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    #[must_use]
    pub const fn with_max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.max_reconnect_attempts = attempts;
        self
    }
}

pub struct GatewayClient {
    config: GatewayClientConfig,
    running: Arc<AtomicBool>,
    command_tx: Option<mpsc::UnboundedSender<GatewayCommand>>,
    identity: Arc<ClientIdentity>,
}

impl GatewayClient {
    #[must_use]
    pub fn new(config: GatewayClientConfig, identity: Arc<ClientIdentity>) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            command_tx: None,
            identity,
        }
    }

    #[must_use]
    pub fn with_default_config(identity: Arc<ClientIdentity>) -> Self {
        Self::new(GatewayClientConfig::default(), identity)
    }

    /// # Errors
    ///
    /// Returns `GatewayError::AlreadyConnected` if connection is already active.
    pub fn connect(
        &mut self,
        token: &str,
    ) -> GatewayResult<mpsc::UnboundedReceiver<GatewayEventKind>> {
        if self.running.load(Ordering::SeqCst) {
            return Err(GatewayError::AlreadyConnected);
        }

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        self.command_tx = Some(command_tx);

        let config = GatewayLoopConfig {
            token: token.to_string(),
            intents: self.config.intents,
            auto_reconnect: self.config.auto_reconnect,
            max_attempts: self.config.max_reconnect_attempts,
        };
        let running = self.running.clone();
        let identity = self.identity.clone();

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let result = std::panic::AssertUnwindSafe(run_gateway_loop(
                config,
                event_tx.clone(),
                command_rx,
                running.clone(),
                identity,
            ));

            if let Err(panic_info) = result.catch_unwind().await {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };

                error!(panic = %panic_msg, "Gateway task panicked");
                running.store(false, Ordering::SeqCst);
                let _ = event_tx.send(GatewayEventKind::Error {
                    message: format!("Gateway task panicked: {panic_msg}"),
                    recoverable: false,
                });
            }
        });

        Ok(event_rx)
    }

    /// Send a command to the gateway (e.g., subscribe to channel typing events).
    pub fn send_command(&self, command: GatewayCommand) {
        if let Some(ref tx) = self.command_tx {
            let _ = tx.send(command);
        }
    }

    pub fn disconnect(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

struct GatewayLoopConfig {
    token: String,
    intents: GatewayIntents,
    auto_reconnect: bool,
    max_attempts: u32,
}

#[allow(clippy::too_many_arguments)]
async fn run_gateway_loop(
    config: GatewayLoopConfig,
    event_tx: mpsc::UnboundedSender<GatewayEventKind>,
    mut command_rx: mpsc::UnboundedReceiver<GatewayCommand>,
    running: Arc<AtomicBool>,
    identity: Arc<ClientIdentity>,
) {
    let mut reconnect_attempts: u32 = 0;
    let mut session = SessionInfo::new();

    while running.load(Ordering::SeqCst) {
        let (payload_tx, payload_rx) = mpsc::channel(32);
        let ack_received = Arc::new(AtomicBool::new(true));

        let connection = Box::new(WebSocketConnection::new());
        let handler = GatewayConnectionHandler::new(
            connection,
            config.token.clone(),
            config.intents,
            event_tx.clone(),
            payload_rx,
            ack_received.clone(),
            identity.clone(),
        );

        let result = run_single_connection(
            handler,
            &payload_tx,
            &mut command_rx,
            &running,
            &mut session,
            &mut reconnect_attempts,
            ack_received,
        )
        .await;

        match result {
            ConnectionResult::Success => {
                reconnect_attempts = 0;
            }
            ConnectionResult::Error(e) => {
                error!(error = %e, "Failed to connect to gateway");

                let _ = event_tx.send(GatewayEventKind::Error {
                    message: e.to_string(),
                    recoverable: e.should_reconnect(),
                });

                if !e.should_reconnect() || !config.auto_reconnect {
                    break;
                }

                reconnect_attempts += 1;
            }
            ConnectionResult::Disconnected(e) => {
                handle_connection_error(&e, &event_tx, &mut session, &mut reconnect_attempts);
            }
        }

        if !running.load(Ordering::SeqCst) {
            break;
        }

        if !config.auto_reconnect {
            let _ = event_tx.send(GatewayEventKind::Disconnected {
                reason: "Connection closed".to_string(),
                can_resume: session.can_resume(),
            });
            break;
        }

        if reconnect_attempts >= config.max_attempts {
            error!(
                attempts = reconnect_attempts,
                "Max reconnection attempts exceeded"
            );
            let _ = event_tx.send(GatewayEventKind::Error {
                message: format!(
                    "Max reconnection attempts ({}) exceeded",
                    config.max_attempts
                ),
                recoverable: false,
            });
            break;
        }

        let delay = calculate_backoff_delay(reconnect_attempts);
        info!(
            attempt = reconnect_attempts,
            delay_ms = delay.as_millis(),
            "Reconnecting to gateway"
        );

        let _ = event_tx.send(GatewayEventKind::Reconnecting {
            attempt: reconnect_attempts,
        });

        sleep(delay).await;
    }

    running.store(false, Ordering::SeqCst);
    info!("Gateway loop terminated");
}

enum ConnectionResult {
    Success,
    Error(GatewayError),
    Disconnected(GatewayError),
}

async fn run_single_connection(
    mut handler: GatewayConnectionHandler,
    payload_tx: &mpsc::Sender<String>,
    command_rx: &mut mpsc::UnboundedReceiver<GatewayCommand>,
    running: &Arc<AtomicBool>,
    session: &mut SessionInfo,
    reconnect_attempts: &mut u32,
    ack_received: Arc<AtomicBool>,
) -> ConnectionResult {
    match handler.connect().await {
        Ok(()) => {
            info!("Gateway connected");
            *reconnect_attempts = 0;

            if let Some(interval) = handler.heartbeat_interval() {
                let mut heartbeat = HeartbeatManager::new(interval);
                heartbeat.set_ack_received(ack_received);
                let (error_tx, mut error_rx) = mpsc::channel(1);
                let _heartbeat_handle = heartbeat.start(payload_tx.clone(), error_tx);

                let run_result = run_connection_loop(
                    &mut handler,
                    payload_tx,
                    command_rx,
                    running,
                    &mut error_rx,
                )
                .await;

                heartbeat.stop();

                *session = handler.session().clone();

                if let Err(e) = run_result {
                    return ConnectionResult::Disconnected(e);
                }
            }

            ConnectionResult::Success
        }
        Err(e) => ConnectionResult::Error(e),
    }
}

async fn run_connection_loop(
    handler: &mut GatewayConnectionHandler,
    payload_tx: &mpsc::Sender<String>,
    command_rx: &mut mpsc::UnboundedReceiver<GatewayCommand>,
    running: &Arc<AtomicBool>,
    heartbeat_error_rx: &mut mpsc::Receiver<()>,
) -> GatewayResult<()> {
    use tokio::select;

    while running.load(Ordering::SeqCst) && handler.state().connection().is_connected() {
        select! {
            result = handler.run() => {
                result?;
            }
            Some(command) = command_rx.recv() => {
                process_gateway_command(command, payload_tx).await;
            }
            Some(()) = heartbeat_error_rx.recv() => {
                warn!("Heartbeat failure detected, closing connection");
                return Err(GatewayError::ConnectionClosed {
                    code: 4000,
                    reason: "Heartbeat ACK missing".to_string(),
                });
            }
        }
    }

    Ok(())
}

async fn process_gateway_command(command: GatewayCommand, payload_tx: &mpsc::Sender<String>) {
    match command {
        GatewayCommand::SubscribeChannel {
            guild_id,
            channel_id,
        } => {
            let payload = GatewayPayload::lazy_request(&guild_id, &channel_id);
            if let Ok(json) = serde_json::to_string(&payload) {
                debug!(guild_id = %guild_id, channel_id = %channel_id, "Sending channel subscription (LazyRequest)");
                let _ = payload_tx.send(json).await;
            }
        }
    }
}

fn handle_connection_error(
    error: &GatewayError,
    event_tx: &mpsc::UnboundedSender<GatewayEventKind>,
    session: &mut SessionInfo,
    reconnect_attempts: &mut u32,
) {
    warn!(error = %error, "Connection error");

    let can_resume = error.can_resume() && session.can_resume();

    if let GatewayError::SessionInvalidated { resumable } = error
        && !resumable
    {
        session.clear();
    }

    if let Some(code) = error.close_code()
        && let Some(close_code) = GatewayCloseCode::from_u16(code)
        && close_code.is_fatal()
    {
        session.clear_all();
    }

    let _ = event_tx.send(GatewayEventKind::Disconnected {
        reason: error.to_string(),
        can_resume,
    });

    if error.should_reconnect() {
        *reconnect_attempts += 1;
    }
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_backoff_delay(attempt: u32) -> Duration {
    let base_delay = RECONNECT_DELAY_BASE.as_millis() as u64;
    let max_delay = RECONNECT_DELAY_MAX.as_millis() as u64;
    let jitter_max = RECONNECT_JITTER_MAX.as_millis() as u64;

    let exponential_delay = base_delay.saturating_mul(2_u64.saturating_pow(attempt.min(6)));
    let capped_delay = exponential_delay.min(max_delay);

    let jitter = rand_jitter(jitter_max);
    let total_delay = capped_delay.saturating_add(jitter);

    Duration::from_millis(total_delay)
}

fn rand_jitter(max: u64) -> u64 {
    use std::time::SystemTime;

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(0);

    nanos % max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = GatewayClientConfig::new()
            .with_auto_reconnect(false)
            .with_max_reconnect_attempts(5);

        assert!(!config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 5);
    }

    #[test]
    fn test_backoff_delay() {
        let delay0 = calculate_backoff_delay(0);
        let delay1 = calculate_backoff_delay(1);
        let delay2 = calculate_backoff_delay(2);

        assert!(delay0 < delay1);
        assert!(delay1 < delay2);

        let delay_max = calculate_backoff_delay(100);
        assert!(delay_max <= RECONNECT_DELAY_MAX + RECONNECT_JITTER_MAX);
    }

    #[test]
    fn test_client_initial_state() {
        let identity = Arc::new(ClientIdentity::new());
        let client = GatewayClient::with_default_config(identity);
        assert!(!client.is_running());
    }
}
