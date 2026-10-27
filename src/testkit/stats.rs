//! 请求生命周期统计，取消时依靠析构记录。
use super::{
    response::Style,
    run_store::{Run, MAX_KEYS},
};
use axum::body::Body;
use futures_util::{stream, StreamExt};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};

/// 每轮最多保存的完成记录。
const MAX_LOG: usize = 1000;

/// 一次请求的观测记录。
#[derive(Serialize, Clone)]
pub(super) struct Entry {
    /// 到达序号。
    pub seq: u64,
    /// 请求方法。
    pub method: String,
    /// 请求路径。
    pub path: String,
    /// 命中分组键。
    pub key: String,
    /// 已鉴权用户名。
    pub auth: Option<String>,
    /// 响应风格。
    pub style: Style,
    /// 失败细分原因。
    pub reason: Option<&'static str>,
    /// HTTP 状态；无响应时为零。
    pub status: u16,
    /// 业务码。
    pub code: i64,
    /// 相对轮次起点的开始毫秒。
    pub start_ms: u64,
    /// 相对轮次起点的结束毫秒。
    pub end_ms: u64,
    /// 客户端在完成之前取消。
    pub aborted: bool,
}

/// 一轮测试的统计快照。
#[derive(Serialize, Clone)]
pub(super) struct Stats {
    /// 总命中数。
    pub total: u64,
    /// 按键计数。
    pub by_key: HashMap<String, u64>,
    /// 按路径计数。
    pub by_path: HashMap<String, u64>,
    /// 在途峰值。
    pub peak_inflight: u64,
    /// 当前在途数。
    pub inflight: u64,
    /// 最近完成记录。
    pub log: VecDeque<Entry>,
    /// 单调时钟起点。
    #[serde(skip)]
    origin: Instant,
}

impl Default for Stats {
    /// 新建空快照。
    fn default() -> Self {
        Self {
            total: 0,
            by_key: HashMap::new(),
            by_path: HashMap::new(),
            peak_inflight: 0,
            inflight: 0,
            log: VecDeque::new(),
            origin: Instant::now(),
        }
    }
}

/// 计数加一；表已达 `MAX_KEYS` 时不再收录新键，防止客户端控制的 key 让表无限增长。
fn bump(map: &mut HashMap<String, u64>, key: &str) {
    if let Some(n) = map.get_mut(key) {
        *n += 1;
    } else if map.len() < MAX_KEYS {
        map.insert(key.into(), 1);
    }
}

/// 从 handler future 延伸到响应体流的取消守卫。
pub(super) struct Guard {
    /// 所属轮次。
    run: Arc<Mutex<Run>>,
    /// 尚未落盘的记录。
    pub entry: Entry,
    /// 已到达响应体末尾，或主动注入服务端故障。
    pub complete: bool,
    /// Barrier 响应完成后再允许调度下一个请求。
    pub release_ack: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Guard {
    /// 开始一次请求并返回守卫及该键的命中序号。
    pub fn start(run: Arc<Mutex<Run>>, method: String, path: String, key: String) -> (Self, u64) {
        let mut r = run.lock().unwrap();
        let hit = r.hit(&key);
        let s = &mut r.stats;
        s.total += 1;
        s.inflight += 1;
        s.peak_inflight = s.peak_inflight.max(s.inflight);
        bump(&mut s.by_path, &path);
        bump(&mut s.by_key, &key);
        let entry = Entry {
            seq: s.total,
            method,
            path,
            key,
            auth: None,
            style: Style::default(),
            reason: None,
            status: 0,
            code: 0,
            start_ms: s.origin.elapsed().as_millis() as u64,
            end_ms: 0,
            aborted: false,
        };
        drop(r);
        (
            Self {
                run,
                entry,
                complete: false,
                release_ack: None,
            },
            hit,
        )
    }

    /// 包裹响应流，直至 EOS 或连接取消才结束计数。
    pub fn body(self, body: Body) -> Body {
        Body::from_stream(stream::unfold(
            (body.into_data_stream(), self),
            |(mut stream, mut guard)| async move {
                match stream.next().await {
                    Some(frame) => Some((frame, (stream, guard))),
                    None => {
                        guard.complete = true;
                        drop(guard);
                        None
                    }
                }
            },
        ))
    }
}

impl Drop for Guard {
    /// 即使 handler future 被取消，也写入结束记录。
    fn drop(&mut self) {
        let mut run = self.run.lock().unwrap();
        run.touched = Instant::now();
        let s = &mut run.stats;
        s.inflight = s.inflight.saturating_sub(1);
        self.entry.end_ms = s.origin.elapsed().as_millis() as u64;
        self.entry.aborted = !self.complete;
        if s.log.len() == MAX_LOG {
            s.log.pop_front();
        }
        // 记录只在析构时移动一次。
        let entry = Entry {
            seq: self.entry.seq,
            method: std::mem::take(&mut self.entry.method),
            path: std::mem::take(&mut self.entry.path),
            key: std::mem::take(&mut self.entry.key),
            auth: self.entry.auth.take(),
            style: self.entry.style,
            reason: self.entry.reason,
            status: self.entry.status,
            code: self.entry.code,
            start_ms: self.entry.start_ms,
            end_ms: self.entry.end_ms,
            aborted: self.entry.aborted,
        };
        s.log.push_back(entry);
        if let Some(ack) = self.release_ack.take() {
            let _ = ack.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 完成、取消、峰值与日志容量都以守卫生命周期为准。
    #[tokio::test]
    async fn lifecycle_and_log_bound() {
        let store = super::super::run_store::RunStore::new();
        let run = store.get("stats-unit");
        let (first, _) = Guard::start(run.clone(), "GET".into(), "/t/echo".into(), "k".into());
        let (mut second, _) = Guard::start(run.clone(), "GET".into(), "/t/echo".into(), "k".into());
        second.complete = true;
        drop(first);
        drop(second);
        {
            let r = run.lock().unwrap();
            assert_eq!(
                (r.stats.total, r.stats.inflight, r.stats.peak_inflight),
                (2, 0, 2)
            );
            assert!(r.stats.log[0].aborted);
            assert!(!r.stats.log[1].aborted);
        }
        for _ in 0..MAX_LOG {
            let (mut g, _) = Guard::start(run.clone(), "GET".into(), "/t/echo".into(), "k".into());
            g.complete = true;
        }
        let r = run.lock().unwrap();
        assert_eq!(r.stats.log.len(), MAX_LOG);
        assert_eq!(r.stats.log.front().unwrap().seq, 3);
    }
}
