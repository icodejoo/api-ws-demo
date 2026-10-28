//! 本地网络模拟器命令行入口。
use std::net::SocketAddr;

/// 解析 --control 并启动 HTTP 控制 API，返回套接字或参数错误。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let control = match (args.next().as_deref(), args.next(), args.next()) {
        (None, None, None) => "127.0.0.1:47090".parse::<SocketAddr>()?,
        (Some("--control"), Some(addr), None) => addr.parse()?,
        _ => return Err("usage: netem [--control 127.0.0.1:47090]".into()),
    };
    let listener = tokio::net::TcpListener::bind(control).await?;
    eprintln!("netem control http://{}", listener.local_addr()?);
    axum::serve(listener, api_ws_demo::netem::control_router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
