use std::sync::Arc;

use axum::http::HeaderMap;
use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;

/// 限流绕过 token 的环境变量名。
pub const BYPASS_ENV: &str = "RATE_LIMIT_BYPASS_TOKEN";
/// 携带绕过 token 的请求头名。
pub const BYPASS_HEADER: &str = "x-testkit-token";
/// 每秒令牌数的默认值（输入为 0 或无法解析时使用）。
const DEFAULT_PER_SECOND: u64 = 5;
/// 突发容量的默认值（输入为 0 或无法解析时使用）。
const DEFAULT_BURST: u32 = 10;
/// 一秒的毫秒数。
const MILLIS_PER_SECOND: u64 = 1000;

/// 把「每秒 n 个令牌」换算成 governor 的补充间隔（毫秒/个）。
/// 0 回退默认值；大于 1000 钳到 1ms（即上限 1000 个/秒）。
pub fn replenish_interval_ms(per_second: u64) -> u64 {
    let n = if per_second == 0 {
        DEFAULT_PER_SECOND
    } else {
        per_second
    };
    (MILLIS_PER_SECOND / n).max(1)
}

/// 读取环境变量并解析为正整数，缺失、非法或为 0 时返回默认值。
fn env_positive<T: std::str::FromStr + PartialOrd + From<u8>>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .filter(|v| *v > T::from(0u8))
        .unwrap_or(default)
}

/// 构建限流层：RATE_LIMIT_PER_SECOND 表示每秒 n 个请求，RATE_LIMIT_BURST 为突发容量。
pub fn build_layer() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware> {
    let per_second: u64 = env_positive("RATE_LIMIT_PER_SECOND", DEFAULT_PER_SECOND);
    let burst: u32 = env_positive("RATE_LIMIT_BURST", DEFAULT_BURST);

    let config = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(replenish_interval_ms(per_second))
            .burst_size(burst)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("valid governor config"),
    );

    GovernorLayer { config }
}

/// 限流绕过判定器；未配置 token（未设或为空）时永不绕过。
#[derive(Clone, Debug, Default)]
pub struct Bypass {
    /// 期望的 token。
    token: Option<Arc<str>>,
}

impl Bypass {
    /// 从环境变量 RATE_LIMIT_BYPASS_TOKEN 读取；未设或为空则关闭绕过。
    pub fn from_env() -> Self {
        Self::new(std::env::var(BYPASS_ENV).ok().as_deref())
    }

    /// 用给定 token 构造；`None` 或空串表示关闭绕过。
    pub fn new(token: Option<&str>) -> Self {
        Self {
            token: token.filter(|t| !t.is_empty()).map(Arc::from),
        }
    }

    /// 是否启用了绕过。
    pub fn enabled(&self) -> bool {
        self.token.is_some()
    }

    /// 请求头 x-testkit-token 与配置的 token 相等（常量时间比较）时返回 true。
    pub fn allows(&self, headers: &HeaderMap) -> bool {
        match (&self.token, headers.get(BYPASS_HEADER)) {
            (Some(expected), Some(got)) => constant_time_eq(expected.as_bytes(), got.as_bytes()),
            _ => false,
        }
    }
}

/// 常量时间字节比较：长度不同也遍历较长一方，避免通过耗时泄露前缀匹配长度。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= (x ^ y) as usize;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// 构造带 x-testkit-token 头的请求头集合。
    fn headers(token: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Some(t) = token {
            map.insert(BYPASS_HEADER, HeaderValue::from_str(t).unwrap());
        }
        map
    }

    /// 正确 token 绕过，错误、缺失、前缀相同但长度不同的都不绕过。
    #[test]
    fn bypass_only_on_exact_token() {
        let bypass = Bypass::new(Some("secret"));
        assert!(bypass.enabled());
        assert!(bypass.allows(&headers(Some("secret"))));
        assert!(!bypass.allows(&headers(Some("wrong!"))));
        assert!(!bypass.allows(&headers(Some("secre"))));
        assert!(!bypass.allows(&headers(Some("secret2"))));
        assert!(!bypass.allows(&headers(Some(""))));
        assert!(!bypass.allows(&headers(None)));
    }

    /// 未配置或配置为空时整体关闭，即使请求带了空 token 也不绕过。
    #[test]
    fn bypass_disabled_when_unset_or_empty() {
        for bypass in [Bypass::new(None), Bypass::new(Some("")), Bypass::default()] {
            assert!(!bypass.enabled());
            assert!(!bypass.allows(&headers(Some(""))));
            assert!(!bypass.allows(&headers(Some("secret"))));
            assert!(!bypass.allows(&headers(None)));
        }
    }

    /// 速率换算：n 个/秒对应 1000/n 毫秒，0 回退默认，超过 1000 钳到 1ms。
    #[test]
    fn interval_conversion() {
        assert_eq!(replenish_interval_ms(1), 1000);
        assert_eq!(replenish_interval_ms(5), 200);
        assert_eq!(replenish_interval_ms(10), 100);
        assert_eq!(replenish_interval_ms(1000), 1);
        assert_eq!(replenish_interval_ms(5000), 1);
        assert_eq!(replenish_interval_ms(u64::MAX), 1);
        assert_eq!(replenish_interval_ms(0), 200);
    }

    /// 常量时间比较的基本语义。
    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
    }
}
