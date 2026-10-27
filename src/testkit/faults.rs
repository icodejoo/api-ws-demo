//! HTTP 故障体；流错误由 hyper 转为连接关闭。
use super::control::{Control, JSON_TYPE, RETRY_AFTER_PARAM};
use axum::{
    body::{Body, Bytes},
    http::header::{CONTENT_TYPE, RETRY_AFTER},
    response::Response,
};
use futures_util::stream;
use serde_json::{json, Value};
use std::{io, time::Duration};

/// 半响应发送后留出刷新套接字的时间。
const HALF_FLUSH: u64 = 20;
/// 故意中止流的错误。
fn broken() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, "injected testkit fault")
}

/// 根据控制参数生成响应；坏响应会有意绕过信封。
pub(super) fn response(
    c: &Control,
    status: u16,
    code: i64,
    data: Value,
    message: &str,
) -> Response {
    let fault = c.get("fault").unwrap_or("");
    let normal = serde_json::to_vec(&json!({"code":code,"data":data,"message":message})).unwrap();
    let mut content_type = match c.get("content_type").unwrap_or("json") {
        "text" => "text/plain",
        "html" => "text/html",
        "octet" => "application/octet-stream",
        _ => JSON_TYPE,
    };
    let body = match fault {
        "close" | "hang" => Body::from_stream(stream::once(async { Err::<Bytes, _>(broken()) })),
        "half_body" => {
            let half = Bytes::copy_from_slice(&normal[..normal.len() / 2]);
            Body::from_stream(stream::unfold(0, move |step| {
                let half = half.clone();
                async move {
                    match step {
                        0 => Some((Ok(half), 1)),
                        1 => {
                            tokio::time::sleep(Duration::from_millis(HALF_FLUSH)).await;
                            Some((Err(broken()), 2))
                        }
                        _ => None,
                    }
                }
            }))
        }
        "slow_body" => {
            let bytes = Bytes::from(normal);
            let gap = c.chunk_ms;
            let chunk_bytes = c.chunk_bytes;
            Body::from_stream(stream::unfold(
                (bytes, 0),
                move |(bytes, offset)| async move {
                    if offset == bytes.len() {
                        return None;
                    }
                    if offset > 0 {
                        tokio::time::sleep(Duration::from_millis(gap)).await;
                    }
                    let end = (offset + chunk_bytes).min(bytes.len());
                    Some((Ok::<_, io::Error>(bytes.slice(offset..end)), (bytes, end)))
                },
            ))
        }
        "bad_json" => {
            content_type = JSON_TYPE;
            Body::from("{invalid json")
        }
        "empty" => Body::empty(),
        "html" => {
            content_type = "text/html";
            Body::from("<!doctype html><html><body>gateway error</body></html>")
        }
        "no_envelope" => {
            let bare = match &c.payload {
                Some(value) if value.is_array() || value.is_object() => value.clone(),
                Some(value) => json!({"payload":value}),
                None => json!([1, 2, 3]),
            };
            Body::from(serde_json::to_vec(&bare).unwrap())
        }
        _ => Body::from(normal),
    };
    let status = if matches!(fault, "empty" | "html") {
        200
    } else {
        status
    };
    let mut response = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .body(body)
        .unwrap();
    response.headers_mut().extend(c.headers.clone());
    if status >= 400 || code != 0 || data.get("reason").is_some() {
        if let Some(value) = c.get(RETRY_AFTER_PARAM) {
            response
                .headers_mut()
                .insert(RETRY_AFTER, value.parse().unwrap());
        }
    }
    response
}
