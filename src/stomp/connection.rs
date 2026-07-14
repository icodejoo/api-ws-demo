use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::auth::jwt;
use crate::compressed_assets;
use crate::stomp::broker::Broker;
use crate::stomp::frame::{StompCommand, StompFrame};

pub async fn handle_stomp_socket(socket: WebSocket, broker: Arc<Broker>, jwt_secret: Arc<[u8]>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<StompFrame>();
    let conn_id = Uuid::new_v4();
    let mut connected = false;
    let mut auth_user: Option<String> = None;

    let writer = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            let bytes = frame.serialize();
            if ws_tx.send(Message::Binary(bytes)).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    'reader: while let Some(Ok(msg)) = ws_rx.next().await {
        let payload: Vec<u8> = match msg {
            Message::Text(t) => t.into_bytes(),
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue,
        };

        for chunk in payload.split(|&b| b == 0) {
            // A lone heartbeat is just EOL bytes with no frame content; skip without
            // touching the frame's own trailing blank-line header/body separator.
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
                    let connected_frame = StompFrame::new(StompCommand::Connected)
                        .header("version", "1.2")
                        .header("server", concat!("api-ws-demo/", env!("CARGO_PKG_VERSION")))
                        .header("heart-beat", "0,0")
                        .header("session", conn_id.to_string());
                    let _ = out_tx.send(connected_frame);
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
                    broker.subscribe(dest, conn_id, id, out_tx.clone());
                    maybe_receipt(&frame, &out_tx);
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
                        broker.publish(dest, frame.get("content-type"), &frame.body, &[]);
                    }
                    maybe_receipt(&frame, &out_tx);
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

    broker.disconnect(conn_id);
    drop(out_tx);
    let _ = writer.await;
}

fn maybe_receipt(frame: &StompFrame, out_tx: &mpsc::UnboundedSender<StompFrame>) {
    if let Some(receipt_id) = frame.get("receipt") {
        let r = StompFrame::new(StompCommand::Receipt).header("receipt-id", receipt_id);
        let _ = out_tx.send(r);
    }
}

fn send_error(out_tx: &mpsc::UnboundedSender<StompFrame>, message: &str, detail: Option<&str>) {
    let mut f = StompFrame::new(StompCommand::Error).header("message", message);
    if let Some(d) = detail {
        f.body = d.as_bytes().to_vec();
    }
    let _ = out_tx.send(f);
}

fn is_heartbeat(b: &[u8]) -> bool {
    b.iter().all(|&c| c == b'\n' || c == b'\r')
}

fn destination_allowed(dest: &str, auth_user: &Option<String>) -> bool {
    if dest.starts_with("/topic/secure/") {
        auth_user.is_some()
    } else {
        true
    }
}
