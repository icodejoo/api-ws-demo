use std::collections::HashMap;
use std::sync::RwLock;

use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::stomp::frame::{StompCommand, StompFrame};

pub type ConnId = Uuid;

struct Subscriber {
    conn_id: ConnId,
    sub_id: String,
    sender: UnboundedSender<StompFrame>,
}

#[derive(Default)]
pub struct Broker {
    subs: RwLock<HashMap<String, Vec<Subscriber>>>,
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
        sender: UnboundedSender<StompFrame>,
    ) {
        self.subs
            .write()
            .unwrap()
            .entry(dest.to_string())
            .or_default()
            .push(Subscriber {
                conn_id,
                sub_id: sub_id.to_string(),
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
    /// knowledge of individual destinations.
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
                frame.body = body.to_vec();
                let _ = sub.sender.send(frame);
            }
        }
    }
}
