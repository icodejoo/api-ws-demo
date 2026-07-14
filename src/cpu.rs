use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::response::AppError;
use crate::state::AppState;

pub fn spawn_sampler() -> Arc<AtomicU8> {
    let usage = Arc::new(AtomicU8::new(0));
    let usage_clone = usage.clone();
    tokio::spawn(async move {
        let mut prev: Option<(u64, u64)> = None;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            match read_cpu_sample() {
                Some(sample) => {
                    if let Some(prev_sample) = prev {
                        let pct = compute_usage_pct(prev_sample, sample);
                        usage_clone.store(pct.min(100) as u8, Ordering::Relaxed);
                    }
                    prev = Some(sample);
                }
                None => {
                    // /proc/stat unreadable (e.g. running the raw binary on
                    // Windows/macOS for local dev) — degrade to "0% used"
                    // rather than panicking; the breaker simply never trips.
                    usage_clone.store(0, Ordering::Relaxed);
                }
            }
        }
    });
    usage
}

/// Pure delta math, factored out so it's unit-testable without a real
/// filesystem read. Each sample is `(idle, total)` jiffies.
fn compute_usage_pct(prev: (u64, u64), curr: (u64, u64)) -> u64 {
    let (prev_idle, prev_total) = prev;
    let (idle, total) = curr;
    let idle_delta = idle.saturating_sub(prev_idle);
    let total_delta = total.saturating_sub(prev_total);
    if total_delta == 0 {
        return 0;
    }
    100u64.saturating_sub(idle_delta * 100 / total_delta)
}

fn read_cpu_sample() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let first_line = content.lines().next()?;
    let mut fields = first_line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let nums: Vec<u64> = fields.filter_map(|f| f.parse().ok()).collect();
    // user nice system idle iowait irq softirq steal [guest guest_nice]
    if nums.len() < 4 {
        return None;
    }
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0); // idle + iowait
    let total: u64 = nums.iter().sum();
    Some((idle, total))
}

fn cpu_threshold() -> u8 {
    std::env::var("CPU_BREAKER_THRESHOLD_PCT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90)
}

pub async fn cpu_breaker_mw(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let usage = state.cpu_usage.load(Ordering::Relaxed);
    if usage >= cpu_threshold() {
        return AppError::service_unavailable().into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_usage_from_deltas() {
        // idle grows by 50, total grows by 200 -> 75% used
        assert_eq!(compute_usage_pct((100, 1000), (150, 1200)), 75);
    }

    #[test]
    fn zero_total_delta_is_zero_usage() {
        assert_eq!(compute_usage_pct((100, 1000), (100, 1000)), 0);
    }

    #[test]
    fn fully_idle_is_zero_usage() {
        assert_eq!(compute_usage_pct((0, 0), (200, 200)), 0);
    }
}
