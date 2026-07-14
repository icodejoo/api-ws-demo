use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::response::AppError;
use crate::state::AppState;

use super::jwt;

pub struct AuthUser {
    pub username: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token =
            extract_bearer(parts).ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        let claims = jwt::decode(&token, &state.jwt_secret)
            .map_err(|_| AppError::unauthorized("invalid or expired token"))?;
        Ok(AuthUser {
            username: claims.sub,
        })
    }
}

pub fn extract_bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
}
