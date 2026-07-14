use std::sync::Arc;

use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;

pub fn build_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware> {
    let per_second: u64 = std::env::var("RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let burst: u32 = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(per_second)
            .burst_size(burst)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("valid governor config"),
    );

    GovernorLayer { config }
}
