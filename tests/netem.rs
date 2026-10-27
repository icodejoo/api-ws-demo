//! 网络模拟器真实 TCP 表现验收。
mod common;
use common::{json, raw, start, Server};
use serde_json::{json, Value};
use std::{
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

/// 控制 API 创建路径。
const PROXIES: &str = "/proxies";
/// 等待一个短网络事件。
const SHORT: Duration = Duration::from_millis(100);

/// 只回显 TCP 字节的上游，避免 HTTP 缓冲掩盖网络行为。
struct Upstream {
    /// 随机地址。
    addr: SocketAddr,
    /// 监听任务。
    task: JoinHandle<()>,
}

impl Drop for Upstream {
    /// 测试退出时停止接受连接。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 创建无缓冲 TCP echo 上游。
async fn upstream() -> Upstream {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if stream.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });
    Upstream { addr, task }
}

/// 创建代理并返回 id 与代理 TCP 地址。
async fn proxy(control: &Server, upstream: SocketAddr, profile: Value) -> (String, SocketAddr) {
    let (status, v) = json(
        control.addr,
        "POST",
        PROXIES,
        "",
        json!({"upstream":upstream.to_string(),"profile":profile}),
    )
    .await;
    assert_eq!(status, 200);
    (
        v["id"].as_str().unwrap().into(),
        SocketAddr::from(([127, 0, 0, 1], v["port"].as_u64().unwrap() as u16)),
    )
}

/// 切换代理配置并确认 API 成功。
async fn update(control: &Server, id: &str, profile: Value) {
    assert_eq!(
        json(
            control.addr,
            "PUT",
            &format!("{PROXIES}/{id}"),
            "",
            json!({"profile":profile})
        )
        .await
        .0,
        200
    );
}

/// 监听拒绝、恢复、已有连接黑洞/恢复、立即 RST 和删除。
#[tokio::test]
async fn modes_and_live_changes() {
    let up = upstream().await;
    let control = start(api_ws_demo::netem::control_router()).await;
    let (id, addr) = proxy(&control, up.addr, json!("offline")).await;
    assert_eq!(
        TcpStream::connect(addr).await.unwrap_err().kind(),
        io::ErrorKind::ConnectionRefused
    );
    update(&control, &id, json!("normal")).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(b"a").await.unwrap();
    let mut byte = [0u8; 1];
    stream.read_exact(&mut byte).await.unwrap();
    assert_eq!(byte, b"a"[..]);
    update(&control, &id, json!("blackhole")).await;
    stream.write_all(b"b").await.unwrap();
    assert!(tokio::time::timeout(SHORT, stream.read_exact(&mut byte))
        .await
        .is_err());
    update(&control, &id, json!("normal")).await;
    tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut byte))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(byte, b"b"[..]);
    update(&control, &id, json!({"mode":"reset"})).await;
    let err = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
    let mut stream = TcpStream::connect(addr).await.unwrap();
    assert_eq!(
        stream.read(&mut byte).await.unwrap_err().kind(),
        io::ErrorKind::ConnectionReset
    );
    let (_, stats) = json(
        control.addr,
        "GET",
        &format!("{PROXIES}/{id}/stats"),
        "",
        Value::Null,
    )
    .await;
    assert!(stats["resets"].as_u64().unwrap() >= 1);
    assert!(stats["bytes_down"].as_u64().unwrap() >= 2);
    update(&control, &id, json!("offline")).await;
    assert_eq!(
        TcpStream::connect(addr).await.unwrap_err().kind(),
        io::ErrorKind::ConnectionRefused
    );
    update(&control, &id, json!("normal")).await;
    let mut live = TcpStream::connect(addr).await.unwrap();
    live.write_all(b"c").await.unwrap();
    live.read_exact(&mut byte).await.unwrap();
    assert_eq!(
        json(
            control.addr,
            "DELETE",
            &format!("{PROXIES}/{id}"),
            "",
            Value::Null
        )
        .await
        .0,
        200
    );
    assert_eq!(
        TcpStream::connect(addr).await.unwrap_err().kind(),
        io::ErrorKind::ConnectionRefused
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(2), live.read(&mut byte))
            .await
            .is_ok()
    );
}

/// 新黑洞连接不回包、64KiB 限速耗时、字节阈值 RST。
#[tokio::test]
async fn shaping_and_threshold_reset() {
    let up = upstream().await;
    let control = start(api_ws_demo::netem::control_router()).await;
    let (id, addr) = proxy(&control, up.addr, json!("blackhole")).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(b"a").await.unwrap();
    let mut byte = [0u8; 1];
    assert!(tokio::time::timeout(SHORT, stream.read(&mut byte))
        .await
        .is_err());
    update(&control, &id, json!({"down_kbps":256,"seed":7})).await;
    drop(stream);
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let bytes = vec![b'x'; 64 * 1024];
    let at = Instant::now();
    stream.write_all(&bytes).await.unwrap();
    let mut got = vec![0; bytes.len()];
    tokio::time::timeout(Duration::from_secs(6), stream.read_exact(&mut got))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bytes, got);
    assert!(
        at.elapsed() >= Duration::from_millis(1800) && at.elapsed() < Duration::from_secs(6),
        "elapsed {:?}",
        at.elapsed()
    );
    update(&control, &id, json!({"reset_after_bytes":100})).await;
    drop(stream);
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&bytes).await.unwrap();
    let mut got = Vec::new();
    let err = stream.read_to_end(&mut got).await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
    assert!(got.len() <= 100);
}

/// flap 内置时间表及自定义调度，切换 PUT 会取消旧时间表。
#[tokio::test]
async fn flap_and_schedule() {
    let up = upstream().await;
    let control = start(api_ws_demo::netem::control_router()).await;
    let (id, addr) = proxy(&control, up.addr, json!("flap")).await;
    assert_offline_window(addr, SHORT).await;
    tokio::time::sleep(Duration::from_millis(1650)).await;
    assert!(TcpStream::connect(addr).await.is_ok());
    let schedule_path = format!("{PROXIES}/{id}/schedule");
    assert_eq!(
        json(
            control.addr,
            "POST",
            &schedule_path,
            "",
            json!([{"at_ms":20,"profile":"offline"},{"at_ms":200,"profile":"normal"}])
        )
        .await
        .0,
        200
    );
    tokio::time::sleep(Duration::from_millis(70)).await;
    assert_offline_window(addr, Duration::from_millis(50)).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(TcpStream::connect(addr).await.is_ok());
    json(
        control.addr,
        "POST",
        &schedule_path,
        "",
        json!([{"at_ms":50,"profile":"offline"}]),
    )
    .await;
    update(&control, &id, json!("normal")).await;
    tokio::time::sleep(SHORT).await;
    assert!(TcpStream::connect(addr).await.is_ok());
    let (_, presets) = json(control.addr, "GET", "/presets", "", Value::Null).await;
    assert_eq!(presets["flap"][1]["at_ms"], 1500);
    assert_eq!(
        raw(control.addr, "GET", "/presets", "", "").await.unwrap()[..8],
        *b"HTTP/1.1"
    );
}

/// 在恢复前完成探测；Windows 的 SYN 重试不能跨过时间表恢复点。
async fn assert_offline_window(address: SocketAddr, window: Duration) {
    match tokio::time::timeout(window, TcpStream::connect(address)).await {
        Ok(Err(error)) => assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused),
        Err(_) => {}
        Ok(Ok(_)) => panic!("offline window accepted a connection"),
    }
}
