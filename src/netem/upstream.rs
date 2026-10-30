//! 代理上游地址：明文 `IP:port`，或 `https://host[:port]` 的 TLS 上游。
use std::net::SocketAddr;

/// https 上游的默认端口。
const HTTPS_PORT: u16 = 443;
/// https 上游前缀。
const HTTPS_PREFIX: &str = "https://";

/// 一个已校验的上游描述。
#[derive(Clone, Debug)]
pub(super) struct Upstream {
    /// 实际 TCP 连接的 `host:port`。
    pub(super) connect: String,
    /// 需要 TLS 时的 SNI 与证书校验域名。
    pub(super) sni: Option<String>,
    /// 需要改写请求 Host 头时的目标值（`host` 或 `host:port`）。
    pub(super) host: Option<String>,
}

impl Upstream {
    /// 解析控制 API 的 upstream 字段；`https://host[:port]` 走 TLS，其余必须是数字套接字地址。
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let Some(rest) = text.strip_prefix(HTTPS_PREFIX) else {
            if text.parse::<SocketAddr>().is_err() {
                return Err(
                    "upstream must be a numeric socket address or https://host[:port]".into(),
                );
            }
            return Ok(Self::from(text));
        };
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() || rest.contains(['/', '@', '[', ']', '?', '#', ' ']) {
            return Err("https upstream must be https://host[:port]".into());
        }
        let (name, port) = match rest.rsplit_once(':') {
            Some((name, port)) => (
                name,
                port.parse::<u16>().map_err(|_| "invalid upstream port")?,
            ),
            None => (rest, HTTPS_PORT),
        };
        if name.is_empty() {
            return Err("https upstream host is empty".into());
        }
        super::tls::validate(name)?;
        let host = if port == HTTPS_PORT {
            name.to_string()
        } else {
            format!("{name}:{port}")
        };
        Ok(Self {
            connect: format!("{name}:{port}"),
            sni: Some(name.to_string()),
            host: Some(host),
        })
    }
}

impl From<&str> for Upstream {
    /// 明文上游，不改写 Host。
    fn from(value: &str) -> Self {
        Self {
            connect: value.to_string(),
            sni: None,
            host: None,
        }
    }
}

impl From<String> for Upstream {
    /// 明文上游，不改写 Host。
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 明文地址保持原样，https 写法得到 SNI 与 Host。
    #[test]
    fn parse_forms() {
        let plain = Upstream::parse("127.0.0.1:47080").unwrap();
        assert!(plain.sni.is_none() && plain.host.is_none());
        let tls = Upstream::parse("https://api.example.com").unwrap();
        assert_eq!(tls.connect, "api.example.com:443");
        assert_eq!(tls.sni.as_deref(), Some("api.example.com"));
        assert_eq!(tls.host.as_deref(), Some("api.example.com"));
        let custom = Upstream::parse("https://api.example.com:8443/").unwrap();
        assert_eq!(custom.connect, "api.example.com:8443");
        assert_eq!(custom.host.as_deref(), Some("api.example.com:8443"));
    }

    /// 非法写法被拒绝。
    #[test]
    fn parse_rejects() {
        for bad in [
            "localhost:80",
            "https://",
            "https://a/b",
            "https://a:x",
            "https://:443",
            "http://a.com",
        ] {
            assert!(Upstream::parse(bad).is_err(), "{bad}");
        }
    }
}
