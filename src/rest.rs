use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
    }))
}

pub async fn info(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "name": "api-ws-demo",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
    }))
}

pub async fn echo(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let content_type = headers.get(axum::http::header::CONTENT_TYPE).cloned();
    let mut resp_headers = HeaderMap::new();
    if let Some(ct) = content_type {
        resp_headers.insert(axum::http::header::CONTENT_TYPE, ct);
    }
    (resp_headers, body)
}
