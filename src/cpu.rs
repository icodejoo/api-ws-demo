use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
        // The container's CPU allotment (e.g. 0.1 on Render free tier), used as
        // the denominator so 100% means "saturating *our* quota", not the host's
        // cores. Read once — it doesn't change over the process's life.
        let cpus = effective_cpus();
        let mut prev: Option<(u64, Instant)> = None;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            match read_cgroup_cpu_usec() {
                Some(usage_usec) => {
                    let now = Instant::now();
                    if let Some((prev_usec, prev_at)) = prev {
                        let wall_usec = now.duration_since(prev_at).as_micros() as u64;
                        let pct = compute_usage_pct(prev_usec, usage_usec, wall_usec, cpus);
                        usage_clone.store(pct.min(100) as u8, Ordering::Relaxed);
                    }
                    prev = Some((usage_usec, now));
                }
                None => {
                    // cgroup cpu.stat unreadable (e.g. running the raw binary on
                    // Windows/macOS for local dev, or a non-cgroup-v2 host) —
                    // degrade to "0% used" rather than panicking; the breaker
                    // simply never trips.
                    usage_clone.store(0, Ordering::Relaxed);
                }
            }
        }
    });
    usage
}

/// Pure math, factored out so it's unit-testable without a real filesystem read.
/// `prev_usec`/`curr_usec` are cumulative container CPU-microseconds (from
/// cgroup `cpu.stat`'s `usage_usec`); `wall_usec` is the wall-clock gap between
/// the two samples; `cpus` is our CPU allotment. Result is percent of allotment.
fn compute_usage_pct(prev_usec: u64, curr_usec: u64, wall_usec: u64, cpus: f64) -> u64 {
    if wall_usec == 0 || cpus <= 0.0 {
        return 0;
    }
    let cpu_delta = curr_usec.saturating_sub(prev_usec) as f64;
    let capacity = wall_usec as f64 * cpus;
    ((cpu_delta / capacity) * 100.0).round() as u64
}

/// Cumulative CPU time the container has used, in microseconds, from cgroup v2's
/// `cpu.stat` (`usage_usec` line). This is scoped to our container — unlike
/// `/proc/stat`, which on a shared host aggregates every tenant's CPU.
fn read_cgroup_cpu_usec() -> Option<u64> {
    let content = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat").ok()?;
    content
        .lines()
        .find_map(|l| l.strip_prefix("usage_usec "))
        .and_then(|v| v.trim().parse().ok())
}

/// The container's effective CPU allotment. cgroup v2 `cpu.max` is
/// `"<quota> <period>"` (both µs) — e.g. `"10000 100000"` = 0.1 CPU. `"max"`
/// means no quota, so fall back to the host's logical CPU count.
fn effective_cpus() -> f64 {
    if let Ok(raw) = std::fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        let mut it = raw.split_whitespace();
        if let (Some(quota), Some(period)) = (it.next(), it.next()) {
            if quota != "max" {
                if let (Ok(q), Ok(p)) = (quota.parse::<f64>(), period.parse::<f64>()) {
                    if q > 0.0 && p > 0.0 {
                        return q / p;
                    }
                }
            }
        }
    }
    std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0)
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
        // used 750ms of CPU over a 1s wall gap on a 1-CPU allotment -> 75%
        assert_eq!(compute_usage_pct(0, 750_000, 1_000_000, 1.0), 75);
    }

    #[test]
    fn scales_by_fractional_cpu_allotment() {
        // used 100ms over 1s on a 0.1-CPU allotment -> fully saturated (100%)
        assert_eq!(compute_usage_pct(0, 100_000, 1_000_000, 0.1), 100);
    }

    #[test]
    fn zero_wall_gap_is_zero_usage() {
        assert_eq!(compute_usage_pct(1000, 2000, 0, 1.0), 0);
    }

    #[test]
    fn no_cpu_time_consumed_is_zero_usage() {
        assert_eq!(compute_usage_pct(500_000, 500_000, 1_000_000, 1.0), 0);
    }
}
