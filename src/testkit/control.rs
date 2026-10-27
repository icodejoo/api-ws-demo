//! 通用查询参数解析与有界控制。
use axum::{
    extract::Query,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};

/// 最大等待毫秒数。
pub(super) const MAX_DELAY: u64 = 10_000;
/// 最大填充与请求体字节数。
pub(super) const MAX_SIZE: usize = 1024 * 1024;
/// 最长故障挂起毫秒数。
pub(super) const MAX_HANG: u64 = 60_000;
/// JSON 媒体类型。
pub(super) const JSON_TYPE: &str = "application/json";
/// 重试间隔参数。
pub(super) const RETRY_AFTER_PARAM: &str = "retry_after";

/// 已校验的通用控制参数。
#[derive(Default, Debug)]
pub(super) struct Control {
    /// 所有查询参数（重复 header 独立保存）。
    pub values: HashMap<String, String>,
    /// 自定义响应头。
    pub headers: HeaderMap,
    /// 延迟毫秒数。
    pub delay: u64,
    /// 正常 HTTP 状态。
    pub status: u16,
    /// 正常业务码。
    pub code: i64,
    /// 前若干次失败。
    pub fail_times: u64,
    /// 周期失败间隔。
    pub fail_every: u64,
    /// 精确指定失败的命中序号。
    pub fail_on: Vec<u64>,
    /// 失败 HTTP 状态。
    pub fail_status: u16,
    /// 失败业务码。
    pub fail_code: i64,
    /// 填充字节数。
    pub size: usize,
    /// 附加业务载荷。
    pub payload: Option<Value>,
    /// 慢响应分块间隔。
    pub chunk_ms: u64,
    /// 慢响应的分块字节数。
    pub chunk_bytes: usize,
}

impl Control {
    /// 从 URI 解析参数，非法值返回说明。
    pub fn parse(uri: &Uri) -> Result<Self, String> {
        let Query(pairs) =
            Query::<Vec<(String, String)>>::try_from_uri(uri).map_err(|e| e.to_string())?;
        let mut c = Self::default();
        for (key, value) in pairs {
            if key == "header" {
                let (name, value) = value.split_once(':').ok_or("header requires k:v")?;
                let name = HeaderName::from_str(name.trim()).map_err(|e| e.to_string())?;
                // 帧边界只能由服务器控制，避免头字段破坏 HTTP 协议。
                if matches!(
                    name.as_str(),
                    "content-length" | "transfer-encoding" | "connection"
                ) {
                    return Err("framing headers are reserved".into());
                }
                c.headers.append(
                    name,
                    HeaderValue::from_str(value.trim()).map_err(|e| e.to_string())?,
                );
            } else {
                c.values.insert(key, value);
            }
        }
        c.delay = c.number("delay_ms", 0)?.min(MAX_DELAY);
        c.status = c.http_status("status", 200)?;
        c.code = c.number(
            "code",
            if c.status == 200 {
                0
            } else {
                i64::from(c.status)
            },
        )?;
        c.fail_times = c.number("fail_times", 0)?;
        c.fail_every = c.number("fail_every", 0)?;
        c.fail_on = c
            .get("fail_on")
            .filter(|v| !v.is_empty())
            .map(|value| {
                value
                    .split(',')
                    .map(|v| {
                        v.parse::<u64>()
                            .ok()
                            .filter(|&v| v > 0)
                            .ok_or_else(|| "invalid fail_on".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        c.fail_status = c.http_status("fail_status", 500)?;
        c.fail_code = c.number("fail_code", i64::from(c.fail_status))?;
        c.size = c.number("size", 0usize)?.min(MAX_SIZE);
        c.chunk_ms = c.number("chunk_ms", 50)?.min(MAX_DELAY);
        c.chunk_bytes = c.number("chunk_bytes", 1024usize)?.clamp(1, MAX_SIZE);
        c.payload = c
            .get("data")
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| e.to_string())?;
        if !matches!(
            c.get("fault").unwrap_or(""),
            "" | "hang"
                | "close"
                | "half_body"
                | "slow_body"
                | "bad_json"
                | "empty"
                | "html"
                | "no_envelope"
        ) {
            return Err("unknown fault".into());
        }
        if !matches!(
            c.get("content_type").unwrap_or("json"),
            "json" | "text" | "html" | "octet"
        ) {
            return Err("unknown content_type".into());
        }
        if let Some(value) = c.get(RETRY_AFTER_PARAM) {
            let _: u64 = value.parse().map_err(|_| "invalid retry_after")?;
        }
        Ok(c)
    }

    /// 读取单个字符串参数。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// 读取带默认值的数值参数。
    pub fn number<T: FromStr>(&self, key: &str, default: T) -> Result<T, String> {
        self.get(key)
            .map(|v| v.parse().map_err(|_| format!("invalid {key}")))
            .unwrap_or(Ok(default))
    }

    /// 校验可用的最终响应状态（不接受临时响应）。
    fn http_status(&self, key: &str, default: u16) -> Result<u16, String> {
        let value = self.number(key, default)?;
        StatusCode::from_u16(value).map_err(|e| e.to_string())?;
        if !(200..=599).contains(&value) {
            return Err(format!("invalid {key}"));
        }
        Ok(value)
    }

    /// 按命中序号计算 flaky 响应。
    pub fn outcome(&self, hit: u64) -> (u16, i64) {
        if self.failed(hit) {
            (self.fail_status, self.fail_code)
        } else {
            (self.status, self.code)
        }
    }

    /// 当前命中是否属于通用失败注入。
    pub fn failed(&self, hit: u64) -> bool {
        hit <= self.fail_times
            || (self.fail_every > 0 && hit.is_multiple_of(self.fail_every))
            || self.fail_on.contains(&hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 重复头、编码、上限与 flaky 计算。
    #[test]
    fn parses_and_counts() {
        let c = Control::parse(&"/t/echo?delay_ms=99999&size=9999999&header=x-a:1&header=x-a:2&data=%7B%22a%22:1%7D&fail_times=2&fail_every=3".parse().unwrap()).unwrap();
        assert_eq!(c.delay, MAX_DELAY);
        assert_eq!(c.size, MAX_SIZE);
        assert_eq!(c.headers.get_all("x-a").iter().count(), 2);
        assert_eq!(c.payload, Some(serde_json::json!({"a":1})));
        assert_eq!(
            (1..=6).map(|i| c.outcome(i).0).collect::<Vec<_>>(),
            [500, 500, 500, 200, 200, 500]
        );
        let c = Control::parse(&"/t/echo?fail_on=1,3&chunk_bytes=512".parse().unwrap()).unwrap();
        assert_eq!(
            (1..=4).map(|i| c.outcome(i).0).collect::<Vec<_>>(),
            [500, 200, 500, 200]
        );
        assert_eq!(c.chunk_bytes, 512);
        for query in [
            "status=999",
            "delay_ms=-1",
            "fault=unknown",
            "data=broken",
            "header=content-length:4",
        ] {
            assert!(Control::parse(&format!("/t/echo?{query}").parse().unwrap()).is_err());
        }
    }
}
