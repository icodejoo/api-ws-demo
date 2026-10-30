//! 可被 profile 广播中断的双向 TCP 转发。
use super::{
    bucket::Bucket,
    host::HostRewriter,
    profile::{chance, random, Mode, Profile, CHUNK},
    tls,
    upstream::Upstream,
    ProxyStats,
};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::watch,
};

/// 到上游的双向字节流（明文 TCP 或 TLS），可对底层 TCP 注入 RST。
pub(super) trait Duplex: AsyncRead + AsyncWrite + Send + Unpin {
    /// 对底层 TCP 注入 RST。
    fn reset(&self);
}

impl Duplex for TcpStream {
    /// 设置 SO_LINGER=0，关闭时发 RST。
    fn reset(&self) {
        reset(self);
    }
}

/// 连接上游：TCP 连接成功后，若声明了 SNI 则完成 TLS 握手。
async fn dial(upstream: &Upstream) -> io::Result<Box<dyn Duplex>> {
    let tcp = TcpStream::connect(&upstream.connect).await?;
    let _ = tcp.set_nodelay(true);
    match &upstream.sni {
        Some(sni) => tls::connect(tcp, sni).await,
        None => Ok(Box::new(tcp)),
    }
}

/// 一条连接在所有退出路径都会减少 active。
struct Active {
    /// 该代理的共享统计。
    stats: Arc<ProxyStats>,
}

impl Drop for Active {
    /// 连接任务取消也准确减少在途数。
    fn drop(&mut self) {
        self.stats.active.fetch_sub(1, Ordering::Relaxed);
    }
}

/// 以 SO_LINGER=0 注入 RST，不引入额外系统依赖。
#[allow(deprecated)]
fn reset(stream: &TcpStream) {
    let _ = stream.set_linger(Some(Duration::ZERO));
}

/// 运行一条连接；done 广播会取消并关闭上下游。
pub(super) async fn connection(
    mut client: TcpStream,
    upstream: Upstream,
    mut profile: watch::Receiver<Profile>,
    mut done: watch::Receiver<bool>,
    stats: Arc<ProxyStats>,
    number: u64,
) {
    stats.active.fetch_add(1, Ordering::Relaxed);
    let _active = Active {
        stats: stats.clone(),
    };
    let reset_flag = Arc::new(AtomicBool::new(false));
    let work = async {
        // blackhole 不建立上游连接；切回 pass 后原连接仍可继续。
        loop {
            let mode = profile.borrow().mode;
            match mode {
                Mode::Pass => break,
                Mode::Reset => {
                    reset_flag.store(true, Ordering::Relaxed);
                    return;
                }
                Mode::Refuse => return,
                Mode::Blackhole => {
                    if profile.changed().await.is_err() {
                        return;
                    }
                }
            }
        }
        let connecting = dial(&upstream);
        tokio::pin!(connecting);
        let server = loop {
            tokio::select! {
                value = &mut connecting => break value,
                value = profile.changed() => {
                    if value.is_err() { return; }
                    match profile.borrow().mode {
                        Mode::Reset => { reset_flag.store(true,Ordering::Relaxed); return; },
                        Mode::Refuse => return,
                        _ => {},
                    }
                }
            }
        };
        let Ok(server) = server else {
            return;
        };
        let _ = client.set_nodelay(true);
        {
            let (read_client, write_client) = client.split();
            let (mut read_server, mut write_server) = tokio::io::split(server);
            let rewriter = upstream.host.as_deref().map(HostRewriter::new);
            let result = tokio::try_join!(
                pump(
                    read_client,
                    &mut write_server,
                    profile.clone(),
                    stats.clone(),
                    false,
                    number,
                    reset_flag.clone(),
                    rewriter
                ),
                pump(
                    &mut read_server,
                    write_client,
                    profile.clone(),
                    stats.clone(),
                    true,
                    number,
                    reset_flag.clone(),
                    None
                )
            );
            if result.is_err() && reset_flag.load(Ordering::Relaxed) {
                read_server.unsplit(write_server).reset();
            }
        }
    };
    tokio::select! {
        _ = work => {},
        _ = done.changed() => {},
    }
    if reset_flag.load(Ordering::Relaxed) {
        reset(&client);
        stats.resets.fetch_add(1, Ordering::Relaxed);
    }
}

/// 控制切换时检查模式；blackhole 保留分块和偏移以便恢复。
async fn pass(
    profile: &mut watch::Receiver<Profile>,
    reset_flag: &AtomicBool,
) -> io::Result<Profile> {
    loop {
        let p = profile.borrow().clone();
        match p.mode {
            Mode::Pass => return Ok(p),
            Mode::Reset => {
                reset_flag.store(true, Ordering::Relaxed);
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            Mode::Refuse => return Err(io::ErrorKind::ConnectionAborted.into()),
            Mode::Blackhole => profile
                .changed()
                .await
                .map_err(|_| io::Error::from(io::ErrorKind::ConnectionAborted))?,
        }
    }
}

/// 两种重置条件同时存在时取最早字节阈值。
fn threshold(profile: &Profile, probabilistic: Option<u64>, down: bool) -> Option<u64> {
    if down {
        profile
            .reset_after_bytes
            .into_iter()
            .chain(probabilistic)
            .min()
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
/// 转发单方向；所有 I/O 与等待都监听配置变化。`rewriter` 仅上行 TLS 模式使用，改写 Host 头。
async fn pump<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut reader: R,
    mut writer: W,
    mut profile: watch::Receiver<Profile>,
    stats: Arc<ProxyStats>,
    down: bool,
    number: u64,
    reset_flag: Arc<AtomicBool>,
    mut rewriter: Option<HostRewriter>,
) -> io::Result<()> {
    let mut buffer = [0u8; CHUNK];
    let mut bucket = Bucket::new();
    let mut total = 0u64;
    let mut first = true;
    let mut seed = profile.borrow().seed;
    let mut rng = seed ^ number.wrapping_mul(2) ^ u64::from(down);
    let mut reset_at = None;
    let mut reset_checked = false;
    let mut reset_probability = profile.borrow().reset_prob;
    loop {
        let p = pass(&mut profile, &reset_flag).await?;
        if threshold(&p, reset_at, down).is_some_and(|limit| total >= limit) {
            reset_flag.store(true, Ordering::Relaxed);
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        let count = tokio::select! {
            value = reader.read(&mut buffer) => value?,
            _ = profile.changed() => continue,
        };
        if count == 0 {
            writer.shutdown().await?;
            return Ok(());
        }
        // 延迟与限速仍按客户端原始字节数计，写出的是改写后的字节。
        let rewritten;
        let payload: &[u8] = match rewriter.as_mut() {
            Some(r) => {
                rewritten = r.feed(&buffer[..count]);
                &rewritten
            }
            None => &buffer[..count],
        };
        loop {
            let p = pass(&mut profile, &reset_flag).await?;
            if seed != p.seed {
                seed = p.seed;
                rng = seed ^ number.wrapping_mul(2) ^ u64::from(down);
                reset_checked = false;
            }
            if reset_probability != p.reset_prob {
                reset_probability = p.reset_prob;
                reset_checked = false;
            }
            if down && !reset_checked {
                reset_checked = true;
                reset_at = None;
                if chance(&mut rng) < p.reset_prob {
                    reset_at = Some(total + 1 + random(&mut rng) % CHUNK as u64);
                }
            }
            let limit = threshold(&p, reset_at, down);
            if limit.is_some_and(|limit| total >= limit) {
                reset_flag.store(true, Ordering::Relaxed);
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            let jitter = if p.jitter_ms == 0 {
                0
            } else {
                (random(&mut rng) % (p.jitter_ms * 2 + 1)) as i64 - p.jitter_ms as i64
            };
            let mut delay = Duration::from_millis((p.latency_ms as i64 + jitter).max(0) as u64);
            if first {
                delay += Duration::from_millis(p.first_byte_ms);
            }
            if chance(&mut rng) < p.stall_prob {
                delay += Duration::from_millis(p.stall_ms);
            }
            delay += bucket.reserve(
                count,
                if down { p.down_kbps } else { p.up_kbps },
                Instant::now(),
            );
            tokio::select! {
                _ = tokio::time::sleep(delay) => {},
                _ = profile.changed() => { bucket.refund(count); continue; },
            }
            break;
        }
        let mut offset = 0;
        while offset < payload.len() {
            let p = pass(&mut profile, &reset_flag).await?;
            let limit = threshold(&p, reset_at, down);
            let remaining = payload.len() - offset;
            let available = limit
                .map(|v| v.saturating_sub(total).min(remaining as u64) as usize)
                .unwrap_or(remaining);
            if available == 0 {
                reset_flag.store(true, Ordering::Relaxed);
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            let wrote = tokio::select! {
                value = writer.write(&payload[offset..offset+available]) => value?,
                _ = profile.changed() => continue,
            };
            if wrote == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            offset += wrote;
            total += wrote as u64;
            if down {
                stats.bytes_down.fetch_add(wrote as u64, Ordering::Relaxed);
            } else {
                stats.bytes_up.fetch_add(wrote as u64, Ordering::Relaxed);
            }
            if limit.is_some_and(|v| total >= v) {
                reset_flag.store(true, Ordering::Relaxed);
                return Err(io::ErrorKind::ConnectionReset.into());
            }
        }
        first = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// 在真实套接字上直接运行转发核心，返回客户端和可切换的配置句柄。
    async fn pair(
        initial: Profile,
    ) -> (
        TcpStream,
        watch::Sender<Profile>,
        watch::Sender<bool>,
        tokio::task::JoinHandle<()>,
    ) {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = upstream.accept().await.unwrap();
            let mut bytes = [0u8; CHUNK];
            loop {
                match socket.read(&mut bytes).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if socket.write_all(&bytes[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let (profiles, receiver) = watch::channel(initial);
        let (done, done_rx) = watch::channel(false);
        let task = tokio::spawn(connection(
            server,
            upstream_addr.into(),
            receiver,
            done_rx,
            Arc::new(ProxyStats::default()),
            0,
        ));
        (client, profiles, done, task)
    }

    /// 起一个只记录收到字节的上游，返回地址与接收通道。
    async fn sink() -> (String, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0u8; CHUNK];
            while let Ok(n) = socket.read(&mut bytes).await {
                if n == 0 || tx.send(bytes[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        (addr, rx)
    }

    /// 在真实套接字上接入一条带上游描述的连接，返回客户端。
    async fn dial_via(
        upstream: Upstream,
    ) -> (TcpStream, watch::Sender<Profile>, watch::Sender<bool>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let (profiles, receiver) = watch::channel(Profile::default());
        let (done, done_rx) = watch::channel(false);
        tokio::spawn(connection(
            server,
            upstream,
            receiver,
            done_rx,
            Arc::new(ProxyStats::default()),
            0,
        ));
        (client, profiles, done)
    }

    /// 配置了 Host 改写时，keep-alive 上的两个请求都被改写，且限速仍作用在客户端侧。
    #[tokio::test]
    async fn host_rewritten_on_every_request() {
        let (addr, mut rx) = sink().await;
        let upstream = Upstream {
            connect: addr,
            sni: None,
            host: Some("api.example.com".into()),
        };
        let (mut client, _profiles, _done) = dial_via(upstream).await;
        client
            .write_all(b"GET /a HTTP/1.1\r\nHost: 127.0.0.1:9\r\n\r\n")
            .await
            .unwrap();
        client
            .write_all(b"GET /b HTTP/1.1\r\nHost: 127.0.0.1:9\r\n\r\n")
            .await
            .unwrap();
        let mut got = Vec::new();
        while got.len() < 84 {
            let chunk = tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap();
            got.extend(chunk);
        }
        let text = String::from_utf8(got).unwrap();
        assert_eq!(
            text.matches("Host: api.example.com\r\n").count(),
            2,
            "{text}"
        );
        assert!(!text.contains("127.0.0.1:9"));
    }

    /// 上游不是 TLS 服务时握手失败，客户端连接被干净关闭而不是挂住。
    #[tokio::test]
    async fn tls_handshake_failure_closes_client() {
        // 明文 HTTP 服务器收到 ClientHello 会回一段非 TLS 数据并关闭。
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        });
        let upstream = Upstream {
            connect: addr,
            sni: Some("localhost".into()),
            host: Some("localhost".into()),
        };
        let (mut client, _profiles, _done) = dial_via(upstream).await;
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(5), client.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(read, Ok(0) | Err(_)));
    }

    /// 黑洞切换能保持字节完整，已有连接立即响应 reset。
    #[tokio::test]
    async fn live_blackhole_and_reset() {
        let (mut stream, profiles, _done, task) = pair(Profile::default()).await;
        stream.write_all(b"a").await.unwrap();
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await.unwrap();
        profiles.send_replace(Profile {
            mode: Mode::Blackhole,
            ..Profile::default()
        });
        stream.write_all(b"b").await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(30), stream.read_exact(&mut byte))
                .await
                .is_err()
        );
        profiles.send_replace(Profile::default());
        tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut byte))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(byte, [b'b']);
        profiles.send_replace(Profile {
            mode: Mode::Reset,
            ..Profile::default()
        });
        assert_eq!(
            stream.read(&mut byte).await.unwrap_err().kind(),
            io::ErrorKind::ConnectionReset
        );
        task.await.unwrap();
    }

    /// 真实 64KiB 字节流被令牌桶限制至约两秒。
    #[tokio::test]
    async fn rate_and_byte_threshold() {
        let (mut stream, profiles, _done, task) = pair(Profile {
            down_kbps: 256,
            ..Profile::default()
        })
        .await;
        let bytes = vec![b'x'; 64 * 1024];
        let mut out = vec![0u8; bytes.len()];
        let start = Instant::now();
        stream.write_all(&bytes).await.unwrap();
        tokio::time::timeout(Duration::from_secs(6), stream.read_exact(&mut out))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bytes, out);
        assert!(start.elapsed() >= Duration::from_millis(1800));
        // 新阈值已经小于累计下行字节，无需再发包就应重置。
        profiles.send_replace(Profile {
            reset_after_bytes: Some(100),
            ..Profile::default()
        });
        let mut byte = [0u8; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap_err()
                .kind(),
            io::ErrorKind::ConnectionReset
        );
        task.await.unwrap();
    }
}
