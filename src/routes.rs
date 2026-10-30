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

/// 用共享状态构建路由；TEST_MODE=1 跳过限流与 CPU 熔断。
/// 参数为应用状态；启动时使用 `into_make_service_with_connect_info::<crate::connection::ConnectionInfo>()`。
pub fn build_router(state: AppState) -> Router {
    let app = Router::new()
        .merge(crate::testkit::router())
        .route("/", get(rest::index))
        .route("/health", get(rest::health))
        .route("/api/info", get(rest::info))
        .route("/api/stats", get(stats::stats))
        .route("/api/echo", post(rest::echo))
        .route("/api/mock", get(mock::mock_get).post(mock::mock_post))
        .route("/api/compressed", get(compressed_http::json_gzip))
        .route("/api/compressed-zstd", get(compressed_http::json_zstd))
        .route("/api/compressed-mp", get(compressed_http::msgpack_plain))
        .route(
            "/api/compressed-mp-gzip",
            get(compressed_http::msgpack_gzip),
        )
        .route(
            "/api/compressed-mp-zstd",
            get(compressed_http::msgpack_zstd),
        )
        .route(
            "/api/compressed-octet",
            get(compressed_http::json_gzip_octet),
        )
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
        .route("/stomp", get(stomp_handler));
    // Layers apply outermost-first in REVERSE declaration order: the last
    // .layer() call here is the outermost/first-run gate. CORS is outermost
    // so every response — including 429s and 503s from the layers below —
    // still carries CORS headers (otherwise a browser can't even read the
    // rejection body). CPU breaker runs before the rate limiter (cheapest
    // possible rejection under load).
    let app = app.with_state(state.clone());
    let app = if std::env::var("TEST_MODE").as_deref() == Ok("1") {
        app
    } else {
        let breaker = axum::middleware::from_fn_with_state(state, cpu::cpu_breaker_mw);
        // 熔断对所有请求生效；限流只作用于 limited 分支。
        let plain = app.clone().layer(breaker.clone());
        let limited = app.layer(ratelimit::build_layer()).layer(breaker);
        with_bypass(ratelimit::Bypass::from_env(), plain, limited)
    };
    app.layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(crate::connection::peer_info))
}

/// 按 x-testkit-token 分流：token 正确走 plain（无限流），否则走 limited。
/// 未配置 RATE_LIMIT_BYPASS_TOKEN 时直接返回 limited，行为与原先完全一致。
fn with_bypass(bypass: ratelimit::Bypass, plain: Router, limited: Router) -> Router {
    if !bypass.enabled() {
        return limited;
    }
    Router::new().fallback(move |request: axum::extract::Request| {
        let target = if bypass.allows(request.headers()) {
            plain.clone()
        } else {
            limited.clone()
        };
        async move {
            match tower::ServiceExt::oneshot(target, request).await {
                Ok(response) => response,
                Err(never) => match never {},
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// 构造极小的被测路由：限流为突发 2、几乎不补充，对比两个分支。
    fn app(bypass: ratelimit::Bypass) -> Router {
        std::env::set_var("RATE_LIMIT_BURST", "2");
        std::env::set_var("RATE_LIMIT_PER_SECOND", "1");
        let base = Router::new().route("/p", get(|| async { "ok" }));
        let limited = base.clone().layer(ratelimit::build_layer());
        with_bypass(bypass, base, limited)
    }

    /// 发 n 次请求，返回状态码列表。
    async fn hit(app: &Router, token: Option<&str>, n: usize) -> Vec<StatusCode> {
        let mut out = Vec::new();
        for _ in 0..n {
            let mut req = Request::builder()
                .uri("/p")
                .header("x-forwarded-for", "10.0.0.1");
            if let Some(t) = token {
                req = req.header(ratelimit::BYPASS_HEADER, t);
            }
            let res = app
                .clone()
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            out.push(res.status());
        }
        out
    }

    /// 正确 token 不被限；错误、缺失 token 超出突发后 429；未配置变量时带 token 也被限。
    #[tokio::test]
    async fn bypass_routing() {
        let on = app(ratelimit::Bypass::new(Some("s3cret")));
        assert!(hit(&on, Some("s3cret"), 8)
            .await
            .iter()
            .all(|s| *s == StatusCode::OK));
        assert!(hit(&on, Some("bad"), 8)
            .await
            .contains(&StatusCode::TOO_MANY_REQUESTS));
        let on = app(ratelimit::Bypass::new(Some("s3cret")));
        assert!(hit(&on, None, 8)
            .await
            .contains(&StatusCode::TOO_MANY_REQUESTS));
        for off in [ratelimit::Bypass::new(None), ratelimit::Bypass::new(Some(""))] {
            let off = app(off);
            assert!(hit(&off, Some("s3cret"), 8)
                .await
                .contains(&StatusCode::TOO_MANY_REQUESTS));
        }
    }
}
