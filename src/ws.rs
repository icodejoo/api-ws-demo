use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::auth::jwt;
use crate::response::AppError;
use crate::state::AppState;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

#[derive(Deserialize, Default)]
pub struct WsSecureQuery {
    #[serde(default)]
    token: Option<String>,
}

pub async fn ws_secure_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<WsSecureQuery>,
) -> Response {
    let Some(token) = q.token else {
        return AppError::unauthorized("missing token query parameter").into_response();
    };
    match jwt::decode(&token, &state.jwt_secret) {
        Ok(_claims) => ws.on_upgrade(handle_socket).into_response(),
        Err(_) => AppError::unauthorized("invalid or expired token").into_response(),
    }
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        let echoed = match msg {
            Message::Text(t) => Message::Text(t),
            Message::Binary(b) => Message::Binary(b),
            Message::Close(_) => break,
            _ => continue,
        };
        if socket.send(echoed).await.is_err() {
            break;
        }
    }
}
