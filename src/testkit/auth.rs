//! 测试专用毫秒会话与令牌轮换。
use super::{now_ms, response::*};
use crate::auth::jwt::{self, Claims};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// 默认 access 有效期。
const ACCESS_TTL: u64 = 60_000;
/// 默认 refresh 有效期。
const REFRESH_TTL: u64 = 600_000;
/// Bearer 类型。
pub(super) const BEARER: &str = "Bearer";
/// Refresh 字段。
const REFRESH_TOKEN: &str = "refresh_token";
/// 用户名字段。
const USERNAME: &str = "username";
/// 全量吊销范围。
const SCOPE_ALL: &str = "all";
/// 登录动作。
const LOGIN: &str = "login";
/// 刷新动作。
const REFRESH: &str = "refresh";
/// 吊销动作。
const REVOKE: &str = "revoke";
/// Access 字段。
const ACCESS_TOKEN: &str = "access_token";

/// 令牌的服务端记录。
struct Token {
    /// 用户名。
    username: String,
    /// 会话族。
    family: Uuid,
    /// 毫秒到期时刻。
    expires: u64,
    /// Access 有效期。
    access_ttl: u64,
    /// Refresh 有效期。
    refresh_ttl: u64,
    /// 是否严格轮换。
    strict_rotation: bool,
    /// Refresh 是否已使用。
    used: bool,
    /// 是否吊销。
    revoked: bool,
}

/// 每轮独立的令牌存储。
#[derive(Default)]
pub(super) struct AuthStore {
    /// Access 索引。
    access: HashMap<String, Token>,
    /// Refresh 索引，保留轮换历史。
    refresh: HashMap<String, Token>,
    /// 已吊销的会话族。
    revoked: HashSet<Uuid>,
    /// 每轮独立签名密钥。
    secret: Uuid,
}

impl AuthStore {
    /// 签发令牌，保留会话族和选项。
    fn issue(
        &mut self,
        username: String,
        family: Uuid,
        access_ttl: u64,
        refresh_ttl: u64,
        strict_rotation: bool,
    ) -> Value {
        if self.secret.is_nil() {
            self.secret = Uuid::new_v4();
        }
        let now = now_ms();
        let access = jwt::encode(
            &Claims {
                sub: format!("{username}:{}", Uuid::new_v4()),
                iat: now / 1000,
                exp: now.saturating_add(access_ttl).div_ceil(1000),
            },
            self.secret.as_bytes(),
        );
        let refresh = Uuid::new_v4().to_string();
        let mut data =
            json!({(ACCESS_TOKEN):access,"token_type":BEARER,"expires_in_ms":access_ttl});
        for (map, key, ttl) in [
            (&mut self.access, &access, access_ttl),
            (&mut self.refresh, &refresh, refresh_ttl),
        ] {
            map.insert(
                key.clone(),
                Token {
                    username: username.clone(),
                    family,
                    expires: now.saturating_add(ttl),
                    access_ttl,
                    refresh_ttl,
                    strict_rotation,
                    used: false,
                    revoked: false,
                },
            );
        }
        if username.starts_with("no_refresh_") {
            self.refresh.remove(&refresh);
        } else {
            data[REFRESH_TOKEN] = json!(refresh);
        }
        data
    }

    /// 区分缺失、伪造、过期和吊销。
    pub fn validate(&self, authorization: Option<&str>) -> Result<String, Failure> {
        let header = authorization.ok_or(MISSING)?;
        if header.trim() == BEARER {
            return Err(MISSING);
        }
        let token = header.strip_prefix("Bearer ").ok_or(INVALID)?.trim();
        if token.is_empty() {
            return Err(MISSING);
        }
        let t = self.access.get(token).ok_or(INVALID)?;
        if t.revoked || self.revoked.contains(&t.family) {
            return Err(REVOKED);
        }
        if now_ms() >= t.expires {
            return Err(EXPIRED);
        }
        jwt::decode(token, self.secret.as_bytes()).map_err(|_| INVALID)?;
        Ok(t.username.clone())
    }

    /// 执行鉴权动作。
    pub fn action(&mut self, action: &str, body: &Value) -> Result<Value, Failure> {
        let string = |key| {
            body.get(key)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .ok_or(BAD_REQUEST)
        };
        match action {
            LOGIN => {
                let user = string(USERNAME)?;
                if body.get("password").and_then(Value::as_str) == Some("wrong") {
                    return Err(BAD_CREDENTIALS);
                }
                let ttl = |key, default| {
                    body.get(key)
                        .map(|v| v.as_u64().ok_or(BAD_REQUEST))
                        .unwrap_or(Ok(default))
                };
                let strict = body
                    .get("strict_rotation")
                    .map(|v| v.as_bool().ok_or(BAD_REQUEST))
                    .unwrap_or(Ok(false))?;
                Ok(self.issue(
                    user.into(),
                    Uuid::new_v4(),
                    ttl("access_ttl_ms", ACCESS_TTL)?,
                    ttl("refresh_ttl_ms", REFRESH_TTL)?,
                    strict,
                ))
            }
            REFRESH => {
                let key = body
                    .get(REFRESH_TOKEN)
                    .and_then(Value::as_str)
                    .ok_or(INVALID_REFRESH)?;
                let t = self.refresh.get_mut(key).ok_or(INVALID_REFRESH)?;
                if t.used {
                    self.revoked.insert(t.family);
                    return Err(REUSED);
                }
                if t.revoked || self.revoked.contains(&t.family) {
                    return Err(REVOKED_REFRESH);
                }
                if now_ms() >= t.expires {
                    return Err(EXPIRED_REFRESH);
                }
                t.used = true;
                let (user, family, access, refresh, strict) = (
                    t.username.clone(),
                    t.family,
                    t.access_ttl,
                    t.refresh_ttl,
                    t.strict_rotation,
                );
                if strict {
                    for token in self.access.values_mut().filter(|t| t.family == family) {
                        token.revoked = true;
                    }
                }
                Ok(self.issue(user, family, access, refresh, strict))
            }
            "logout" => {
                if let Some(t) = self.refresh.get(string(REFRESH_TOKEN)?) {
                    self.revoked.insert(t.family);
                }
                Ok(json!({"logged_out":true}))
            }
            REVOKE => {
                let user = string(USERNAME)?;
                let scope = body
                    .get("scope")
                    .map(|v| v.as_str().ok_or(BAD_REQUEST))
                    .transpose()?
                    .unwrap_or(SCOPE_ALL);
                let all = match scope {
                    SCOPE_ALL => true,
                    "access" => false,
                    _ => return Err(BAD_REQUEST),
                };
                let mut count = 0;
                for t in self
                    .access
                    .values_mut()
                    .chain(self.refresh.values_mut().filter(|_| all))
                    .filter(|t| t.username == user)
                {
                    if !t.revoked
                        && !t.used
                        && !self.revoked.contains(&t.family)
                        && now_ms() < t.expires
                    {
                        count += 1;
                    }
                    t.revoked = true;
                }
                Ok(json!({"revoked":count}))
            }
            _ => Err(NOT_FOUND),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 测试用户。
    const USER: &str = "u";

    /// 毫秒过期、轮换重用、同族吊销和用户撤销。
    #[tokio::test]
    async fn expiry_and_reuse() {
        let mut store = AuthStore::default();
        let first = store
            .action(LOGIN, &json!({(USERNAME):USER,"access_ttl_ms":1}))
            .unwrap();
        let authorization =
            |data: &Value| format!("{BEARER} {}", data[ACCESS_TOKEN].as_str().unwrap());
        tokio::time::sleep(std::time::Duration::from_millis(4)).await;
        assert_eq!(store.validate(Some(&authorization(&first))), Err(EXPIRED));
        let next = store.action(REFRESH, &first).unwrap();
        assert_eq!(store.action(REFRESH, &first), Err(REUSED));
        assert_eq!(store.validate(Some(&authorization(&next))), Err(REVOKED));
        assert_eq!(store.action(REFRESH, &next), Err(REVOKED_REFRESH));
        let other = store.action(LOGIN, &json!({(USERNAME):USER})).unwrap();
        store.action(REVOKE, &json!({(USERNAME):USER})).unwrap();
        assert_eq!(store.validate(Some(&authorization(&other))), Err(REVOKED));
    }
}
