use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

pub struct RefreshRecord {
    pub username: String,
    pub expires_at: u64,
}

#[derive(Default)]
pub struct RefreshRegistry {
    tokens: RwLock<HashMap<Uuid, RefreshRecord>>,
}

impl RefreshRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issue(&self, username: &str, ttl_secs: u64) -> Uuid {
        let id = Uuid::new_v4();
        let expires_at = now() + ttl_secs;
        self.tokens.write().unwrap().insert(
            id,
            RefreshRecord {
                username: username.to_string(),
                expires_at,
            },
        );
        id
    }

    /// Validates and atomically rotates: the old id is removed and a new one
    /// issued under a single write-lock acquisition, so the old token can't
    /// be double-spent between validation and revocation.
    pub fn rotate(&self, old_id: Uuid, ttl_secs: u64) -> Option<(Uuid, String)> {
        let mut tokens = self.tokens.write().unwrap();
        let record = tokens.remove(&old_id)?;
        if record.expires_at < now() {
            return None;
        }
        let new_id = Uuid::new_v4();
        let username = record.username.clone();
        tokens.insert(
            new_id,
            RefreshRecord {
                username: username.clone(),
                expires_at: now() + ttl_secs,
            },
        );
        Some((new_id, username))
    }

    pub fn revoke(&self, id: Uuid) {
        self.tokens.write().unwrap().remove(&id);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after unix epoch")
        .as_secs()
}
