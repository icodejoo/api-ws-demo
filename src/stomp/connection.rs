use std::collections::HashSet;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::auth::jwt;
use crate::compressed_assets;
use crate::stomp::broker::{AckMode, Broker};
use crate::stomp::frame::{next_wire_id, OutgoingItem, StompCommand, StompFrame};

/// Pre-serialized auto-push body when a destination has no cached message —
/// avoids re-running `json!` + `to_string` on every SUBSCRIBE's delayed push.
const READY_BODY: &[u8] = br#"{"response":"ready"}"#;

/// Pre-serialized ACK/NACK confirmation body — avoids `json!` per ACK/NACK.
const STATUS_OK_BODY: &[u8] = br#"{"status":"ok"}"#;

/// How long a connection may live at all, regardless of activity/heartbeat
/// health — forces recycling of long-lived connections on the free tier.
const MAX_CONNECTION_LIFETIME: Duration = Duration::from_secs(180);

/// Server's own heartbeat guarantees, offered on CONNECTED and negotiated
/// against whatever the client proposed on CONNECT. At 60s, the incoming-
/// heartbeat timeout (60s × HEARTBEAT_MISS_GRACE_FACTOR = 180s) lands right at
/// the 180s hard TTL — a dead peer is caught by whichever of the two fires
/// first, they're no longer meaningfully staggered the way a shorter interval
/// would allow.
const SERVER_HEARTBEAT_SEND_MS: u64 = 60_000;
const SERVER_HEARTBEAT_WANT_MS: u64 = 60_000;

/// A negotiated interval of 0 means "disabled" — represented internally as an
/// interval so long it will never practically fire within a connection's
/// bounded 180s lifetime, avoiding `Option<Interval>` plumbing in the select loop.
const DISABLED_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24 * 365);

/// STOMP spec convention: tolerate missing up to this many negotiated
/// incoming-heartbeat intervals (network jitter) before declaring the peer dead.
const HEARTBEAT_MISS_GRACE_FACTOR: u32 = 3;

/// Delay after a successful SUBSCRIBE before auto-pushing a message to just
/// that new subscriber, unconditionally (see `send_delayed_push`).
const DELAYED_PUSH_DELAY: Duration = Duration::from_secs(3);

pub async fn handle_stomp_socket(socket: WebSocket, broker: Arc<Broker>, jwt_secret: Arc<[u8]>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutgoingItem>();
    let conn_id = Uuid::new_v4();
    let mut connected = false;
    let mut auth_user: Option<String> = None;

    // Ack ids this connection has issued to the client (via MESSAGE frames in
    // client/client-individual ack mode) and not yet acknowledged. Populated
    // by the writer task below as it forwards MESSAGE frames out; consumed by
    // the ACK/NACK dispatch arm in the main loop.
    let pending_acks: Arc<StdMutex<HashSet<String>>> = Arc::new(StdMutex::new(HashSet::new()));

    let writer_pending_acks = pending_acks.clone();
    let writer = tokio::spawn(async move {
        while let Some(item) = out_rx.recv().await {
            let bytes = match item {
                OutgoingItem::Frame(frame) => {
                    if frame.command == StompCommand::Message {
                        if let Some(ack_id) = frame.get("ack") {
                            writer_pending_acks.lock().unwrap().insert(ack_id.to_string());
                        }
                    }
                    frame.serialize()
                }
                OutgoingItem::Heartbeat => vec![b'\n'],
            };
            if ws_tx.send(ws_message_for(bytes)).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    let ttl_sleep = tokio::time::sleep(MAX_CONNECTION_LIFETIME);
    tokio::pin!(ttl_sleep);

    let mut last_activity = Instant::now();
    let mut outgoing_hb = tokio::time::interval(DISABLED_INTERVAL);
    let mut incoming_check = tokio::time::interval(DISABLED_INTERVAL);
    let mut incoming_timeout = DISABLED_INTERVAL;

    'reader: loop {
        tokio::select! {
            msg = ws_rx.next() => {
                let Some(Ok(msg)) = msg else { break 'reader };
                last_activity = Instant::now();

                let payload: Vec<u8> = match msg {
                    Message::Text(t) => t.into_bytes(),
                    Message::Binary(b) => b,
                    Message::Close(_) => break 'reader,
                    _ => continue,
                };

                for chunk in payload.split(|&b| b == 0) {
                    // A lone heartbeat is just EOL bytes with no frame content; skip
                    // without touching the frame's own trailing blank-line separator.
                    // Already counted as activity above.
                    if is_heartbeat(chunk) {
                        continue;
                    }

                    let frame = match StompFrame::parse(chunk) {
                        Ok(f) => f,
                        Err(e) => {
                            send_error(&out_tx, &format!("malformed frame: {}", e.0), None);
                            break 'reader;
                        }
                    };

                    if !connected
                        && frame.command != StompCommand::Connect
                        && frame.command != StompCommand::Stomp
                    {
                        send_error(&out_tx, "you must first issue a CONNECT frame", None);
                        break 'reader;
                    }

                    match frame.command {
                        StompCommand::Connect | StompCommand::Stomp => {
                            connected = true;
                            if let Some(auth_header) =
                                frame.get("Authorization").or_else(|| frame.get("authorization"))
                            {
                                match auth_header
                                    .strip_prefix("Bearer ")
                                    .map(|token| jwt::decode(token, &jwt_secret))
                                {
                                    Some(Ok(claims)) => auth_user = Some(claims.sub),
                                    _ => {
                                        // A present-but-invalid Authorization header is rejected
                                        // outright rather than silently downgraded to anonymous —
                                        // a typo'd token silently becoming "no auth" is a worse
                                        // failure mode than an explicit error.
                                        send_error(&out_tx, "invalid Authorization header on CONNECT", None);
                                        break 'reader;
                                    }
                                }
                            }

                            let (effective_out_ms, effective_in_ms) = negotiate_heartbeat(
                                frame.get("heart-beat"),
                                SERVER_HEARTBEAT_SEND_MS,
                                SERVER_HEARTBEAT_WANT_MS,
                            );
                            if effective_out_ms > 0 {
                                outgoing_hb = tokio::time::interval(Duration::from_millis(effective_out_ms));
                            }
                            if effective_in_ms > 0 {
                                incoming_check = tokio::time::interval(Duration::from_millis(effective_in_ms));
                                incoming_timeout =
                                    Duration::from_millis(effective_in_ms * u64::from(HEARTBEAT_MISS_GRACE_FACTOR));
                            }

                            let connected_frame = StompFrame::new(StompCommand::Connected)
                                .header("version", "1.2")
                                .header("server", concat!("api-ws-demo/", env!("CARGO_PKG_VERSION")))
                                .header("heart-beat", format!("{effective_out_ms},{effective_in_ms}"))
                                .header("session", conn_id.to_string());
                            let _ = out_tx.send(OutgoingItem::Frame(connected_frame));
                        }
                        StompCommand::Subscribe => {
                            let (Some(dest), Some(id)) = (frame.get("destination"), frame.get("id"))
                            else {
                                send_error(
                                    &out_tx,
                                    "SUBSCRIBE requires 'destination' and 'id' headers",
                                    None,
                                );
                                break 'reader;
                            };
                            if !destination_allowed(dest, &auth_user) {
                                send_error(&out_tx, "not authorized to subscribe to this destination", None);
                                continue;
                            }
                            let ack_mode = AckMode::parse(frame.get("ack"));
                            broker.subscribe(dest, conn_id, id, ack_mode, out_tx.clone());
                            maybe_receipt(&frame, &out_tx);

                            let push_out_tx = out_tx.clone();
                            let push_broker = broker.clone();
                            let push_dest = dest.to_string();
                            let push_sub_id = id.to_string();
                            tokio::spawn(async move {
                                tokio::time::sleep(DELAYED_PUSH_DELAY).await;
                                send_delayed_push(&push_out_tx, &push_broker, &push_dest, &push_sub_id);
                            });
                        }
                        StompCommand::Unsubscribe => {
                            let Some(id) = frame.get("id") else {
                                send_error(&out_tx, "UNSUBSCRIBE requires 'id' header", None);
                                break 'reader;
                            };
                            broker.unsubscribe(conn_id, id);
                            maybe_receipt(&frame, &out_tx);
                        }
                        StompCommand::Send => {
                            let Some(dest) = frame.get("destination") else {
                                send_error(&out_tx, "SEND requires 'destination' header", None);
                                break 'reader;
                            };
                            if !destination_allowed(dest, &auth_user) {
                                send_error(&out_tx, "not authorized to send to this destination", None);
                                continue;
                            }
                            if let Some(asset) = compressed_assets::lookup_by_topic(dest) {
                                // Ignore whatever body the client sent — these destinations
                                // always broadcast the same static, build-time-generated
                                // payload, so subscribers can test decompression/decoding
                                // without the server ever spending CPU encoding anything.
                                let mut extra_headers: Vec<(&str, &str)> = Vec::new();
                                if let Some(enc) = asset.content_encoding {
                                    extra_headers.push(("content-encoding", enc));
                                }
                                broker.publish(dest, Some(asset.content_type), asset.bytes, &extra_headers);
                            } else {
                                let text = String::from_utf8_lossy(&frame.body).into_owned();
                                let wrapped = json!({ "response": text }).to_string().into_bytes();
                                broker.record_last_message(dest, &wrapped);
                                broker.publish(dest, Some("application/json"), &wrapped, &[]);
                            }
                            maybe_receipt(&frame, &out_tx);
                        }
                        StompCommand::Ack | StompCommand::Nack => {
                            let Some(ack_id) = frame.get("id") else {
                                send_error(&out_tx, "ACK/NACK requires 'id' header", None);
                                continue;
                            };
                            let known = pending_acks.lock().unwrap().remove(ack_id);
                            if known {
                                send_status_ok(&out_tx, ack_id);
                            } else {
                                send_error(&out_tx, "unknown or already-acknowledged ack id", None);
                            }
                        }
                        StompCommand::Disconnect => {
                            maybe_receipt(&frame, &out_tx);
                            break 'reader;
                        }
                        _ => {
                            send_error(
                                &out_tx,
                                &format!("unsupported client command '{}'", frame.command.as_str()),
                                None,
                            );
                            break 'reader;
                        }
                    }
                }
            }
            _ = outgoing_hb.tick() => {
                let _ = out_tx.send(OutgoingItem::Heartbeat);
            }
            _ = incoming_check.tick() => {
                if last_activity.elapsed() > incoming_timeout {
                    send_error(&out_tx, "missed heartbeat, connection considered dead", None);
                    break 'reader;
                }
            }
            _ = &mut ttl_sleep => {
                send_error(&out_tx, "connection time limit exceeded, please reconnect", None);
                break 'reader;
            }
        }
    }

    broker.disconnect(conn_id);
    drop(out_tx);
    let _ = writer.await;
}

/// STOMP 1.2 heartbeat negotiation. `client_header` is the raw value of the
/// client's CONNECT `heart-beat` header (format `cx,cy`); absent or malformed
/// defaults to `0,0` (disabled) rather than failing CONNECT over a soft
/// feature. Returns `(effective_outgoing_ms, effective_incoming_ms)` — what
/// the server will actually send at / expects to receive at (0 = disabled).
fn negotiate_heartbeat(client_header: Option<&str>, server_send_ms: u64, server_want_ms: u64) -> (u64, u64) {
    let (client_send_ms, client_want_ms) = client_header
        .and_then(|h| h.split_once(','))
        .and_then(|(a, b)| Some((a.trim().parse::<u64>().ok()?, b.trim().parse::<u64>().ok()?)))
        .unwrap_or((0, 0));

    let effective_outgoing = if server_send_ms == 0 || client_want_ms == 0 {
        0
    } else {
        server_send_ms.max(client_want_ms)
    };
    let effective_incoming = if server_want_ms == 0 || client_send_ms == 0 {
        0
    } else {
        server_want_ms.max(client_send_ms)
    };
    (effective_outgoing, effective_incoming)
}

/// Fires once, 3 seconds after a successful SUBSCRIBE, unconditionally. For
/// the static-compressed-asset topics, pushes that topic's static payload
/// (same binary format as an explicit SEND would produce). For any other
/// destination, replays the broker's cached last-sent message for it, or
/// `{"response":"ready"}` if nothing has ever been sent there.
fn send_delayed_push(out_tx: &mpsc::UnboundedSender<OutgoingItem>, broker: &Broker, dest: &str, sub_id: &str) {
    let mut frame = StompFrame::new(StompCommand::Message)
        .header("destination", dest)
        .header("subscription", sub_id)
        .header("message-id", next_wire_id().to_string());

    if let Some(asset) = compressed_assets::lookup_by_topic(dest) {
        frame = frame
            .header("content-type", asset.content_type)
            .header("content-length", asset.bytes.len().to_string());
        if let Some(enc) = asset.content_encoding {
            frame = frame.header("content-encoding", enc);
        }
        frame.body = asset.bytes.to_vec();
    } else {
        let body = broker
            .last_message(dest)
            .unwrap_or_else(|| READY_BODY.to_vec());
        frame = frame
            .header("content-type", "application/json")
            .header("content-length", body.len().to_string());
        frame.body = body;
    }

    let _ = out_tx.send(OutgoingItem::Frame(frame));
}

fn send_status_ok(out_tx: &mpsc::UnboundedSender<OutgoingItem>, ack_id: &str) {
    let mut frame = StompFrame::new(StompCommand::Receipt)
        .header("receipt-id", ack_id)
        .header("content-type", "application/json")
        .header("content-length", STATUS_OK_BODY.len().to_string());
    frame.body = STATUS_OK_BODY.to_vec();
    let _ = out_tx.send(OutgoingItem::Frame(frame));
}

fn maybe_receipt(frame: &StompFrame, out_tx: &mpsc::UnboundedSender<OutgoingItem>) {
    if let Some(receipt_id) = frame.get("receipt") {
        let r = StompFrame::new(StompCommand::Receipt).header("receipt-id", receipt_id);
        let _ = out_tx.send(OutgoingItem::Frame(r));
    }
}

fn send_error(out_tx: &mpsc::UnboundedSender<OutgoingItem>, message: &str, detail: Option<&str>) {
    let mut f = StompFrame::new(StompCommand::Error).header("message", message);
    if let Some(d) = detail {
        f.body = d.as_bytes().to_vec();
    }
    let _ = out_tx.send(OutgoingItem::Frame(f));
}

fn is_heartbeat(b: &[u8]) -> bool {
    b.iter().all(|&c| c == b'\n' || c == b'\r')
}

/// Chooses the WebSocket frame type to carry `bytes`: `Text` when they're
/// valid UTF-8 (the common case — CONNECTED/RECEIPT/ERROR frames and
/// JSON-bodied MESSAGE frames), `Binary` otherwise (the 5 static
/// compressed-asset topics' gzip/zstd/msgpack bodies, which aren't valid
/// UTF-8). This matters specifically because this is a *test* server for
/// STOMP client implementations: some real-world servers send Text frames
/// for text content, and a client with a bug in its Text-frame handling path
/// (e.g. assuming `event.data` is always a Blob/ArrayBuffer) would never be
/// caught here if every frame were sent as Binary regardless of content.
fn ws_message_for(bytes: Vec<u8>) -> Message {
    match String::from_utf8(bytes) {
        Ok(text) => Message::Text(text),
        Err(e) => Message::Binary(e.into_bytes()),
    }
}

fn destination_allowed(dest: &str, auth_user: &Option<String>) -> bool {
    if dest.starts_with("/topic/secure/") {
        auth_user.is_some()
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_utf8_becomes_a_text_frame() {
        assert!(matches!(ws_message_for(b"hello".to_vec()), Message::Text(t) if t == "hello"));
    }

    #[test]
    fn invalid_utf8_becomes_a_binary_frame() {
        let raw = vec![0x1f, 0x8b, 0x08, 0x00, 0xff, 0xfe, 0xfd];
        assert!(matches!(ws_message_for(raw.clone()), Message::Binary(b) if b == raw));
    }

    #[test]
    fn lone_heartbeat_newline_becomes_a_text_frame() {
        assert!(matches!(ws_message_for(vec![b'\n']), Message::Text(t) if t == "\n"));
    }

    #[test]
    fn negotiates_both_directions_when_both_sides_want_heartbeats() {
        assert_eq!(negotiate_heartbeat(Some("10000,10000"), 10_000, 10_000), (10_000, 10_000));
    }

    #[test]
    fn absent_client_header_disables_both_directions() {
        assert_eq!(negotiate_heartbeat(None, 10_000, 10_000), (0, 0));
    }

    #[test]
    fn malformed_client_header_defaults_to_disabled() {
        assert_eq!(negotiate_heartbeat(Some("garbage"), 10_000, 10_000), (0, 0));
    }

    #[test]
    fn client_declining_to_receive_disables_outgoing_only() {
        // client: "I can send every 5000ms, I don't want to receive any (0)"
        assert_eq!(negotiate_heartbeat(Some("5000,0"), 10_000, 10_000), (0, 10_000));
    }

    #[test]
    fn client_declining_to_send_disables_incoming_only() {
        // client: "I won't send any (0), I want to receive every 5000ms"
        assert_eq!(negotiate_heartbeat(Some("0,5000"), 10_000, 10_000), (10_000, 0));
    }

    #[test]
    fn takes_the_larger_of_the_two_proposed_intervals() {
        assert_eq!(negotiate_heartbeat(Some("20000,20000"), 10_000, 10_000), (20_000, 20_000));
    }
}
