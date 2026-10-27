//! 按契约提供可控的客户端黑盒验收接口。
mod auth;
mod barrier;
mod control;
mod echo;
mod faults;
mod response;
pub(crate) mod run_store;
mod stats;

use crate::state::AppState;
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use control::{Control, MAX_HANG, MAX_SIZE, RETRY_AFTER_PARAM};
use response::{Style, BARRIER_FAILURE, INJECTED, INJECTED_FAILURE, OK};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 受保护路径前缀。
const PRIVATE: &str = "/t/private/";
/// Mock 路径前缀。
const MOCK: &str = "/t/mock/";
/// 失败原因字段。
const REASON: &str = "reason";
/// 非法路径消息。
const INVALID_PATH: &str = "invalid path escape";

/// 毫秒级 Unix 时间戳。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 注册 /t 路由，适用于参数化路径与任意请求方法。
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/t/*rest", any(handle))
}

/// 无效请求的统一错误信封。
fn error(style: Style, status: u16, message: impl Into<String>, c: Option<&Control>) -> Response {
    let mut response = (
        StatusCode::from_u16(style.status(status)).unwrap(),
        Json(json!({"code":status,"data":{"reason":INJECTED},"message":message.into()})),
    )
        .into_response();
    if let Some(value) = c.and_then(|c| c.get(RETRY_AFTER_PARAM)) {
        response
            .headers_mut()
            .insert(axum::http::header::RETRY_AFTER, value.parse().unwrap());
    }
    response
}

/// 构造所有 `/t/*` 响应共用的 `meta` 字段（CONTRACT §2）。
fn meta_of(run: &str, key: &str, hit: u64, path: &str, method: &str) -> Value {
    json!({"run":run,"key":key,"hit":hit,"path":path,"method":method,"auth":null,"ts":now_ms()})
}

/// 把请求记为注入失败并返回对应风格的错误响应，响应体经守卫包裹以正确记录结束。
fn reject(
    mut guard: stats::Guard,
    style: Style,
    status: u16,
    message: impl Into<String>,
    c: &Control,
) -> Response {
    guard.entry.status = style.status(status);
    guard.entry.code = i64::from(status);
    guard.entry.reason = Some(INJECTED);
    let (parts, body) = error(style, status, message, Some(c)).into_parts();
    Response::from_parts(parts, guard.body(body))
}

/// 解码路径标识的百分号转义，保留字面加号；非法 UTF-8 返回错误。
fn path_id(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes.get(index + 1..index + 3).ok_or(INVALID_PATH)?;
            let hex = std::str::from_utf8(pair).map_err(|_| INVALID_PATH)?;
            decoded.push(u8::from_str_radix(hex, 16).map_err(|_| INVALID_PATH)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "invalid UTF-8 path".into())
}

/// 解析控制、跟踪整个 future 并执行具体业务。
async fn handle(State(state): State<AppState>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let style = Style::resolve(&parts.uri, &parts.headers);
    let c = match Control::parse(&parts.uri) {
        Ok(c) => c,
        Err(e) => return error(style, 400, e, None),
    };
    let path = parts.uri.path();
    let run_id = parts
        .headers
        .get("x-test-run")
        .and_then(|h| h.to_str().ok())
        .or_else(|| c.get("run"))
        .unwrap_or("default");
    let run = state.testkit.get(run_id);
    let key = c
        .get("key")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{} {path}", parts.method));
    // 观测与探针不计入业务统计，避免读取统计本身改变并发与总数。
    if let Some(target) = path.strip_prefix("/t/runs/") {
        let target = match path_id(target) {
            Ok(value) => value,
            Err(e) => return error(style, 400, e, Some(&c)),
        };
        let hit = run.lock().unwrap().hit(&key);
        if parts.method == "DELETE" {
            state.testkit.delete(&target);
        } else if parts.method != "GET" {
            return error(style, 405, "method not allowed", Some(&c));
        }
        let meta = meta_of(run_id, &key, hit, path, parts.method.as_str());
        let mut data = if parts.method == "DELETE" {
            json!({"deleted":true})
        } else {
            {
                // 锁内只拷贝快照，序列化放到锁外，读统计不阻塞该轮在途请求的记账
                let snapshot = state.testkit.get(&target).lock().unwrap().stats.clone();
                serde_json::to_value(&snapshot).unwrap()
            }
        };
        data["meta"] = meta;
        return Json(json!({"code":0,"data":data,"message":OK})).into_response();
    }
    if path == "/t/ping" && parts.method == "GET" {
        return Json(json!({"code":0})).into_response();
    }
    let (mut guard, hit) = stats::Guard::start(
        run.clone(),
        parts.method.to_string(),
        path.into(),
        key.clone(),
    );
    guard.entry.style = style;
    let bytes = match to_bytes(body, MAX_SIZE).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return reject(guard, style, 413, e.to_string(), &c);
        }
    };
    let (mut status, mut code) = c.outcome(hit);
    let mut injected = c.failed(hit);
    let mut failure = None;
    let mut message = c.get("message").unwrap_or(OK).to_owned();
    if injected {
        message = c.get("message").unwrap_or(INJECTED_FAILURE).into();
    }
    let mut meta = meta_of(run_id, &key, hit, path, parts.method.as_str());
    let mut data = json!({});
    let mut ack = None;
    // 所有通用延迟在副作用之前，失败刷新不会消耗 refresh token。
    tokio::time::sleep(Duration::from_millis(c.delay)).await;
    if c.get("fault") == Some("hang") {
        tokio::time::sleep(Duration::from_millis(MAX_HANG.saturating_sub(c.delay))).await;
    }
    if path.starts_with(PRIVATE) {
        let authorization = parts
            .headers
            .get("authorization")
            .map(|v| v.to_str().unwrap_or(""));
        match run.lock().unwrap().auth.validate(authorization) {
            Ok(username) => {
                guard.entry.auth = Some(username.clone());
                meta["auth"] = json!(username);
            }
            Err(e) => {
                failure = Some(e);
            }
        }
        if guard.entry.auth.is_some() && c.get("deny") == Some("1") {
            failure = Some(response::FORBIDDEN);
        }
    }
    if !injected && failure.is_none() {
        if let Some(action) = path.strip_prefix("/t/auth/") {
            if parts.method != "POST" {
                status = 405;
                code = 405;
            } else if let Ok(body) = serde_json::from_slice::<Value>(&bytes) {
                match run.lock().unwrap().auth.action(action, &body) {
                    Ok(value) => data = value,
                    Err(e) => failure = Some(e),
                }
            } else {
                status = 400;
                code = 400;
            }
        } else if let Some(group) = path.strip_prefix("/t/barrier/") {
            let group = match path_id(group) {
                Ok(value) => value,
                Err(e) => {
                    return reject(guard, style, 400, e, &c);
                }
            };
            match barrier::arrive(run.clone(), group, &c).await {
                Ok((idx, release)) => {
                    meta["partial"] = json!(release.partial);
                    data = json!({"idx":idx,"rank":release.rank});
                    if release.failed {
                        injected = true;
                        status = c.fail_status;
                        code = c.fail_code;
                        message = c.get("message").unwrap_or(BARRIER_FAILURE).into();
                    }
                    ack = Some(release.ack);
                }
                Err(e) => {
                    data = json!({"error":e});
                    status = 400;
                    code = 400;
                }
            }
        } else if path != "/t/echo"
            && !path.starts_with("/t/public/")
            && !path.starts_with(PRIVATE)
            && !path.starts_with(MOCK)
        {
            status = 404;
            code = 404;
        } else {
            data = echo::request(&parts, &bytes);
            if path.starts_with(MOCK) {
                data["mock"] = json!(true);
            }
        }
    }
    injected |= status >= 400 || code != 0;
    if let Some(e) = failure {
        status = e.status;
        code = i64::from(e.status);
        message = e.message.into();
        data = json!({"reason":e.reason});
    } else if injected {
        data[REASON] = json!(INJECTED);
    }
    data["meta"] = meta;
    data["payload"] = c.payload.clone().unwrap_or(Value::Null);
    data["pad"] = json!("x".repeat(c.size));
    guard.entry.reason = failure
        .map(|e| e.reason)
        .or_else(|| injected.then_some(INJECTED));
    let response = faults::response(&c, style.status(status), code, data, &message);
    let status = response.status().as_u16();
    guard.entry.status = status;
    guard.entry.code = code;
    // 服务端主动关闭不应被记作客户端取消。
    guard.complete = matches!(c.get("fault"), Some("close" | "half_body" | "hang"))
        || parts.method == "HEAD"
        || matches!(status, 204 | 304);
    guard.release_ack = ack;
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, guard.body(body))
}
