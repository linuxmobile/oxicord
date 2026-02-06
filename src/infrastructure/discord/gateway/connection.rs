use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::{debug, error, info, trace, warn};

use super::codec::{EventParser, GatewayCodec};
use super::constants::{
    CONNECTION_TIMEOUT, GATEWAY_URL, GatewayIntents, GatewayOpcode, IDENTIFY_TIMEOUT,
};
use super::error::{GatewayError, GatewayResult};
use super::events::{DispatchEvent, GatewayEventKind};
use super::payloads::{GatewayMessage, GatewayPayload};
use super::session::SessionInfo;
use super::state::GatewayState;
use crate::infrastructure::discord::identity::ClientIdentity;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWriter = SplitSink<WsStream, WsMessage>;
type WsReader = SplitStream<WsStream>;

#[async_trait]
pub trait GatewayConnection: Send + Sync {
    async fn connect(&mut self, gateway_url: Option<&str>) -> GatewayResult<()>;
    async fn disconnect(&mut self) -> GatewayResult<()>;
    async fn send(&mut self, payload: &GatewayPayload) -> GatewayResult<()>;
    async fn receive(&mut self) -> GatewayResult<Option<GatewayMessage>>;
    fn is_connected(&self) -> bool;
}

pub struct WebSocketConnection {
    writer: Option<WsWriter>,
    reader: Option<WsReader>,
    codec: GatewayCodec,
    connected: bool,
}

impl WebSocketConnection {
    #[must_use]
    pub fn new() -> Self {
        Self {
            writer: None,
            reader: None,
            codec: GatewayCodec::new(),
            connected: false,
        }
    }

    async fn connect_internal(&mut self, url: &str) -> GatewayResult<()> {
        let connect_future = connect_async(url);
        let (ws_stream, _) = timeout(CONNECTION_TIMEOUT, connect_future)
            .await
            .map_err(|_| GatewayError::timeout("connection"))?
            .map_err(|e| GatewayError::connection_failed(e.to_string()))?;

        let (writer, reader) = ws_stream.split();
        self.writer = Some(writer);
        self.reader = Some(reader);
        self.connected = true;
        self.codec.reset();

        Ok(())
    }
}

impl Default for WebSocketConnection {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GatewayConnection for WebSocketConnection {
    async fn connect(&mut self, gateway_url: Option<&str>) -> GatewayResult<()> {
        let url = gateway_url.unwrap_or(GATEWAY_URL);
        self.connect_internal(url).await
    }

    async fn disconnect(&mut self) -> GatewayResult<()> {
        if let Some(mut writer) = self.writer.take() {
            let _ = writer.close().await;
        }
        self.reader = None;
        self.connected = false;
        self.codec.reset();
        debug!("WebSocket connection closed");
        Ok(())
    }

    async fn send(&mut self, payload: &GatewayPayload) -> GatewayResult<()> {
        let writer = self.writer.as_mut().ok_or(GatewayError::NotConnected)?;

        let json = serde_json::to_string(payload)
            .map_err(|e| GatewayError::serialization(e.to_string()))?;

        writer
            .send(WsMessage::Text(json.into()))
            .await
            .map_err(|e| GatewayError::websocket(e.to_string()))?;

        Ok(())
    }

    async fn receive(&mut self) -> GatewayResult<Option<GatewayMessage>> {
        let reader = self.reader.as_mut().ok_or(GatewayError::NotConnected)?;

        loop {
            match reader.next().await {
                Some(Ok(WsMessage::Binary(data))) => {
                    if let Some(json) = self.codec.decode_binary(&data)? {
                        let message = EventParser::parse_message(&json).map_err(|e| {
                            error!(error = %e, "Failed to parse decompressed message. Snippet: {:.200}", json);
                            e
                        })?;
                        return Ok(Some(message));
                    }
                }
                Some(Ok(WsMessage::Text(text))) => {
                    let message = EventParser::parse_message(&text).map_err(|e| {
                        error!(error = %e, "Failed to parse text message. Snippet: {:.200}", text);
                        e
                    })?;
                    return Ok(Some(message));
                }
                Some(Ok(WsMessage::Close(frame))) => {
                    self.connected = false;
                    let (code, reason) = frame.map_or_else(
                        || (1000, "Normal closure".to_string()),
                        |f| (f.code.into(), f.reason.to_string()),
                    );

                    return Err(GatewayError::ConnectionClosed { code, reason });
                }
                Some(Ok(WsMessage::Ping(data))) => {
                    if let Some(writer) = self.writer.as_mut() {
                        let _ = writer.send(WsMessage::Pong(data)).await;
                    }
                }
                Some(Ok(WsMessage::Pong(_) | WsMessage::Frame(_))) => {}
                Some(Err(e)) => {
                    self.connected = false;
                    return Err(GatewayError::websocket(e.to_string()));
                }
                None => {
                    self.connected = false;
                    return Err(GatewayError::ConnectionClosed {
                        code: 1000,
                        reason: "Stream ended".to_string(),
                    });
                }
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

pub struct GatewayConnectionHandler {
    connection: Box<dyn GatewayConnection>,
    state: GatewayState,
    session: SessionInfo,
    token: String,
    intents: GatewayIntents,
    event_tx: mpsc::UnboundedSender<GatewayEventKind>,
    payload_rx: mpsc::Receiver<String>,
    ack_received: Arc<AtomicBool>,
    identity: Arc<ClientIdentity>,
}

impl GatewayConnectionHandler {
    pub fn new(
        connection: Box<dyn GatewayConnection>,
        token: String,
        intents: GatewayIntents,
        event_tx: mpsc::UnboundedSender<GatewayEventKind>,
        payload_rx: mpsc::Receiver<String>,
        ack_received: Arc<AtomicBool>,
        identity: Arc<ClientIdentity>,
    ) -> Self {
        Self {
            connection,
            state: GatewayState::new(),
            session: SessionInfo::new(),
            token,
            intents,
            event_tx,
            payload_rx,
            ack_received,
            identity,
        }
    }

    pub async fn connect(&mut self) -> GatewayResult<()> {
        self.state.transition_to_connecting();

        let resume_url = self.session.resume_gateway_url().map(String::from);
        self.connection.connect(resume_url.as_deref()).await?;

        self.state.transition_to_waiting_hello();
        self.await_hello().await?;

        if self.session.can_resume() {
            self.resume().await?;
        } else {
            self.identify().await?;
        }

        Ok(())
    }

    async fn await_hello(&mut self) -> GatewayResult<()> {
        let hello_timeout = Duration::from_secs(10);

        let message = timeout(hello_timeout, self.connection.receive())
            .await
            .map_err(|_| GatewayError::timeout("Hello"))?
            .map_err(|e| GatewayError::connection_failed(format!("Failed to receive Hello: {e}")))?
            .ok_or_else(|| GatewayError::protocol("Expected Hello message"))?;

        let opcode = GatewayOpcode::from_u8(message.op);
        if opcode != Some(GatewayOpcode::Hello) {
            return Err(GatewayError::UnexpectedOpcode { opcode });
        }

        let data = message
            .d
            .ok_or_else(|| GatewayError::protocol("Hello missing data"))?;

        let hello = EventParser::parse_hello(&data)?;
        self.state.set_heartbeat_interval(hello.heartbeat_interval);

        debug!(
            interval_ms = hello.heartbeat_interval,
            "Received Hello from gateway"
        );

        Ok(())
    }

    async fn identify(&mut self) -> GatewayResult<()> {
        self.state.transition_to_identifying();

        let payload = GatewayPayload::identify(
            &self.token,
            self.intents.as_u32(),
            &self.identity.get_props(),
        );
        self.connection.send(&payload).await?;

        self.await_ready().await
    }

    async fn resume(&mut self) -> GatewayResult<()> {
        self.state.transition_to_resuming();

        let session_id = self
            .session
            .session_id()
            .ok_or_else(|| GatewayError::protocol("No session to resume"))?
            .to_string();

        let sequence = self
            .session
            .sequence()
            .ok_or_else(|| GatewayError::protocol("No sequence to resume"))?;

        let payload = GatewayPayload::resume(&self.token, &session_id, sequence);
        self.connection.send(&payload).await?;

        debug!(session_id = %session_id, sequence = sequence, "Sent Resume payload");

        self.await_resumed().await
    }

    async fn await_ready(&mut self) -> GatewayResult<()> {
        let message = timeout(IDENTIFY_TIMEOUT, self.connection.receive())
            .await
            .map_err(|_| GatewayError::timeout("Ready"))?
            .map_err(|e| GatewayError::connection_failed(format!("Failed to receive Ready: {e}")))?
            .ok_or_else(|| GatewayError::protocol("Expected Ready message"))?;

        let opcode = GatewayOpcode::from_u8(message.op);
        match opcode {
            Some(GatewayOpcode::Dispatch) => {
                if message.t.as_deref() == Some("READY") {
                    self.handle_ready_event(message)?;
                    self.state.transition_to_connected();
                    return Ok(());
                }
            }
            Some(GatewayOpcode::InvalidSession) => {
                let resumable = message.d.and_then(|d| d.as_bool()).unwrap_or(false);
                return Err(GatewayError::SessionInvalidated { resumable });
            }
            _ => {}
        }

        Err(GatewayError::protocol("Expected Ready event"))
    }

    async fn await_resumed(&mut self) -> GatewayResult<()> {
        let message = timeout(IDENTIFY_TIMEOUT, self.connection.receive())
            .await
            .map_err(|_| GatewayError::timeout("Resumed"))?
            .map_err(|e| {
                GatewayError::connection_failed(format!("Failed to receive Resumed: {e}"))
            })?
            .ok_or_else(|| GatewayError::protocol("Expected Resumed message"))?;

        let opcode = GatewayOpcode::from_u8(message.op);
        match opcode {
            Some(GatewayOpcode::Dispatch) => {
                if message.t.as_deref() == Some("RESUMED") {
                    info!("Session resumed successfully");
                    self.state.transition_to_connected();

                    let _ = self.event_tx.send(GatewayEventKind::Resumed);
                    return Ok(());
                }
            }
            Some(GatewayOpcode::InvalidSession) => {
                let resumable = message.d.and_then(|d| d.as_bool()).unwrap_or(false);

                if !resumable {
                    self.session.clear();
                }
                return Err(GatewayError::SessionInvalidated { resumable });
            }
            _ => {}
        }

        Err(GatewayError::protocol("Expected Resumed event"))
    }

    fn handle_ready_event(&mut self, message: GatewayMessage) -> GatewayResult<()> {
        if let Some(seq) = message.s {
            self.session.set_sequence(seq);
        }

        let dispatch = EventParser::parse_dispatch("READY", message.d)?;

        if let DispatchEvent::Ready {
            session_id,
            resume_gateway_url,
            user_id,
            guilds,
            initial_guild_channels: _,
            ..
        } = &dispatch
        {
            self.session
                .set_session(session_id.clone(), resume_gateway_url.clone());
            self.session.set_user_id(user_id.clone());

            info!(session_id = %session_id, guild_count = guilds.len(), "Gateway ready");

            let _ = self.event_tx.send(GatewayEventKind::Connected {
                session_id: session_id.clone(),
                resume_url: resume_gateway_url.clone(),
            });

            let _ = self.event_tx.send(GatewayEventKind::Dispatch(dispatch));
        }

        Ok(())
    }

    pub async fn run(&mut self) -> GatewayResult<()> {
        if self.state.connection().is_active() {
            tokio::select! {
                result = self.connection.receive() => {
                    match result {
                        Ok(Some(message)) => {
                            self.handle_message(message)?;
                        }
                        Ok(None) => {}
                        Err(e) => return Err(e),
                    }
                }

                Some(payload) = self.payload_rx.recv() => {
                    match serde_json::from_str::<GatewayPayload>(&payload) {
                        Ok(gateway_payload) => {
                            trace!(op = gateway_payload.op, "Sending payload to gateway");
                            if let Err(e) = self.connection.send(&gateway_payload).await {
                                warn!(error = %e, "Failed to send payload");
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, payload = %payload, "Failed to parse outgoing payload");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: GatewayMessage) -> GatewayResult<()> {
        self.session.update_sequence(message.s);

        let opcode = GatewayOpcode::from_u8(message.op);
        match opcode {
            Some(GatewayOpcode::Dispatch) => {
                if let Some(event_type) = message.t.as_deref() {
                    trace!(event = event_type, "Raw dispatch received");
                    self.handle_dispatch(event_type, message.d);
                }
            }
            Some(GatewayOpcode::HeartbeatAck) => {
                trace!("Received heartbeat ACK from gateway");
                self.state.record_heartbeat_ack();
                self.ack_received.store(true, Ordering::SeqCst);
                if let Some(latency) = self.state.latency_ms() {
                    let _ = self.event_tx.send(GatewayEventKind::HeartbeatAck {
                        latency_ms: latency,
                    });
                }
            }
            Some(GatewayOpcode::Heartbeat) => {
                debug!("Gateway requested immediate heartbeat");
            }
            Some(GatewayOpcode::Reconnect) => {
                info!("Gateway requested reconnect");
                return Err(GatewayError::ConnectionClosed {
                    code: 4000,
                    reason: "Reconnect requested".to_string(),
                });
            }
            Some(GatewayOpcode::InvalidSession) => {
                let resumable = message.d.and_then(|d| d.as_bool()).unwrap_or(false);

                warn!(resumable = resumable, "Session invalidated");

                if !resumable {
                    self.session.clear();
                }

                return Err(GatewayError::SessionInvalidated { resumable });
            }
            _ => {
                debug!(opcode = ?opcode, "Unhandled opcode");
            }
        }

        Ok(())
    }

    fn handle_dispatch(&self, event_type: &str, data: Option<serde_json::Value>) {
        match EventParser::parse_dispatch(event_type, data) {
            Ok(event) => {
                debug!(event = event_type, "Dispatching event");
                let _ = self.event_tx.send(GatewayEventKind::Dispatch(event));
            }
            Err(e) => {
                warn!(event = event_type, error = %e, "Failed to parse dispatch event");
            }
        }
    }

    #[must_use]
    pub const fn session(&self) -> &SessionInfo {
        &self.session
    }

    #[must_use]
    pub const fn state(&self) -> &GatewayState {
        &self.state
    }

    pub const fn heartbeat_interval(&self) -> Option<u64> {
        self.state.heartbeat_interval_ms()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_connection_initial_state() {
        let conn = WebSocketConnection::new();
        assert!(!conn.is_connected());
    }
}
