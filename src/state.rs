use std::collections::HashMap;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use rand::Rng;

use crate::auth::refresh::RefreshRegistry;
use crate::auth::user_store::User;
use crate::stomp::broker::Broker;

#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub broker: Arc<Broker>,
    pub users: Arc<RwLock<HashMap<String, User>>>,
    pub refresh_registry: Arc<RefreshRegistry>,
    pub jwt_secret: Arc<[u8]>,
    pub cpu_usage: Arc<AtomicU8>,
}

impl AppState {
    pub fn new(cpu_usage: Arc<AtomicU8>) -> Self {
        let jwt_secret: Arc<[u8]> = match std::env::var("JWT_SECRET") {
            Ok(s) if !s.is_empty() => Arc::from(s.into_bytes()),
            _ => {
                let mut buf = [0u8; 32];
                rand::rng().fill_bytes(&mut buf);
                tracing::warn!(
                    "JWT_SECRET not set — generated an ephemeral secret; all sessions invalidate on restart"
                );
                Arc::from(buf.to_vec())
            }
        };

        Self {
            start_time: Instant::now(),
            broker: Arc::new(Broker::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            refresh_registry: Arc::new(RefreshRegistry::new()),
            jwt_secret,
            cpu_usage,
        }
    }
}
