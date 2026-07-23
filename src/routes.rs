use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::state::AppState;
use crate::stomp::connection::handle_stomp_socket;
use crate::{auth, compressed_http, cpu, mock, ratelimit, rest, stats, ws};

async fn stomp_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let jwt_secret = state.jwt_secret.clone();
    // Standards-compliant STOMP client libraries (e.g. @stomp/stompjs) request one of these
    // WebSocket subprotocols on the handshake. Per the WebSocket spec, if the client offers a
    // subprotocol list and the server's response doesn't echo one back, the client MUST abort
    // the connection — so without this, no real STOMP.js-based client can ever connect here,
    // even though a bare `new WebSocket(url)` (no subprotocol requested) works fine either way.
    // We only speak one frame dialect (STOMP 1.2-level) regardless of which of these gets
    // negotiated; declaring all three just satisfies clients pinned to older protocol versions.
    ws.protocols(["v12.stomp", "v11.stomp", "v10.stomp"])
        .on_upgrade(move |socket| handle_stomp_socket(socket, state.broker.clone(), jwt_secret))
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(rest::index))
        .route("/health", get(rest::health))
        .route("/api/info", get(rest::info))
        .route("/api/stats", get(stats::stats))
        .route("/api/echo", post(rest::echo))
        .route("/api/mock", get(mock::mock_get).post(mock::mock_post))
        .route("/api/compressed", get(compressed_http::json_gzip))
        .route("/api/compressed-zstd", get(compressed_http::json_zstd))
        .route("/api/compressed-mp", get(compressed_http::msgpack_plain))
        .route("/api/compressed-mp-gzip", get(compressed_http::msgpack_gzip))
        .route("/api/compressed-mp-zstd", get(compressed_http::msgpack_zstd))
        .route("/api/compressed-octet", get(compressed_http::json_gzip_octet))
        .route(
            "/api/compressed-zstd-octet",
            get(compressed_http::json_zstd_octet),
        )
        .route(
            "/api/compressed-mp-octet",
            get(compressed_http::msgpack_plain_octet),
        )
        .route(
            "/api/compressed-mp-gzip-octet",
            get(compressed_http::msgpack_gzip_octet),
        )
        .route(
            "/api/compressed-mp-zstd-octet",
            get(compressed_http::msgpack_zstd_octet),
        )
        .route("/api/me", get(auth::handlers::me))
        .route("/auth/register", post(auth::handlers::register))
        .route("/auth/login", post(auth::handlers::login))
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/auth/logout", post(auth::handlers::logout))
        .route("/ws", get(ws::ws_handler))
        .route("/ws/secure", get(ws::ws_secure_handler))
        .route("/stomp", get(stomp_handler))
        // Layers apply outermost-first in REVERSE declaration order: the last
        // .layer() call here is the outermost/first-run gate. CORS is outermost
        // so every response — including 429s and 503s from the layers below —
        // still carries CORS headers (otherwise a browser can't even read the
        // rejection body). CPU breaker runs before the rate limiter (cheapest
        // possible rejection under load).
        .layer(CorsLayer::permissive())
        .layer(ratelimit::build_layer())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            cpu::cpu_breaker_mw,
        ))
        .with_state(state)
}
