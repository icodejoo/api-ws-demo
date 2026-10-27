//! 轮次隔离、空闲回收与共享状态。
use super::{auth::AuthStore, barrier::Group, stats::Stats};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// 无访问轮次的存活时间。
const IDLE_TTL: Duration = Duration::from_secs(600);
/// 回收扫描间隔。
const GC_INTERVAL: Duration = Duration::from_secs(30);
/// 同时存活的轮次上限；满了按最久未访问淘汰，防止线上被随意的 run id 撑爆内存。
const MAX_RUNS: usize = 64;
/// 每轮不同 key 的上限；超出后新 key 不再计数（hit 恒为 1、不进 by_key/by_path）。
pub(super) const MAX_KEYS: usize = 1024;

/// 一轮测试的全部可变状态。
pub(super) struct Run {
    /// 最近访问时间。
    pub(super) touched: Instant,
    /// 观测数据。
    pub(super) stats: Stats,
    /// 测试会话。
    pub(super) auth: AuthStore,
    /// 尚未结束的 barrier 组。
    pub(super) groups: HashMap<String, Arc<Group>>,
    /// 全部接口的命中序号，管理读取不写业务统计。
    pub(super) hits: HashMap<String, u64>,
}

impl Run {
    /// 增加指定键命中并返回从一开始的序号。
    pub(super) fn hit(&mut self, key: &str) -> u64 {
        if let Some(hit) = self.hits.get_mut(key) {
            *hit += 1;
            return *hit;
        }
        if self.hits.len() < MAX_KEYS {
            self.hits.insert(key.into(), 1);
        }
        1
    }
}

/// 由短临界区保护的轮次集合。
#[derive(Default)]
pub(crate) struct RunStore {
    /// 轮次索引。
    runs: Mutex<HashMap<String, Arc<Mutex<Run>>>>,
}

impl RunStore {
    /// 创建存储并启动弱引用 GC；返回共享句柄。
    pub fn new() -> Arc<Self> {
        let store = Arc::new(Self::default());
        let weak = Arc::downgrade(&store);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(GC_INTERVAL).await;
                let Some(store) = weak.upgrade() else { break };
                store.gc();
            }
        });
        store
    }

    /// 取得并触碰轮次；不存在时创建。
    pub(super) fn get(&self, id: &str) -> Arc<Mutex<Run>> {
        let mut runs = self.runs.lock().unwrap();
        let run = match runs.get(id) {
            Some(run) => run.clone(),
            None => {
                if runs.len() >= MAX_RUNS {
                    Self::evict_oldest(&mut runs);
                }
                let run = Arc::new(Mutex::new(Run {
                    touched: Instant::now(),
                    stats: Stats::default(),
                    auth: AuthStore::default(),
                    groups: HashMap::new(),
                    hits: HashMap::new(),
                }));
                runs.insert(id.into(), run.clone());
                run
            }
        };
        run.lock().unwrap().touched = Instant::now();
        run
    }

    /// 淘汰最久未访问的一轮；在途请求仍持有旧句柄，不受影响。
    fn evict_oldest(runs: &mut HashMap<String, Arc<Mutex<Run>>>) {
        let oldest = runs
            .iter()
            .min_by_key(|(_, run)| run.lock().unwrap().touched)
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            runs.remove(&id);
        }
    }

    /// 删除轮次；在途请求保留旧句柄，不污染新轮次。
    pub(super) fn delete(&self, id: &str) {
        self.runs.lock().unwrap().remove(id);
    }

    /// 回收十分钟无访问的轮次。
    fn gc(&self) {
        self.runs
            .lock()
            .unwrap()
            .retain(|_, run| run.lock().unwrap().touched.elapsed() < IDLE_TTL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 访问续期、删除与定期 GC 互不串轮次。
    #[tokio::test]
    async fn isolation_and_gc() {
        let s = RunStore::new();
        let old = s.get("old");
        old.lock().unwrap().touched = Instant::now() - IDLE_TTL;
        s.get("live");
        s.gc();
        assert!(!s.runs.lock().unwrap().contains_key("old"));
        s.delete("live");
        assert!(s.runs.lock().unwrap().is_empty());
    }

    /// 轮次数与每轮 key 数都有上限，超出时淘汰最旧轮次、新 key 不再计数。
    #[tokio::test]
    async fn bounded_runs_and_keys() {
        let s = RunStore::new();
        let first = s.get("r0");
        first.lock().unwrap().touched = Instant::now() - Duration::from_secs(1);
        for i in 1..=MAX_RUNS {
            s.get(&format!("r{i}"));
        }
        let runs = s.runs.lock().unwrap();
        assert_eq!(runs.len(), MAX_RUNS);
        assert!(!runs.contains_key("r0"));
        drop(runs);
        let mut run = first.lock().unwrap();
        for i in 0..MAX_KEYS {
            run.hit(&format!("k{i}"));
        }
        assert_eq!(run.hit("k0"), 2);
        assert_eq!(run.hit("overflow"), 1);
        assert_eq!(run.hit("overflow"), 1);
        assert_eq!(run.hits.len(), MAX_KEYS);
    }
}
