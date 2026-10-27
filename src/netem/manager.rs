//! 代理监听器、配置切换与时间表的生命周期。
use super::{
    profile::{Mode, Profile},
    relay,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, watch},
};
/// 连接与流量计数，查询时读取快照。
#[derive(Default)]
pub(super) struct ProxyStats {
    /// 累计接受的连接。
    pub(super) connections: AtomicU64,
    /// 当前连接。
    pub(super) active: AtomicU64,
    /// 已转发上行字节。
    pub(super) bytes_up: AtomicU64,
    /// 已转发下行字节。
    pub(super) bytes_down: AtomicU64,
    /// 注入的 RST 数。
    pub(super) resets: AtomicU64,
    /// 进入拒绝监听状态的次数（内核拒绝尝试无法逐次观测）。
    pub(super) refused: AtomicU64,
}

/// 发给监听循环的完整 profile 切换命令。
pub(super) struct Change {
    /// 新配置。
    pub(super) profile: Profile,
    /// 配置代际；陈旧时间表不可覆盖新操作。
    pub(super) generation: u64,
    /// 已应用配置的确认。
    pub(super) ack: oneshot::Sender<Result<(), String>>,
}

/// 一个可独立关闭的代理。
pub(super) struct Proxy {
    /// 分配后固定的监听端口。
    pub(super) port: u16,
    /// 切换命令队列。
    pub(super) changes: mpsc::Sender<Change>,
    /// 广播关闭信号。
    pub(super) done: watch::Sender<bool>,
    /// 当前调度代际。
    pub(super) generation: AtomicU64,
    /// 原子统计。
    pub(super) stats: Arc<ProxyStats>,
    /// 唯一当前时间表任务，替换时立即取消旧任务。
    pub(super) timer: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Drop for Proxy {
    /// 控制服务器和代理一起退出时关闭所有连接。
    fn drop(&mut self) {
        let _ = self.done.send(true);
        if let Some(timer) = self.timer.get_mut().unwrap().take() {
            timer.abort();
        }
    }
}

/// 创建代理的监听器与生命周期，返回共享句柄或绑定错误。
pub(super) async fn create(upstream: String, profile: Profile) -> std::io::Result<Arc<Proxy>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (changes, receiver) = mpsc::channel(16);
    let (done, done_rx) = watch::channel(false);
    let stats = Arc::new(ProxyStats::default());
    let proxy = Arc::new(Proxy {
        port,
        changes,
        done,
        generation: AtomicU64::new(0),
        stats: stats.clone(),
        timer: Mutex::new(None),
    });
    let listener = if profile.mode == Mode::Refuse {
        stats.refused.fetch_add(1, Ordering::Relaxed);
        drop(listener);
        None
    } else {
        Some(listener)
    };
    tokio::spawn(listen(
        listener,
        upstream,
        profile,
        receiver,
        done_rx,
        Arc::downgrade(&proxy),
    ));
    Ok(proxy)
}
/// 向监听循环发送命令并等待真正完成监听切换。
pub(super) async fn apply(proxy: &Proxy, profile: Profile, generation: u64) -> Result<(), String> {
    let (ack, rx) = oneshot::channel();
    proxy
        .changes
        .send(Change {
            profile,
            generation,
            ack,
        })
        .await
        .map_err(|_| "proxy closed")?;
    rx.await.map_err(|_| "proxy closed")?
}

/// 时间表持弱引用，删除代理后不会让监听器复活。
pub(super) fn start_schedule(proxy: &Arc<Proxy>, steps: Vec<(u64, Profile)>, generation: u64) {
    let weak = Arc::downgrade(proxy);
    let origin = tokio::time::Instant::now();
    let task = tokio::spawn(async move {
        for (at, p) in steps {
            tokio::time::sleep_until(origin + Duration::from_millis(at)).await;
            let Some(proxy) = weak.upgrade() else { break };
            if proxy.generation.load(Ordering::SeqCst) != generation || *proxy.done.borrow() {
                break;
            }
            if apply(&proxy, p, generation).await.is_err() {
                break;
            }
        }
    });
    if let Some(previous) = proxy.timer.lock().unwrap().replace(task) {
        previous.abort();
    }
}

/// 监听与切换串行化，连接各自订阅最新配置。
async fn listen(
    mut listener: Option<TcpListener>,
    upstream: String,
    initial: Profile,
    mut changes: mpsc::Receiver<Change>,
    mut done: watch::Receiver<bool>,
    weak: std::sync::Weak<Proxy>,
) {
    let (profiles, _) = watch::channel(initial);
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = done.changed() => break,
            command = changes.recv() => {
                let Some(command) = command else {break};
                let Some(proxy) = weak.upgrade() else {break};
                if command.generation != proxy.generation.load(Ordering::SeqCst) {let _ = command.ack.send(Ok(()));continue}
                let mut result = Ok(());
                if command.profile.mode == Mode::Refuse {
                    if listener.take().is_some() {proxy.stats.refused.fetch_add(1,Ordering::Relaxed);}
                } else if listener.is_none() {
                    match TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,proxy.port)).await {
                        Ok(value)=>listener=Some(value),Err(e)=>result=Err(e.to_string()),
                    }
                }
                if result.is_ok() {profiles.send_replace(command.profile);}
                let _ = command.ack.send(result);
            }
            accepted = async { match &listener {Some(l)=>l.accept().await,None=>std::future::pending().await} } => {
                if let Ok((stream,_)) = accepted {
                    let Some(proxy) = weak.upgrade() else {break};
                    let number = proxy.stats.connections.fetch_add(1,Ordering::Relaxed);
                    connections.spawn(relay::connection(stream,upstream.clone(),profiles.subscribe(),done.clone(),proxy.stats.clone(),number));
                }
            }
            _ = connections.join_next(), if !connections.is_empty() => {},
        }
    }
    drop(listener);
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io,
        net::{Ipv4Addr, SocketAddr},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };

    /// 真实连接在 refuse 后必须拒绝，恢复时仍使用原端口。
    #[tokio::test]
    async fn refuse_restores_same_port_and_closes_connections() {
        let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = upstream.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = upstream.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut byte = [0u8; 1];
                    while stream.read_exact(&mut byte).await.is_ok() {
                        if stream.write_all(&byte).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        let proxy = create(
            address,
            Profile {
                mode: Mode::Refuse,
                ..Profile::default()
            },
        )
        .await
        .unwrap();
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, proxy.port));
        assert_eq!(
            TcpStream::connect(address).await.unwrap_err().kind(),
            io::ErrorKind::ConnectionRefused
        );
        apply(&proxy, Profile::default(), 0).await.unwrap();
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(b"a").await.unwrap();
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await.unwrap();
        apply(
            &proxy,
            Profile {
                mode: Mode::Refuse,
                ..Profile::default()
            },
            0,
        )
        .await
        .unwrap();
        assert_eq!(
            TcpStream::connect(address).await.unwrap_err().kind(),
            io::ErrorKind::ConnectionRefused
        );
        apply(&proxy, Profile::default(), 0).await.unwrap();
        let mut next = TcpStream::connect(address).await.unwrap();
        next.write_all(b"b").await.unwrap();
        next.read_exact(&mut byte).await.unwrap();
        proxy.done.send(true).unwrap();
        proxy.changes.closed().await;
        assert_eq!(
            TcpStream::connect(address).await.unwrap_err().kind(),
            io::ErrorKind::ConnectionRefused
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(2), next.read(&mut byte))
                .await
                .is_ok()
        );
    }

    /// 时间表会在指定时刻恢复监听，代际变化阻止旧时间表覆盖新配置。
    #[tokio::test]
    async fn flap_and_generation() {
        let proxy = create(
            "127.0.0.1:1".into(),
            Profile {
                mode: Mode::Refuse,
                ..Profile::default()
            },
        )
        .await
        .unwrap();
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, proxy.port));
        start_schedule(
            &proxy,
            vec![(super::super::profile::FLAP_MS, Profile::default())],
            0,
        );
        match tokio::time::timeout(Duration::from_millis(100), TcpStream::connect(address)).await {
            Ok(Err(error)) => assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused),
            Err(_) => {} // Windows 会重试 SYN；只探测本次离线窗口。
            Ok(Ok(_)) => panic!("offline window must not accept a connection"),
        }
        tokio::time::sleep(Duration::from_millis(1650)).await;
        assert!(TcpStream::connect(address).await.is_ok());
        start_schedule(
            &proxy,
            vec![(
                50,
                Profile {
                    mode: Mode::Refuse,
                    ..Profile::default()
                },
            )],
            0,
        );
        let generation = proxy.generation.fetch_add(1, Ordering::SeqCst) + 1;
        apply(&proxy, Profile::default(), generation).await.unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(TcpStream::connect(address).await.is_ok());
        proxy.done.send(true).unwrap();
        proxy.changes.closed().await;
    }
}
