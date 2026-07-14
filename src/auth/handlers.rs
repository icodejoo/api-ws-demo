use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::response::{ApiResponse, ApiResult, AppError};
use crate::state::AppState;

use super::extractor::AuthUser;
use super::jwt::{self, Claims};
use super::password;
use super::user_store::User;

const ACCESS_TTL_SECS: u64 = 15 * 60;
const REFRESH_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Deserialize)]
pub struct Credentials {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    refresh_token: String,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after unix epoch")
        .as_secs()
}

fn issue_tokens(state: &AppState, username: &str) -> serde_json::Value {
    let access_token = jwt::encode(
        &Claims {
            sub: username.to_string(),
            iat: now(),
            exp: now() + ACCESS_TTL_SECS,
        },
        &state.jwt_secret,
    );
    let refresh_token = state.refresh_registry.issue(username, REFRESH_TTL_SECS);
    json!({
        "access_token": access_token,
        "refresh_token": refresh_token.to_string(),
        "token_type": "Bearer",
        "expires_in": ACCESS_TTL_SECS,
    })
}

fn parse_body<T: serde::de::DeserializeOwned>(
    body: Result<Json<T>, JsonRejection>,
) -> Result<T, AppError> {
    body.map(|Json(v)| v)
        .map_err(|e| AppError::bad_request(format!("invalid request body: {e}")))
}

pub async fn register(
    State(state): State<AppState>,
    body: Result<Json<Credentials>, JsonRejection>,
) -> ApiResult<serde_json::Value> {
    let creds = parse_body(body)?;
    if creds.username.is_empty() || creds.password.is_empty() {
        return Err(AppError::bad_request("username and password are required"));
    }

    let mut users = state.users.write().unwrap();
    if users.contains_key(&creds.username) {
        return Err(AppError::conflict("username already registered"));
    }

    let salt = password::generate_salt();
    let password_hash = password::hash_password(&creds.password, &salt);
    users.insert(
        creds.username.clone(),
        User {
            username: creds.username.clone(),
            salt,
            password_hash,
        },
    );
    drop(users);

    Ok(Json(ApiResponse::ok(
        json!({ "username": creds.username }),
    )))
}

pub async fn login(
    State(state): State<AppState>,
    body: Result<Json<Credentials>, JsonRejection>,
) -> ApiResult<serde_json::Value> {
    let creds = parse_body(body)?;

    let users = state.users.read().unwrap();
    let user = users
        .get(&creds.username)
        .ok_or_else(|| AppError::unauthorized("invalid username or password"))?;
    if !password::verify_password(&creds.password, &user.salt, &user.password_hash) {
        return Err(AppError::unauthorized("invalid username or password"));
    }
    let username = user.username.clone();
    drop(users);

    Ok(Json(ApiResponse::ok(issue_tokens(&state, &username))))
}

pub async fn refresh(
    State(state): State<AppState>,
    body: Result<Json<RefreshRequest>, JsonRejection>,
) -> ApiResult<serde_json::Value> {
    let req = parse_body(body)?;
    let old_id = Uuid::parse_str(&req.refresh_token)
        .map_err(|_| AppError::unauthorized("invalid refresh token"))?;

    let Some((new_id, username)) = state.refresh_registry.rotate(old_id, REFRESH_TTL_SECS) else {
        return Err(AppError::unauthorized(
            "refresh token invalid, expired, or already used",
        ));
    };

    let access_token = jwt::encode(
        &Claims {
            sub: username,
            iat: now(),
            exp: now() + ACCESS_TTL_SECS,
        },
        &state.jwt_secret,
    );

    Ok(Json(ApiResponse::ok(json!({
        "access_token": access_token,
        "refresh_token": new_id.to_string(),
        "token_type": "Bearer",
        "expires_in": ACCESS_TTL_SECS,
    }))))
}

pub async fn logout(
    State(state): State<AppState>,
    body: Result<Json<RefreshRequest>, JsonRejection>,
) -> ApiResult<serde_json::Value> {
    let req = parse_body(body)?;
    if let Ok(id) = Uuid::parse_str(&req.refresh_token) {
        state.refresh_registry.revoke(id);
    }
    Ok(Json(ApiResponse::ok(json!({ "logged_out": true }))))
}

pub async fn me(auth: AuthUser) -> ApiResult<serde_json::Value> {
    Ok(Json(ApiResponse::ok(
        json!({ "username": auth.username }),
    )))
}
