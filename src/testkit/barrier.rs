//! 基于单个调度器的有序 barrier，完成后复用组名。
use super::{
    control::{Control, MAX_DELAY},
    run_store::Run,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{oneshot, Notify};

/// 可复现的轻量伪随机数（无需额外 RNG crate）。
pub(super) fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// 同组必须一致的编排参数。
#[derive(PartialEq, Eq, Clone)]
struct Config {
    /// 请求数。
    n: usize,
    /// 完整放行顺序。
    seq: Vec<usize>,
    /// 放行间隔。
    gap: u64,
    /// 收集超时。
    wait: u64,
    /// 失败序号。
    fail: Vec<usize>,
}

/// 发送给 handler 的实际放行结果。
pub(super) struct Release {
    /// 实际名次，从零开始。
    pub rank: usize,
    /// 超时未齐。
    pub partial: bool,
    /// 此序号是否失败。
    pub failed: bool,
    /// 响应体完成或被取消后确认，再放行下一个请求。
    pub ack: oneshot::Sender<()>,
}

/// 一轮组的收集状态。
#[derive(Default)]
struct Pending {
    /// 到达请求及返回通道。
    arrivals: HashMap<usize, oneshot::Sender<Release>>,
    /// 是否开始放行。
    releasing: bool,
}

/// 一个 barrier 组实例。
pub(super) struct Group {
    /// 第一位到达者锁定参数。
    config: Config,
    /// 收集状态。
    pending: Mutex<Pending>,
    /// 全员到齐通知。
    ready: Notify,
    /// 本代完全释放后的通知。
    finished: Notify,
}

/// 解析逗号序号列表。
fn indices(value: &str) -> Result<Vec<usize>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|v| v.parse().map_err(|_| "invalid index list".into()))
        .collect()
}

impl Config {
    /// 校验人数、顺序、失败列表与等待上限。
    fn parse(c: &Control) -> Result<Self, String> {
        let n = c.number("n", 0usize)?;
        if !(1..=64).contains(&n) {
            return Err("n must be 1..64".into());
        }
        let mut seq: Vec<_> = (0..n).collect();
        match c.get("order").unwrap_or("asc") {
            "asc" => {}
            "desc" => seq.reverse(),
            "shuffle" => {
                let mut seed = c.number("seed", 1u64)?;
                for i in (1..n).rev() {
                    seq.swap(i, (random(&mut seed) % (i as u64 + 1)) as usize);
                }
            }
            "given" => {
                seq = indices(c.get("seq").ok_or("given requires seq")?)?;
                let mut sorted = seq.clone();
                sorted.sort_unstable();
                if sorted != (0..n).collect::<Vec<_>>() {
                    return Err("seq must be a permutation".into());
                }
            }
            _ => return Err("unknown order".into()),
        }
        let mut fail = match c.get("fail").unwrap_or("") {
            "all" => (0..n).collect(),
            v => indices(v)?,
        };
        if fail.iter().any(|&i| i >= n) {
            return Err("fail index out of range".into());
        }
        fail.sort_unstable();
        fail.dedup();
        Ok(Self {
            n,
            seq,
            gap: c.number("gap_ms", 30)?.min(MAX_DELAY),
            wait: c.number("wait_ms", 5000)?.min(MAX_DELAY),
            fail,
        })
    }
}

/// 加入 barrier，返回放行结果或参数/冲突说明。
pub(super) async fn arrive(
    run: Arc<Mutex<Run>>,
    name: String,
    c: &Control,
) -> Result<(usize, Release), String> {
    let config = Config::parse(c)?;
    let idx = c.number("idx", usize::MAX)?;
    if idx >= config.n {
        return Err("idx required and must be below n".into());
    }
    let rx = loop {
        let (tx, rx) = oneshot::channel();
        let (group, created) = {
            let mut r = run.lock().unwrap();
            let created = !r.groups.contains_key(&name);
            let group = r
                .groups
                .entry(name.clone())
                .or_insert_with(|| {
                    Arc::new(Group {
                        config: config.clone(),
                        pending: Mutex::new(Pending::default()),
                        ready: Notify::new(),
                        finished: Notify::new(),
                    })
                })
                .clone();
            (group, created)
        };
        let finished = group.finished.notified();
        tokio::pin!(finished);
        finished.as_mut().enable();
        let releasing = {
            let mut p = group.pending.lock().unwrap();
            if p.releasing {
                true
            } else {
                if group.config != config || p.arrivals.contains_key(&idx) {
                    return Err("barrier configuration conflict or duplicate idx".into());
                }
                p.arrivals.insert(idx, tx);
                if p.arrivals.len() == config.n {
                    group.ready.notify_one();
                }
                false
            }
        };
        if releasing {
            let still_current = run
                .lock()
                .unwrap()
                .groups
                .get(&name)
                .is_some_and(|g| Arc::ptr_eq(g, &group));
            if still_current {
                finished.await;
            }
            continue;
        }
        if created {
            let task_run = run.clone();
            let task_name = name.clone();
            let task_group = group.clone();
            tokio::spawn(async move {
                release_group(task_run, task_name, task_group).await;
            });
        }
        break rx;
    };
    Ok((idx, rx.await.map_err(|_| "barrier closed")?))
}

/// 等待到齐或超时，单线程依次发放并等待 handler 确认。
async fn release_group(run: Arc<Mutex<Run>>, name: String, group: Arc<Group>) {
    let _ = tokio::time::timeout(
        Duration::from_millis(group.config.wait),
        group.ready.notified(),
    )
    .await;
    let mut arrivals = {
        let mut p = group.pending.lock().unwrap();
        p.releasing = true;
        std::mem::take(&mut p.arrivals)
    };
    arrivals.retain(|_, tx| !tx.is_closed());
    let partial = arrivals.len() != group.config.n;
    let mut rank = 0;
    for idx in &group.config.seq {
        if let Some(tx) = arrivals.remove(idx) {
            if rank > 0 {
                tokio::time::sleep(Duration::from_millis(group.config.gap)).await;
            }
            let (ack, done) = oneshot::channel();
            if tx
                .send(Release {
                    rank,
                    partial,
                    failed: group.config.fail.contains(idx),
                    ack,
                })
                .is_ok()
            {
                let _ = done.await;
                rank += 1;
            }
        }
    }
    let mut r = run.lock().unwrap();
    if r.groups.get(&name).is_some_and(|g| Arc::ptr_eq(g, &group)) {
        r.groups.remove(&name);
    }
    group.finished.notify_waiters();
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 确定种子可复现，given 必须完整，asc/desc 精确排序。
    #[test]
    fn order_and_shuffle() {
        let parse = |q: &str| {
            Config::parse(
                &Control::parse(&format!("/t/barrier/g?n=4&{q}").parse().unwrap()).unwrap(),
            )
            .unwrap()
            .seq
        };
        assert_eq!(parse("order=asc"), [0, 1, 2, 3]);
        assert_eq!(parse("order=desc"), [3, 2, 1, 0]);
        assert_eq!(parse("order=given&seq=2,0,3,1"), [2, 0, 3, 1]);
        assert_eq!(parse("order=shuffle&seed=7"), parse("order=shuffle&seed=7"));
        assert_ne!(parse("order=shuffle&seed=7"), parse("order=shuffle&seed=8"));
    }

    /// 实际调度必须依次收到前一个响应确认，组名可立即复用。
    #[tokio::test]
    async fn releases_in_order_and_resets() {
        let store = super::super::run_store::RunStore::new();
        let run = store.get("barrier-unit");
        for _ in 0..2 {
            let (results, mut receiver) = tokio::sync::mpsc::unbounded_channel();
            let mut tasks = tokio::task::JoinSet::new();
            for idx in 0..3 {
                let run = run.clone();
                let results = results.clone();
                tasks.spawn(async move {
                    let c = Control::parse(
                        &format!("/t/barrier/g?n=3&idx={idx}&order=desc&gap_ms=0")
                            .parse()
                            .unwrap(),
                    )
                    .unwrap();
                    let (idx, release) = arrive(run, "g".into(), &c).await.unwrap();
                    results.send((idx, release.rank)).unwrap();
                    let _ = release.ack.send(());
                });
            }
            assert_eq!(receiver.recv().await, Some((2, 0)));
            assert_eq!(receiver.recv().await, Some((1, 1)));
            assert_eq!(receiver.recv().await, Some((0, 2)));
            while let Some(result) = tasks.join_next().await {
                result.unwrap();
            }
        }
    }
}
