use std::collections::HashMap;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use rand::Rng;

use crate::auth::refresh::RefreshRegistry;
use crate::auth::user_store::User;
use crate::stomp::broker::Broker;

#[derive(Clone)]
/// 可低成本克隆的全站共享状态。
pub struct AppState {
    /// 按轮次隔离的验收状态。
    pub(crate) testkit: Arc<crate::testkit::run_store::RunStore>,
    /// 服务启动的单调时间。
    pub start_time: Instant,
    /// STOMP 共享消息代理。
    pub broker: Arc<Broker>,
    /// 线上鉴权用户表。
    pub users: Arc<RwLock<HashMap<String, User>>>,
    /// 线上 refresh token 注册表。
    pub refresh_registry: Arc<RefreshRegistry>,
    /// 线上 JWT 签名密钥。
    pub jwt_secret: Arc<[u8]>,
    /// 最近 CPU 百分比采样。
    pub cpu_usage: Arc<AtomicU8>,
}

impl AppState {
    /// 参数为共享 CPU 采样值，返回新状态；在 Tokio 运行时内调用。
    /// 例如 `AppState::new(cpu::spawn_sampler())`，同时启动测试轮次的 GC。
    pub fn new(cpu_usage: Arc<AtomicU8>) -> Self {
        let jwt_secret: Arc<[u8]> = match std::env::var("JWT_SECRET") {
            Ok(s) if !s.is_empty() => Arc::from(s.into_bytes()),
            _ => {
                let mut buf = [0u8; 32];
                rand::rng().fill_bytes(&mut buf);
                eprintln!(
                    "JWT_SECRET not set — generated an ephemeral secret; all sessions invalidate on restart"
                );
                Arc::from(buf.to_vec())
            }
        };

        Self {
            testkit: crate::testkit::run_store::RunStore::new(),
            start_time: Instant::now(),
            broker: Arc::new(Broker::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            refresh_registry: Arc::new(RefreshRegistry::new()),
            jwt_secret,
            cpu_usage,
        }
    }
}
