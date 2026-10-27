//! 独立 TCP 网络模拟器及本地 HTTP 控制服务。
mod bucket;
mod manager;
mod profile;
mod relay;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use manager::{apply, start_schedule, Proxy, ProxyStats};
use profile::{Profile, FLAP_MS, MAX_TIME_MS};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;

/// 控制服务共享状态。
#[derive(Default)]
struct ControlState {
    /// 已创建代理。
    proxies: Mutex<HashMap<String, Arc<Proxy>>>,
}

/// 创建与切换请求。
#[derive(Deserialize)]
struct Input {
    /// 创建时必须给出的上游地址。
    upstream: Option<String>,
    /// 字符串预设或对象。
    profile: Value,
}

/// 一条相对调用时刻的时间表项目。
#[derive(Deserialize)]
struct Scheduled {
    /// 相对毫秒时刻。
    at_ms: u64,
    /// 该时刻的完整 profile。
    profile: Value,
}

/// 构建独立控制 API。无参数，返回路由；例如 `axum::serve(listener, control_router())`。
pub fn control_router() -> Router {
    Router::new()
        .route("/proxies", post(create))
        .route("/proxies/:id", put(update).delete(delete))
        .route("/proxies/:id/schedule", post(schedule))
        .route("/proxies/:id/stats", get(stats))
        .route("/presets", get(|| async { Json(profile::presets()) }))
        .with_state(Arc::new(ControlState::default()))
}

/// 控制 API 错误信封。
fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error":message.into()}))).into_response()
}

/// 查找代理，缺失时返回 404。
fn lookup(state: &ControlState, id: &str) -> Option<Arc<Proxy>> {
    state.proxies.lock().unwrap().get(id).cloned()
}

/// 创建本地随机端口代理，成功返回 id 和 port。
async fn create(State(state): State<Arc<ControlState>>, Json(input): Json<Input>) -> Response {
    let Some(upstream) = input.upstream else {
        return error(StatusCode::BAD_REQUEST, "upstream required");
    };
    if upstream.parse::<SocketAddr>().is_err() {
        return error(
            StatusCode::BAD_REQUEST,
            "upstream must be a numeric socket address",
        );
    }
    let (p, flap) = match Profile::parse(&input.profile) {
        Ok(v) => v,
        Err(e) => return error(StatusCode::BAD_REQUEST, e),
    };
    let proxy = match manager::create(upstream, p).await {
        Ok(proxy) => proxy,
        Err(error_value) => {
            return error(StatusCode::INTERNAL_SERVER_ERROR, error_value.to_string())
        }
    };
    let port = proxy.port;
    let id = Uuid::new_v4().to_string();
    state
        .proxies
        .lock()
        .unwrap()
        .insert(id.clone(), proxy.clone());
    if flap {
        start_schedule(&proxy, vec![(FLAP_MS, Profile::default())], 0);
    }
    Json(json!({"id":id,"port":port})).into_response()
}

/// 切换配置，取消先前时间表。
async fn update(
    State(state): State<Arc<ControlState>>,
    Path(id): Path<String>,
    Json(input): Json<Input>,
) -> Response {
    let Some(proxy) = lookup(&state, &id) else {
        return error(StatusCode::NOT_FOUND, "proxy not found");
    };
    let (p, flap) = match Profile::parse(&input.profile) {
        Ok(v) => v,
        Err(e) => return error(StatusCode::BAD_REQUEST, e),
    };
    let generation = proxy.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(timer) = proxy.timer.lock().unwrap().take() {
        timer.abort();
    }
    if let Err(e) = apply(&proxy, p, generation).await {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e);
    };
    if flap {
        start_schedule(&proxy, vec![(FLAP_MS, Profile::default())], generation);
    }
    Json(json!({"ok":true})).into_response()
}

/// 校验整张时间表后安装；以调用时刻为共同起点。
async fn schedule(
    State(state): State<Arc<ControlState>>,
    Path(id): Path<String>,
    Json(input): Json<Vec<Scheduled>>,
) -> Response {
    let Some(proxy) = lookup(&state, &id) else {
        return error(StatusCode::NOT_FOUND, "proxy not found");
    };
    let mut steps = Vec::new();
    for step in input {
        if step.at_ms > MAX_TIME_MS {
            return error(StatusCode::BAD_REQUEST, "schedule exceeds one day");
        }
        let (p, flap) = match Profile::parse(&step.profile) {
            Ok(v) => v,
            Err(e) => return error(StatusCode::BAD_REQUEST, e),
        };
        steps.push((step.at_ms, p));
        if flap {
            steps.push((step.at_ms + FLAP_MS, Profile::default()));
        }
    }
    steps.sort_by_key(|s| s.0);
    let generation = proxy.generation.fetch_add(1, Ordering::SeqCst) + 1;
    start_schedule(&proxy, steps, generation);
    Json(json!({"ok":true})).into_response()
}

/// 返回原子计数快照。
async fn stats(State(state): State<Arc<ControlState>>, Path(id): Path<String>) -> Response {
    let Some(proxy) = lookup(&state, &id) else {
        return error(StatusCode::NOT_FOUND, "proxy not found");
    };
    let s = &proxy.stats;
    let load = |v: &AtomicU64| v.load(Ordering::Relaxed);
    Json(json!({"connections":load(&s.connections),"active":load(&s.active),"bytes_up":load(&s.bytes_up),"bytes_down":load(&s.bytes_down),"resets":load(&s.resets),"refused":load(&s.refused)})).into_response()
}

/// 删除后广播关闭，并等待监听循环释放端口。
async fn delete(State(state): State<Arc<ControlState>>, Path(id): Path<String>) -> Response {
    let Some(proxy) = state.proxies.lock().unwrap().remove(&id) else {
        return error(StatusCode::NOT_FOUND, "proxy not found");
    };
    let _ = proxy.done.send(true);
    if let Some(timer) = proxy.timer.lock().unwrap().take() {
        timer.abort();
    }
    proxy.changes.closed().await;
    Json(json!({"ok":true})).into_response()
}
