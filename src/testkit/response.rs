//! 响应风格与统一失败信息。
use axum::http::{HeaderMap, Uri};
use serde::Serialize;

/// 默认成功消息。
pub(super) const OK: &str = "ok";
/// 注入失败原因。
pub(super) const INJECTED: &str = "injected";
/// 通用注入失败消息。
pub(super) const INJECTED_FAILURE: &str = "injected failure";
/// Barrier 失败消息。
pub(super) const BARRIER_FAILURE: &str = "barrier fail";
/// 越权原因和消息。
const FORBIDDEN_TEXT: &str = "forbidden";

/// 请求选择的响应风格。
#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Style {
    #[default]
    Http,
    Business,
}

impl Style {
    /// 查询参数优先于请求头。
    pub fn resolve(uri: &Uri, headers: &HeaderMap) -> Self {
        let query = axum::extract::Query::<Vec<(String, String)>>::try_from_uri(uri).ok();
        let value = query
            .as_ref()
            .and_then(|q| {
                q.iter()
                    .rev()
                    .find(|(k, _)| k == "style")
                    .map(|(_, v)| v.as_str())
            })
            .or_else(|| headers.get("x-test-style").and_then(|v| v.to_str().ok()));
        if value == Some("business") {
            Self::Business
        } else {
            Self::Http
        }
    }

    /// 转换逻辑状态为 HTTP 状态。
    pub fn status(self, status: u16) -> u16 {
        match self {
            Self::Http => status,
            Self::Business => 200,
        }
    }
}

/// 失败的逻辑状态、原因和消息。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    pub status: u16,
    pub reason: &'static str,
    pub message: &'static str,
}

impl Failure {
    /// 构造鉴权失败。
    pub const fn auth(reason: &'static str, message: &'static str) -> Self {
        Self {
            status: 401,
            reason,
            message,
        }
    }
}

/// 无令牌。
pub(super) const MISSING: Failure = Failure::auth("missing", "missing token");
/// 假令牌。
pub(super) const INVALID: Failure = Failure::auth("invalid", "invalid token");
/// 过期令牌。
pub(super) const EXPIRED: Failure = Failure::auth("expired", "token expired");
/// 已吊销令牌。
pub(super) const REVOKED: Failure = Failure::auth("revoked", "token revoked");
/// 登录失败。
pub(super) const BAD_CREDENTIALS: Failure =
    Failure::auth("bad_credentials", "invalid username or password");
/// Refresh 无效。
pub(super) const INVALID_REFRESH: Failure = Failure::auth(INVALID.reason, "invalid refresh token");
/// Refresh 过期。
pub(super) const EXPIRED_REFRESH: Failure = Failure::auth(EXPIRED.reason, "refresh token expired");
/// Refresh 已吊销。
pub(super) const REVOKED_REFRESH: Failure = Failure::auth(REVOKED.reason, REVOKED.message);
/// Refresh 重用。
pub(super) const REUSED: Failure = Failure::auth("reused", "refresh token reused");
/// 越权。
pub(super) const FORBIDDEN: Failure = Failure {
    status: 403,
    reason: FORBIDDEN_TEXT,
    message: FORBIDDEN_TEXT,
};
/// 请求参数错误。
pub(super) const BAD_REQUEST: Failure = Failure {
    status: 400,
    reason: INJECTED,
    message: "invalid request",
};
/// 未知鉴权动作。
pub(super) const NOT_FOUND: Failure = Failure {
    status: 404,
    reason: INJECTED,
    message: "not found",
};
