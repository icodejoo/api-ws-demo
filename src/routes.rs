use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::stomp::connection::handle_stomp_socket;
use crate::{rest, ws};

async fn stomp_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_stomp_socket(socket, state.broker.clone()))
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(rest::index))
        .route("/health", get(rest::health))
        .route("/api/info", get(rest::info))
        .route("/api/echo", post(rest::echo))
        .route("/ws", get(ws::ws_handler))
        .route("/stomp", get(stomp_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
