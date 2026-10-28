//! 在 axum 每次接受 TCP 连接时分配进程内唯一序号。
use axum::{
    extract::{connect_info::Connected, ConnectInfo, Request},
    middleware::Next,
    response::Response,
    serve::IncomingStream,
};
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CONN: AtomicU64 = AtomicU64::new(1);

/// 与连接一起克隆到每个请求的元数据。
#[derive(Clone, Copy, Debug)]
pub struct ConnectionInfo {
    pub id: u64,
    pub peer_addr: SocketAddr,
}

impl Connected<IncomingStream<'_>> for ConnectionInfo {
    fn connect_info(stream: IncomingStream<'_>) -> Self {
        Self {
            id: NEXT_CONN
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("connection sequence exhausted"),
            peer_addr: stream.remote_addr(),
        }
    }
}

/// 保留原有 SocketAddr 扩展，供非 testkit 路由的 IP 限流使用。
pub(crate) async fn peer_info(mut request: Request, next: Next) -> Response {
    if let Some(ConnectInfo(info)) = request
        .extensions()
        .get::<ConnectInfo<ConnectionInfo>>()
        .copied()
    {
        request.extensions_mut().insert(ConnectInfo(info.peer_addr));
    }
    next.run(request).await
}
