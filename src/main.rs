mod auth;
mod compressed_assets;
mod compressed_http;
mod cpu;
mod mock;
mod ratelimit;
mod response;
mod rest;
mod routes;
mod state;
mod stomp;
mod ws;

use std::net::SocketAddr;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cpu_usage = cpu::spawn_sampler();
    let state = AppState::new(cpu_usage);
    let app = routes::build_router(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!(%addr, "starting api-ws-demo");
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

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
