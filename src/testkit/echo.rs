//! 有选择的请求回显。
use axum::extract::Query;
use axum::http::request::Parts;
use serde_json::{json, Value};

/// 构造请求字段；仅回显鉴权、内容类型和 x-* 头。
pub(super) fn request(parts: &Parts, body: &[u8]) -> Value {
    let query = Query::<std::collections::HashMap<String, String>>::try_from_uri(&parts.uri)
        .map(|q| q.0)
        .unwrap_or_default();
    let headers: serde_json::Map<String, Value> = parts
        .headers
        .iter()
        .filter(|(name, _)| {
            matches!(name.as_str(), "authorization" | "content-type")
                || name.as_str().starts_with("x-")
        })
        .map(|(k, v)| {
            (
                k.to_string(),
                Value::String(v.to_str().unwrap_or_default().into()),
            )
        })
        .collect();
    json!({"request":{"method":parts.method.as_str(), "path":parts.uri.path(), "query":query, "headers":headers, "body":{"raw":String::from_utf8_lossy(body),"json":serde_json::from_slice::<Value>(body).unwrap_or(Value::Null)}}})
}
