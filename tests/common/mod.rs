//! 集成测试共用的真实套接字 HTTP/1.1 客户端。
#![allow(dead_code)]
use axum::Router;
use serde_json::Value;
use std::{io, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

/// 常规集成请求的最长等待。
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// 测试轮次头。
pub const RUN_HEADER: &str = "X-Test-Run: integration\r\n";

/// 测试服务器生命周期，离开作用域时停止监听。
pub struct Server {
    /// 随机端口地址。
    pub addr: SocketAddr,
    /// 监听任务。
    task: JoinHandle<()>,
}

impl Drop for Server {
    /// 防止测试失败后遗留服务。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 启动指定路由，参数为路由，返回带随机端口的句柄。
pub async fn start(router: Router) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<api_ws_demo::connection::ConnectionInfo>(),
        )
        .await
        .unwrap();
    });
    Server { addr, task }
}

/// 发送完整请求；headers 必须已带每行 CRLF。
pub async fn send(
    stream: &mut TcpStream,
    method: &str,
    path: &str,
    headers: &str,
    body: &str,
) -> io::Result<()> {
    let request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}\r\n{body}",body.len());
    stream.write_all(request.as_bytes()).await
}

/// 获取原始 HTTP 字节，连接级错误直接返回。
pub async fn raw(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &str,
    body: &str,
) -> io::Result<Vec<u8>> {
    tokio::time::timeout(TIMEOUT, async {
        let mut stream = TcpStream::connect(addr).await?;
        send(&mut stream, method, path, headers, body).await?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await?;
        Ok(bytes)
    })
    .await
    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
}

/// 解析 HTTP 状态及响应体，包含 chunked 解码。
pub fn decode(bytes: &[u8]) -> (u16, Vec<u8>) {
    let split = bytes
        .windows(4)
        .position(|v| v == b"\r\n\r\n")
        .expect("HTTP headers");
    let headers = std::str::from_utf8(&bytes[..split]).unwrap();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut body = &bytes[split + 4..];
    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut out = Vec::new();
        loop {
            let end = body
                .windows(2)
                .position(|v| v == b"\r\n")
                .expect("chunk length");
            let len = usize::from_str_radix(
                std::str::from_utf8(&body[..end])
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap(),
                16,
            )
            .unwrap();
            body = &body[end + 2..];
            if len == 0 {
                break;
            }
            assert!(body.len() >= len + 2, "truncated chunk");
            out.extend_from_slice(&body[..len]);
            body = &body[len + 2..];
        }
        (status, out)
    } else {
        (status, body.to_vec())
    }
}

/// 发起 JSON 请求，返回 HTTP 状态与 JSON。
pub async fn json(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &str,
    body: Value,
) -> (u16, Value) {
    let bytes = raw(addr, method, path, headers, &body.to_string())
        .await
        .unwrap();
    let (status, body) = decode(&bytes);
    (status, serde_json::from_slice(&body).unwrap())
}
