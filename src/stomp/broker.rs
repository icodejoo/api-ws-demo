use std::collections::HashMap;
use std::sync::RwLock;

use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::stomp::frame::{OutgoingItem, StompCommand, StompFrame};

pub type ConnId = Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AckMode {
    Auto,
    Client,
    ClientIndividual,
}

impl AckMode {
    pub fn parse(header: Option<&str>) -> Self {
        match header {
            Some("client") => Self::Client,
            Some("client-individual") => Self::ClientIndividual,
            _ => Self::Auto,
        }
    }
}

struct Subscriber {
    conn_id: ConnId,
    sub_id: String,
    ack_mode: AckMode,
    sender: UnboundedSender<OutgoingItem>,
}

#[derive(Default)]
pub struct Broker {
    subs: RwLock<HashMap<String, Vec<Subscriber>>>,
    last_message: RwLock<HashMap<String, Vec<u8>>>,
}

impl Broker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &self,
        dest: &str,
        conn_id: ConnId,
        sub_id: &str,
        ack_mode: AckMode,
        sender: UnboundedSender<OutgoingItem>,
    ) {
        self.subs
            .write()
            .unwrap()
            .entry(dest.to_string())
            .or_default()
            .push(Subscriber {
                conn_id,
                sub_id: sub_id.to_string(),
                ack_mode,
                sender,
            });
    }

    pub fn unsubscribe(&self, conn_id: ConnId, sub_id: &str) {
        let mut subs = self.subs.write().unwrap();
        for list in subs.values_mut() {
            list.retain(|s| !(s.conn_id == conn_id && s.sub_id == sub_id));
        }
        subs.retain(|_, v| !v.is_empty());
    }

    pub fn disconnect(&self, conn_id: ConnId) {
        let mut subs = self.subs.write().unwrap();
        for list in subs.values_mut() {
            list.retain(|s| s.conn_id != conn_id);
        }
        subs.retain(|_, v| !v.is_empty());
    }

    /// Publishes to every subscriber of `dest`, building a distinct MESSAGE frame per
    /// subscriber (each needs its own `subscription` + `message-id`). `extra_headers`
    /// lets callers attach destination-specific metadata (e.g. `content-encoding: gzip`
    /// for the static-compressed test topic) without the broker needing any special
    /// knowledge of individual destinations. Subscribers in `client`/`client-individual`
    /// ack mode get a fresh `ack` header on their copy of the frame; the *receiving*
    /// connection's own writer task is responsible for tracking that id as pending
    /// (see `connection.rs`), since only it can later match an incoming ACK/NACK to it.
    pub fn publish(
        &self,
        dest: &str,
        content_type: Option<&str>,
        body: &[u8],
        extra_headers: &[(&str, &str)],
    ) {
        let subs = self.subs.read().unwrap();
        if let Some(list) = subs.get(dest) {
            for sub in list {
                let mut frame = StompFrame::new(StompCommand::Message)
                    .header("destination", dest)
                    .header("subscription", &sub.sub_id)
                    .header("message-id", Uuid::new_v4().to_string())
                    .header("content-length", body.len().to_string());
                if let Some(ct) = content_type {
                    frame = frame.header("content-type", ct);
                }
                for (k, v) in extra_headers {
                    frame = frame.header(*k, *v);
                }
                if sub.ack_mode != AckMode::Auto {
                    frame = frame.header("ack", Uuid::new_v4().to_string());
                }
                frame.body = body.to_vec();
                let _ = sub.sender.send(OutgoingItem::Frame(frame));
            }
        }
    }

    /// Records the most recent body SENT to `dest` (non-static-asset destinations
    /// only), so a late-joining subscriber's delayed auto-push has something to
    /// replay instead of falling back to "ready".
    pub fn record_last_message(&self, dest: &str, body: &[u8]) {
        self.last_message
            .write()
            .unwrap()
            .insert(dest.to_string(), body.to_vec());
    }

    pub fn last_message(&self, dest: &str) -> Option<Vec<u8>> {
        self.last_message.read().unwrap().get(dest).cloned()
    }
}
