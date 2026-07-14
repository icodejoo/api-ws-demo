use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn stats(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let cpu_percent = state.cpu_usage.load(Ordering::Relaxed);
    let memory = read_memory();
    let process_rss_kb = read_process_rss_kb();
    let (disk_total_bytes, disk_available_bytes) = read_disk_usage("/").unzip();
    let disk_used_bytes = disk_total_bytes
        .zip(disk_available_bytes)
        .map(|(t, a)| t.saturating_sub(a));

    Json(ApiResponse::ok(json!({
        "cpu_percent": cpu_percent,
        "memory": {
            "source": memory.as_ref().map(|m| m.source),
            "total_kb": memory.as_ref().map(|m| m.total_kb),
            "available_kb": memory.as_ref().map(|m| m.available_kb),
            "used_kb": memory.as_ref().map(|m| m.used_kb),
            "process_rss_kb": process_rss_kb,
        },
        "disk": {
            "path": "/",
            "total_bytes": disk_total_bytes,
            "available_bytes": disk_available_bytes,
            "used_bytes": disk_used_bytes,
        },
    })))
}

struct Memory {
    source: &'static str,
    total_kb: u64,
    available_kb: u64,
    used_kb: u64,
}

/// Prefers the container's actual cgroup memory limit/usage (what Render's
/// free-tier quota really enforces) over `/proc/meminfo`, which — on a
/// shared multi-tenant host — reports the *host's* total memory, wildly
/// overstating how much headroom the container actually has. Falls back to
/// `/proc/meminfo` when no cgroup limit is set (e.g. local non-containerized
/// Linux, or an intentionally unlimited cgroup).
fn read_memory() -> Option<Memory> {
    read_cgroup_memory().or_else(read_meminfo)
}

/// Tries cgroup v2 first (`/sys/fs/cgroup/memory.{max,current}`), then falls
/// back to the legacy cgroup v1 layout
/// (`/sys/fs/cgroup/memory/memory.{limit_in_bytes,usage_in_bytes}`).
fn read_cgroup_memory() -> Option<Memory> {
    // cgroup v2: a literal "max" means no limit is set — not useful as a quota.
    if let Ok(raw) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
        let raw = raw.trim();
        if raw != "max" {
            let limit_bytes: u64 = raw.parse().ok()?;
            let used_bytes: u64 = std::fs::read_to_string("/sys/fs/cgroup/memory.current")
                .ok()?
                .trim()
                .parse()
                .ok()?;
            return Some(Memory {
                source: "cgroup_v2",
                total_kb: limit_bytes / 1024,
                used_kb: used_bytes / 1024,
                available_kb: limit_bytes.saturating_sub(used_bytes) / 1024,
            });
        }
    }

    // cgroup v1: "unlimited" shows up as a huge sentinel near i64::MAX, not a real quota.
    const CGROUP_V1_UNLIMITED_THRESHOLD: u64 = 1 << 62;
    if let Ok(raw) = std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
        let limit_bytes: u64 = raw.trim().parse().ok()?;
        if limit_bytes < CGROUP_V1_UNLIMITED_THRESHOLD {
            let used_bytes: u64 =
                std::fs::read_to_string("/sys/fs/cgroup/memory/memory.usage_in_bytes")
                    .ok()?
                    .trim()
                    .parse()
                    .ok()?;
            return Some(Memory {
                source: "cgroup_v1",
                total_kb: limit_bytes / 1024,
                used_kb: used_bytes / 1024,
                available_kb: limit_bytes.saturating_sub(used_bytes) / 1024,
            });
        }
    }

    None
}

/// System-wide memory from `/proc/meminfo` — the fallback when no cgroup
/// memory limit is in effect. On a shared host this reflects the *host's*
/// memory, not any container-specific quota.
fn read_meminfo() -> Option<Memory> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest.trim().split_whitespace().next().and_then(|v| v.parse::<u64>().ok());
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = rest.trim().split_whitespace().next().and_then(|v| v.parse::<u64>().ok());
        }
    }
    let total_kb = total?;
    let available_kb = available?;
    Some(Memory {
        source: "system",
        total_kb,
        available_kb,
        used_kb: total_kb.saturating_sub(available_kb),
    })
}

/// Returns this process's resident set size in KB from `/proc/self/status`.
fn read_process_rss_kb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.trim().split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// Returns `(total_bytes, available_bytes)` for the filesystem containing
/// `path`, via the `statvfs` syscall. Linux-only; `None` elsewhere.
#[cfg(target_os = "linux")]
fn read_disk_usage(path: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let c_path = CString::new(path).ok()?;
    let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if ret != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    let block_size = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * block_size;
    let available = stat.f_bavail as u64 * block_size;
    Some((total, available))
}

#[cfg(not(target_os = "linux"))]
fn read_disk_usage(_path: &str) -> Option<(u64, u64)> {
    None
}
