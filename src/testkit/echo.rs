//! 有选择的请求回显。
use axum::extract::Query;
use axum::http::request::Parts;
use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Request},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// 构造请求字段；仅回显鉴权、内容类型和 x-* 头。
pub(super) async fn request(parts: &Parts, body: &[u8]) -> Result<Value, String> {
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
    let mut data = json!({"request":{"method":parts.method.as_str(), "path":parts.uri.path(), "query":query, "headers":headers, "body":{"raw":String::from_utf8_lossy(body),"json":serde_json::from_slice::<Value>(body).unwrap_or(Value::Null)}}});
    let multipart = parts
        .headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            h.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("multipart/form-data")
        });
    if multipart {
        let mut request = Request::new(Body::from(body.to_vec()));
        *request.headers_mut() = parts.headers.clone();
        let mut multipart = Multipart::from_request(request, &())
            .await
            .map_err(|e| e.to_string())?;
        let mut fields = serde_json::Map::new();
        let mut files = Vec::new();
        while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
            let name = field
                .name()
                .ok_or("multipart part requires name")?
                .to_owned();
            let filename = field.file_name().map(str::to_owned);
            let bytes = field.bytes().await.map_err(|e| e.to_string())?;
            if let Some(filename) = filename {
                let digest: String = Sha256::digest(&bytes)[..8]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                files.push(
                    json!({"name":name,"filename":filename,"size":bytes.len(),"sha256":digest}),
                );
            } else {
                fields.insert(
                    name,
                    json!(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?),
                );
            }
        }
        data["multipart"] = json!({"fields":fields,"files":files});
    }
    Ok(data)
}
