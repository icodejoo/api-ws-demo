use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn stats(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let cpu_percent = state.cpu_usage.load(Ordering::Relaxed);
    let (mem_total_kb, mem_available_kb) = read_meminfo().unzip();
    let mem_used_kb = mem_total_kb.zip(mem_available_kb).map(|(t, a)| t.saturating_sub(a));
    let process_rss_kb = read_process_rss_kb();
    let (disk_total_bytes, disk_available_bytes) = read_disk_usage("/").unzip();
    let disk_used_bytes = disk_total_bytes
        .zip(disk_available_bytes)
        .map(|(t, a)| t.saturating_sub(a));

    Json(ApiResponse::ok(json!({
        "cpu_percent": cpu_percent,
        "memory": {
            "total_kb": mem_total_kb,
            "available_kb": mem_available_kb,
            "used_kb": mem_used_kb,
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

/// Returns `(total_kb, available_kb)` from `/proc/meminfo`. `None` on
/// non-Linux (e.g. local Windows/macOS dev) or if the file is unreadable.
fn read_meminfo() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = None;
    let mut available = None;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest.trim().split_whitespace().next().and_then(|v| v.parse().ok());
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = rest.trim().split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    Some((total?, available?))
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
