use api_ws_demo::{cpu, routes, state};

use std::net::SocketAddr;

use state::AppState;

#[tokio::main]
/// 从环境读取端口，启动主服务与 CPU 采样。
async fn main() {
    let cpu_usage = cpu::spawn_sampler();
    let state = AppState::new(cpu_usage);
    let app = routes::build_router(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind port");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server error");
}

/// 收到 Ctrl+C 后触发优雅停止。
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
