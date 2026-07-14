use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::response::{ApiResponse, AppError};

const MAX_DELAY_MS: u64 = 10_000;

#[derive(Deserialize, Default)]
pub struct MockQuery {
    #[serde(default)]
    delay_ms: Option<u64>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    data: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct MockBody {
    #[serde(default)]
    delay_ms: Option<u64>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

pub async fn mock_get(Query(q): Query<MockQuery>) -> Response {
    build_mock_response(
        q.delay_ms,
        q.status,
        q.code,
        q.message,
        q.data.map(serde_json::Value::String),
    )
    .await
}

pub async fn mock_post(Json(b): Json<MockBody>) -> Response {
    build_mock_response(b.delay_ms, b.status, b.code, b.message, b.data).await
}

async fn build_mock_response(
    delay_ms: Option<u64>,
    status: Option<u16>,
    code: Option<i32>,
    message: Option<String>,
    data: Option<serde_json::Value>,
) -> Response {
    let delay = delay_ms.unwrap_or(0).min(MAX_DELAY_MS);
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }

    let status_code = status.unwrap_or(200);
    let Ok(status) = axum::http::StatusCode::from_u16(status_code) else {
        return AppError::bad_request(format!("invalid status code: {status_code}")).into_response();
    };

    let body = ApiResponse {
        code: code.unwrap_or(0),
        data: data.unwrap_or(serde_json::Value::Null),
        message: message.unwrap_or_else(|| "mock response".to_string()),
    };
    (status, Json(body)).into_response()
}
