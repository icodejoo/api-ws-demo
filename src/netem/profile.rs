//! 网络配置、内置预设和确定性伪随机数。
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 每个转发分块的最大字节数。
pub(super) const CHUNK: usize = 4096;
/// flap 首段离线毫秒数。
pub(super) const FLAP_MS: u64 = 1500;
/// 单段模拟时长的校验上限。
pub(super) const MAX_TIME_MS: u64 = 86_400_000;
/// 控制 API 暴露的预设名称。
pub(super) const PRESETS: [&str; 7] = [
    "normal",
    "weak",
    "lossy",
    "offline",
    "blackhole",
    "flap",
    "dns",
];

/// TCP 代理工作模式。
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum Mode {
    /// 正常转发。
    #[default]
    Pass,
    /// 停止监听并断开已有连接。
    Refuse,
    /// 接收但不读写。
    Blackhole,
    /// 以 RST 立即关闭。
    Reset,
}

/// 每次切换完整替换的网络配置。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Profile {
    /// 工作模式。
    pub mode: Mode,
    /// 每方向每分块基础延迟。
    pub latency_ms: u64,
    /// 均匀正负抖动范围。
    pub jitter_ms: u64,
    /// 下行千比特每秒，零不限速。
    pub down_kbps: u64,
    /// 上行千比特每秒，零不限速。
    pub up_kbps: u64,
    /// 每方向第一个分块额外延迟。
    pub first_byte_ms: u64,
    /// 每连接中途重置概率。
    pub reset_prob: f64,
    /// 下行达到该累计字节数时重置。
    pub reset_after_bytes: Option<u64>,
    /// 每分块随机卡顿概率。
    pub stall_prob: f64,
    /// 卡顿毫秒数。
    pub stall_ms: u64,
    /// 伪随机种子。
    #[serde(default = "default_seed")]
    pub seed: u64,
}

/// 默认确定性种子。
fn default_seed() -> u64 {
    1
}

impl Default for Profile {
    /// 正常转发，使用契约默认种子。
    fn default() -> Self {
        Self {
            mode: Mode::Pass,
            latency_ms: 0,
            jitter_ms: 0,
            down_kbps: 0,
            up_kbps: 0,
            first_byte_ms: 0,
            reset_prob: 0.0,
            reset_after_bytes: None,
            stall_prob: 0.0,
            stall_ms: 0,
            seed: default_seed(),
        }
    }
}

impl Profile {
    /// 解析字符串预设或对象；返回配置及是否启用 flap 时间表。
    pub fn parse(value: &Value) -> Result<(Self, bool), String> {
        if let Some(name) = value.as_str() {
            let mut p = Self {
                seed: default_seed(),
                ..Self::default()
            };
            match name {
                "normal" => {}
                "weak" | "lossy" => {
                    p.latency_ms = 300;
                    p.jitter_ms = 200;
                    p.down_kbps = 64;
                    p.up_kbps = 32;
                    p.first_byte_ms = 200;
                    if name == "lossy" {
                        p.reset_prob = 0.15;
                        p.stall_prob = 0.1;
                        p.stall_ms = 800;
                    }
                }
                "offline" | "flap" => p.mode = Mode::Refuse,
                "blackhole" => p.mode = Mode::Blackhole,
                "dns" => return Err("dns is executor-only, not a TCP proxy profile".into()),
                _ => return Err("unknown preset".into()),
            }
            return Ok((p, name == "flap"));
        }
        let p: Self = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if !(0.0..=1.0).contains(&p.reset_prob) || !(0.0..=1.0).contains(&p.stall_prob) {
            return Err("probability must be 0..1".into());
        }
        // 防止恶意时长加法溢出，仍允许数小时的离线模拟。
        if [p.latency_ms, p.jitter_ms, p.first_byte_ms, p.stall_ms]
            .into_iter()
            .any(|v| v > MAX_TIME_MS)
        {
            return Err("delay exceeds one day".into());
        }
        Ok((p, false))
    }
}

/// 返回可直接使用的预设定义，dns 明确标为执行器动作。
pub(super) fn presets() -> Value {
    let mut result = serde_json::Map::new();
    for name in PRESETS {
        let value = match name {
            "flap" => json!([{"at_ms":0,"profile":"offline"},{"at_ms":FLAP_MS,"profile":"normal"}]),
            "dns" => json!({"executor_host":"nonexistent.invalid"}),
            _ => serde_json::to_value(Profile::parse(&json!(name)).unwrap().0).unwrap(),
        };
        result.insert(name.into(), value);
    }
    Value::Object(result)
}

/// SplitMix64，固定输入序列得到固定结果。
pub(super) fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// 从固定状态产生 [0,1) 的随机分数。
pub(super) fn chance(state: &mut u64) -> f64 {
    (random(state) >> 11) as f64 / ((1u64 << 53) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 预设定义与契约数值一致，并拒绝拼写错误。
    #[test]
    fn parses_presets() {
        let weak = Profile::parse(&json!("weak")).unwrap().0;
        assert_eq!(
            (
                weak.latency_ms,
                weak.jitter_ms,
                weak.down_kbps,
                weak.up_kbps,
                weak.first_byte_ms
            ),
            (300, 200, 64, 32, 200)
        );
        assert_eq!(Profile::parse(&json!("lossy")).unwrap().0.reset_prob, 0.15);
        assert!(Profile::parse(&json!("flap")).unwrap().1);
        assert_eq!(presets().as_object().unwrap().len(), PRESETS.len());
        assert!(Profile::parse(&json!({"reset_prob":1.1})).is_err());
        assert!(Profile::parse(&json!({"typo":0})).is_err());
        let (mut a, mut b) = (1, 1);
        assert_eq!(
            (0..20).map(|_| random(&mut a)).collect::<Vec<_>>(),
            (0..20).map(|_| random(&mut b)).collect::<Vec<_>>()
        );
    }
}
