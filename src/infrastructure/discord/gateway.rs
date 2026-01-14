//! Discord Gateway WebSocket client.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use flate2::read::ZlibDecoder;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};

use crate::domain::entities::{
    Attachment, AuthToken, ChannelId, Message, MessageAuthor, MessageId, MessageKind,
    MessageReference,
};
use crate::domain::errors::AuthError;
use crate::domain::ports::GatewayEvent;

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json&compress=zlib-stream";
const HEARTBEAT_BUFFER_MS: u64 = 250;

#[derive(Debug, Serialize)]
struct GatewayPayload {
    op: u8,
    d: Value,
}

#[derive(Debug, Deserialize)]
struct GatewayReceive {
    op: u8,
    d: Option<Value>,
    s: Option<u64>,
    t: Option<String>,
}

#[derive(Debug, Serialize)]
struct IdentifyPayload {
    token: String,
    properties: IdentifyProperties,
    compress: bool,
    large_threshold: u16,
    intents: u32,
}

#[derive(Debug, Serialize)]
struct IdentifyProperties {
    os: String,
    browser: String,
    device: String,
}

#[derive(Debug, Deserialize)]
struct HelloPayload {
    heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
struct ReadyPayload {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct MessagePayload {
    id: String,
    channel_id: String,
    author: AuthorPayload,
    content: String,
    timestamp: String,
    edited_timestamp: Option<String>,
    #[serde(rename = "type", default)]
    kind: u8,
    #[serde(default)]
    attachments: Vec<AttachmentPayload>,
    message_reference: Option<MessageReferencePayload>,
    referenced_message: Option<Box<MessagePayload>>,
    #[serde(default)]
    pinned: bool,
}

#[derive(Debug, Deserialize)]
struct AuthorPayload {
    id: String,
    username: String,
    #[serde(default)]
    discriminator: String,
    avatar: Option<String>,
    #[serde(default)]
    bot: bool,
}

#[derive(Debug, Deserialize)]
struct AttachmentPayload {
    id: String,
    filename: String,
    #[serde(default)]
    size: u64,
    url: String,
    content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageReferencePayload {
    message_id: Option<String>,
    channel_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageDeletePayload {
    id: String,
    channel_id: String,
}

const GATEWAY_INTENTS: u32 = 1 << 0 | 1 << 9 | 1 << 12 | 1 << 15;

pub struct DiscordGateway {
    connected: Arc<AtomicBool>,
}

impl DiscordGateway {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// # Errors
    /// Returns error if gateway connection fails.
    pub fn connect(
        &self,
        token: &AuthToken,
    ) -> Result<mpsc::UnboundedReceiver<GatewayEvent>, AuthError> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let token_string = token.as_str().to_string();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::run_gateway_loop(token_string, event_tx.clone(), connected).await
            {
                error!(error = %e, "Gateway connection failed");
                let _ = event_tx.send(GatewayEvent::Error {
                    message: e.to_string(),
                });
            }
        });

        self.connected.store(true, Ordering::SeqCst);
        Ok(event_rx)
    }

    #[allow(clippy::too_many_lines)]
    async fn run_gateway_loop(
        token: String,
        event_tx: mpsc::UnboundedSender<GatewayEvent>,
        connected: Arc<AtomicBool>,
    ) -> Result<(), AuthError> {
        let (ws_stream, _) = connect_async(GATEWAY_URL)
            .await
            .map_err(|e| AuthError::network(format!("WebSocket connection failed: {e}")))?;

        let (mut write, mut read) = ws_stream.split();
        let mut sequence: Option<u64> = None;
        let mut zlib_buffer = Vec::new();

        while connected.load(Ordering::SeqCst) {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(WsMessage::Binary(data))) => {
                            zlib_buffer.extend_from_slice(&data);

                            if data.ends_with(&[0x00, 0x00, 0xff, 0xff]) {
                                let mut decoder = ZlibDecoder::new(&zlib_buffer[..]);
                                let mut decompressed = String::new();
                                if decoder.read_to_string(&mut decompressed).is_ok()
                                    && let Ok(payload) =
                                        serde_json::from_str::<GatewayReceive>(&decompressed)
                                {
                                    if let Some(s) = payload.s {
                                        sequence = Some(s);
                                    }

                                    match payload.op {
                                        10 => {
                                            if let Some(d) = payload.d
                                                && let Ok(hello) =
                                                    serde_json::from_value::<HelloPayload>(d)
                                            {
                                                let heartbeat_interval_ms =
                                                    hello.heartbeat_interval;
                                                debug!(
                                                    interval = heartbeat_interval_ms,
                                                    "Received Hello"
                                                );

                                                let identify = GatewayPayload {
                                                    op: 2,
                                                    d: serde_json::to_value(IdentifyPayload {
                                                        token: token.clone(),
                                                        properties: IdentifyProperties {
                                                            os: "linux".to_string(),
                                                            browser: "discordo".to_string(),
                                                            device: "discordo".to_string(),
                                                        },
                                                        compress: true,
                                                        large_threshold: 250,
                                                        intents: GATEWAY_INTENTS,
                                                    })
                                                    .unwrap_or(Value::Null),
                                                };

                                                if let Ok(msg) = serde_json::to_string(&identify) {
                                                    let _ =
                                                        write.send(WsMessage::Text(msg.into())).await;
                                                }

                                                let tx = event_tx.clone();
                                                let seq = sequence;
                                                tokio::spawn(Self::heartbeat_loop(
                                                    heartbeat_interval_ms,
                                                    tx,
                                                    seq,
                                                ));
                                            }
                                        }
                                        0 => {
                                            if let Some(event_name) = payload.t.as_deref() {
                                                Self::handle_dispatch_event(
                                                    event_name,
                                                    payload.d,
                                                    &event_tx,
                                                );
                                            }
                                        }
                                        11 => {
                                            debug!("Heartbeat acknowledged");
                                        }
                                        7 => {
                                            info!("Gateway requested reconnect");
                                            let _ = event_tx.send(GatewayEvent::Reconnecting);
                                        }
                                        9 => {
                                            warn!("Session invalidated");
                                            let _ = event_tx.send(GatewayEvent::Disconnected);
                                            return Ok(());
                                        }
                                        _ => {}
                                    }
                                }
                                zlib_buffer.clear();
                            }
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            info!("Gateway connection closed");
                            let _ = event_tx.send(GatewayEvent::Disconnected);
                            return Ok(());
                        }
                        Some(Err(e)) => {
                            error!(error = %e, "WebSocket error");
                            let _ = event_tx.send(GatewayEvent::Error {
                                message: e.to_string(),
                            });
                            return Err(AuthError::network(e.to_string()));
                        }
                        None => {
                            info!("Gateway stream ended");
                            let _ = event_tx.send(GatewayEvent::Disconnected);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    async fn heartbeat_loop(
        interval_ms: u64,
        _event_tx: mpsc::UnboundedSender<GatewayEvent>,
        _sequence: Option<u64>,
    ) {
        let mut ticker = interval(Duration::from_millis(interval_ms - HEARTBEAT_BUFFER_MS));
        ticker.tick().await;

        loop {
            ticker.tick().await;
            debug!("Sending heartbeat");
        }
    }

    fn handle_dispatch_event(
        event_name: &str,
        data: Option<Value>,
        event_tx: &mpsc::UnboundedSender<GatewayEvent>,
    ) {
        let Some(data) = data else { return };

        match event_name {
            "READY" => {
                if let Ok(ready) = serde_json::from_value::<ReadyPayload>(data) {
                    info!(session_id = %ready.session_id, "Gateway ready");
                    let _ = event_tx.send(GatewayEvent::Ready {
                        session_id: ready.session_id,
                    });
                }
            }
            "MESSAGE_CREATE" => {
                if let Ok(msg_payload) = serde_json::from_value::<MessagePayload>(data)
                    && let Some(message) = Self::parse_message_payload(msg_payload)
                {
                    debug!(message_id = %message.id(), "Received new message");
                    let _ = event_tx.send(GatewayEvent::MessageCreate { message });
                }
            }
            "MESSAGE_UPDATE" => {
                if let Ok(msg_payload) = serde_json::from_value::<MessagePayload>(data)
                    && let Some(message) = Self::parse_message_payload(msg_payload)
                {
                    debug!(message_id = %message.id(), "Message updated");
                    let _ = event_tx.send(GatewayEvent::MessageUpdate { message });
                }
            }
            "MESSAGE_DELETE" => {
                if let Ok(delete) = serde_json::from_value::<MessageDeletePayload>(data)
                    && let (Ok(msg_id), Ok(ch_id)) =
                        (delete.id.parse::<u64>(), delete.channel_id.parse::<u64>())
                {
                    debug!(message_id = msg_id, "Message deleted");
                    let _ = event_tx.send(GatewayEvent::MessageDelete {
                        message_id: MessageId(msg_id),
                        channel_id: ChannelId(ch_id),
                    });
                }
            }
            _ => {
                debug!(event = event_name, "Unhandled gateway event");
            }
        }
    }

    fn parse_message_payload(payload: MessagePayload) -> Option<Message> {
        let id: u64 = payload.id.parse().ok()?;
        let channel_id: u64 = payload.channel_id.parse().ok()?;
        let timestamp: DateTime<Utc> = payload.timestamp.parse().ok()?;

        let author = MessageAuthor::new(
            payload.author.id,
            payload.author.username,
            payload.author.discriminator,
            payload.author.avatar,
            payload.author.bot,
        );

        let mut message = Message::new(id, channel_id, author, payload.content, timestamp)
            .with_kind(MessageKind::from(payload.kind))
            .with_pinned(payload.pinned);

        if !payload.attachments.is_empty() {
            let attachments = payload
                .attachments
                .into_iter()
                .map(|a| {
                    let mut att = Attachment::new(a.id, a.filename, a.size, a.url);
                    if let Some(ct) = a.content_type {
                        att = att.with_content_type(ct);
                    }
                    att
                })
                .collect();
            message = message.with_attachments(attachments);
        }

        if let Some(edited) = payload.edited_timestamp
            && let Ok(edited_ts) = edited.parse::<DateTime<Utc>>()
        {
            message = message.with_edited_timestamp(edited_ts);
        }

        if let Some(reference) = payload.message_reference {
            let ref_msg_id = reference.message_id.and_then(|id| id.parse::<u64>().ok());
            let ref_channel_id = reference.channel_id.and_then(|id| id.parse::<u64>().ok());
            message = message.with_reference(MessageReference::new(
                ref_msg_id.map(Into::into),
                ref_channel_id.map(Into::into),
            ));
        }

        if let Some(referenced) = payload.referenced_message
            && let Some(ref_message) = Self::parse_message_payload(*referenced)
        {
            message = message.with_referenced(ref_message);
        }

        Some(message)
    }

    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

impl Default for DiscordGateway {
    fn default() -> Self {
        Self::new()
    }
}
