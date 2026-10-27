//! 按方向维护的字节令牌桶。
use super::profile::CHUNK;
use std::time::{Duration, Instant};

/// 容量一分块、无初始突发额度的令牌桶。
pub(super) struct Bucket {
    /// 最近一次累计时刻。
    at: Instant,
    /// 当前可用额度，可负以预留后续发送。
    tokens: f64,
    /// 当前千比特速率。
    kbps: u64,
}

impl Bucket {
    /// 创建空桶。
    pub fn new() -> Self {
        Self {
            at: Instant::now(),
            tokens: 0.0,
            kbps: 0,
        }
    }

    /// 根据给定单调时间预留字节，返回至少需要等待的时长。
    pub fn reserve(&mut self, bytes: usize, kbps: u64, now: Instant) -> Duration {
        if self.kbps != kbps {
            self.tokens = 0.0;
            self.at = now;
            self.kbps = kbps;
        }
        if kbps == 0 {
            return Duration::ZERO;
        }
        let rate = kbps as f64 * 1000.0 / 8.0;
        self.tokens = (self.tokens + now.saturating_duration_since(self.at).as_secs_f64() * rate)
            .min(CHUNK as f64);
        self.at = now;
        self.tokens -= bytes as f64;
        Duration::from_secs_f64((-self.tokens / rate).max(0.0))
    }

    /// 配置中断发送等待时撤回本次预留。
    pub fn refund(&mut self, bytes: usize) {
        self.tokens += bytes as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 64kbps = 8000B/s，累计、突发上限和配置切换均可预测。
    #[test]
    fn token_math() {
        let mut b = Bucket::new();
        let at = Instant::now();
        assert_eq!(b.reserve(4000, 64, at), Duration::from_millis(500));
        assert_eq!(
            b.reserve(4000, 64, at + Duration::from_millis(500)),
            Duration::from_millis(500)
        );
        assert_eq!(
            b.reserve(4096, 64, at + Duration::from_secs(10)),
            Duration::ZERO
        );
        assert_eq!(b.reserve(4096, 0, at), Duration::ZERO);
        assert_eq!(b.reserve(4000, 32, at), Duration::from_secs(1));
    }
}
