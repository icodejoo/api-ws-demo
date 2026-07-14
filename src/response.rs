use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    pub data: T,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            data,
            message: "ok".into(),
        }
    }
}

pub struct AppError {
    pub status: StatusCode,
    pub code: i32,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, 401, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, 400, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, 409, message)
    }

    pub fn service_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            503,
            "server under high load, please retry",
        )
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiResponse::<Value> {
            code: self.code,
            data: Value::Null,
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<Json<ApiResponse<T>>, AppError>;
