use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn index() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(json!({
        "name": "api-ws-demo",
        "endpoints": [
            "GET /health",
            "GET /api/info",
            "POST /api/echo",
            "GET|POST /api/mock",
            "GET /api/compressed",
            "GET /api/compressed-zstd",
            "GET /api/compressed-mp",
            "GET /api/compressed-mp-gzip",
            "GET /api/compressed-mp-zstd",
            "GET /api/me (requires auth)",
            "POST /auth/register",
            "POST /auth/login",
            "POST /auth/refresh",
            "POST /auth/logout",
            "GET /ws (websocket)",
            "GET /ws/secure (websocket, requires ?token=)",
            "GET /stomp (stomp over websocket)",
        ],
    })))
}

pub async fn health(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
    })))
}

pub async fn info(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(json!({
        "name": "api-ws-demo",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
    })))
}

pub async fn echo(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let content_type = headers.get(axum::http::header::CONTENT_TYPE).cloned();
    let mut resp_headers = HeaderMap::new();
    if let Some(ct) = content_type {
        resp_headers.insert(axum::http::header::CONTENT_TYPE, ct);
    }
    (resp_headers, body)
}
