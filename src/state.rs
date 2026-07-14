use std::sync::Arc;
use std::time::Instant;

use crate::stomp::broker::Broker;

#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub broker: Arc<Broker>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            broker: Arc::new(Broker::new()),
        }
    }
}
