//! /t 接口的真实 HTTP 验收。
mod common;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use common::{decode, json, raw, send, start, Server, RUN_HEADER};
use serde_json::{json, Value};
use std::{
    sync::{atomic::AtomicU8, Arc},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// 创建启用 TEST_MODE 的完整主服务器。
async fn server() -> Server {
    std::env::set_var("TEST_MODE", "1");
    start(api_ws_demo::routes::build_router(
        api_ws_demo::state::AppState::new(Arc::new(AtomicU8::new(100))),
    ))
    .await
}

/// 测试用户名。
const USER: &str = "alice";
/// 受保护的测试路径。
const PRIVATE: &str = "/t/private/resource";
/// 登录路径。
const LOGIN: &str = "/t/auth/login";
/// 刷新路径。
const REFRESH: &str = "/t/auth/refresh";
/// 统计路径。
const STATS: &str = "/t/runs/integration";
/// 吊销路径。
const REVOKE: &str = "/t/auth/revoke";
/// 登出路径。
const LOGOUT: &str = "/t/auth/logout";
/// 回显路径。
const ECHO: &str = "/t/echo";
/// 信封字段。
const CODE: &str = "code";
/// 数据字段。
const DATA: &str = "data";
/// 原因字段。
const REASON: &str = "reason";
/// 消息字段。
const MESSAGE: &str = "message";
/// Access 字段。
const ACCESS_TOKEN: &str = "access_token";
/// Refresh 字段。
const REFRESH_TOKEN: &str = "refresh_token";
/// 标准风格。
const HTTP: &str = "http";
/// 业务风格。
const BUSINESS: &str = "business";
/// 风格与未登录状态。
const STYLES: [(&str, u16); 2] = [(HTTP, 401), (BUSINESS, 200)];
/// 假令牌原因。
const INVALID: &str = "invalid";
/// 过期原因。
const EXPIRED: &str = "expired";
/// 吊销原因。
const REVOKED: &str = "revoked";
/// 注入原因。
const INJECTED: &str = "injected";
/// 空请求体。
const EMPTY: Value = Value::Null;
/// 空文本。
const EMPTY_TEXT: &str = "";
/// 请求方法。
const GET: &str = "GET";
/// 写入方法。
const POST: &str = "POST";
/// 删除方法。
const DELETE: &str = "DELETE";
/// 用户名字段。
const USERNAME: &str = "username";
/// 未登录原因。
const MISSING: &str = "missing";
/// 未登录消息。
const MISSING_TOKEN: &str = "missing token";
/// 假令牌消息。
const INVALID_TOKEN: &str = "invalid token";
/// 过期消息。
const TOKEN_EXPIRED: &str = "token expired";
/// 吊销消息。
const TOKEN_REVOKED: &str = "token revoked";
/// 越权原因和消息。
const FORBIDDEN: &str = "forbidden";
/// 注入失败消息。
const INJECTED_FAILURE: &str = "injected failure";
/// 重试响应头。
const RETRY_AFTER: &str = "retry-after: 3";
/// 风格字段。
const STYLE: &str = "style";
/// 状态字段。
const STATUS: &str = "status";
/// 吊销范围字段。
const SCOPE: &str = "scope";
/// Access 吊销范围。
const SCOPE_ACCESS: &str = "access";
/// 全量吊销范围。
const SCOPE_ALL: &str = "all";
/// 到达序号字段。
const IDX: &str = "idx";
/// 放行名次字段。
const RANK: &str = "rank";

/// 带风格和可选令牌的轮次请求头。
fn headers(style: &str, token: Option<&str>) -> String {
    let mut headers = format!("{RUN_HEADER}X-Test-Style: {style}\r\n");
    if let Some(token) = token {
        headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    headers
}

/// 校验完整失败语义。
fn assert_failure(response: (u16, Value), status: u16, code: i64, reason: &str, message: &str) {
    assert_eq!(response.0, status, "{}", response.1);
    assert_eq!(response.1[CODE], code);
    assert_eq!(response.1[DATA][REASON], reason);
    assert_eq!(response.1[MESSAGE], message);
}

/// 登录并取出令牌数据。
async fn login(s: &Server, options: Value) -> Value {
    let (status, value) = json(s.addr, POST, LOGIN, RUN_HEADER, options).await;
    assert_eq!((status, value[CODE].as_i64()), (200, Some(0)));
    value[DATA].clone()
}

/// 两种风格的四类 401、空 Bearer、签名和载荷篡改、错误前缀。
#[tokio::test]
async fn auth_reasons_in_both_styles() {
    let s = server().await;
    let valid = login(&s, json!({(USERNAME):USER})).await;
    let token = valid[ACCESS_TOKEN].as_str().unwrap();
    let segments: Vec<_> = token.split('.').collect();
    assert_eq!(segments.len(), 3);
    let claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(segments[1]).unwrap()).unwrap();
    assert!(claims["exp"].is_number());
    let bad_signature = format!("{}.{}.AAAA", segments[0], segments[1]);
    let bad_payload = format!(
        "{}.{}.{}",
        segments[0],
        URL_SAFE_NO_PAD.encode(br#"{"sub":"attacker","exp":9999999999}"#),
        segments[2]
    );
    let expired = login(&s, json!({(USERNAME):"expired_user","access_ttl_ms":0})).await;
    let revoked = login(&s, json!({(USERNAME):"logout_user"})).await;
    json(
        s.addr,
        POST,
        LOGOUT,
        RUN_HEADER,
        json!({(REFRESH_TOKEN):revoked[REFRESH_TOKEN]}),
    )
    .await;
    for (style, status) in STYLES {
        let h = headers(style, None);
        for token in [None, Some(EMPTY_TEXT)] {
            assert_failure(
                json(s.addr, GET, PRIVATE, &headers(style, token), EMPTY).await,
                status,
                401,
                MISSING,
                MISSING_TOKEN,
            );
        }
        for token in ["not-a-jwt", bad_signature.as_str(), bad_payload.as_str()] {
            assert_failure(
                json(s.addr, GET, PRIVATE, &headers(style, Some(token)), EMPTY).await,
                status,
                401,
                INVALID,
                INVALID_TOKEN,
            );
        }
        let wrong_prefix = format!("{h}Authorization: Token {token}\r\n");
        assert_failure(
            json(s.addr, GET, PRIVATE, &wrong_prefix, EMPTY).await,
            status,
            401,
            INVALID,
            INVALID_TOKEN,
        );
        assert_failure(
            json(
                s.addr,
                GET,
                PRIVATE,
                &headers(style, expired[ACCESS_TOKEN].as_str()),
                EMPTY,
            )
            .await,
            status,
            401,
            EXPIRED,
            TOKEN_EXPIRED,
        );
        assert_failure(
            json(
                s.addr,
                GET,
                PRIVATE,
                &headers(style, revoked[ACCESS_TOKEN].as_str()),
                EMPTY,
            )
            .await,
            status,
            401,
            REVOKED,
            TOKEN_REVOKED,
        );
    }
}

/// 两种风格的越权和三种失败注入均保留 Retry-After。
#[tokio::test]
async fn forbidden_and_injected_retry_in_both_styles() {
    let s = server().await;
    let tokens = login(&s, json!({(USERNAME):USER})).await;
    for (style, _) in STYLES {
        let h = headers(style, tokens[ACCESS_TOKEN].as_str());
        assert_failure(
            json(s.addr, GET, &format!("{PRIVATE}?deny=1"), &h, EMPTY).await,
            if style == HTTP { 403 } else { 200 },
            403,
            FORBIDDEN,
            FORBIDDEN,
        );
        for injection in ["fail_times=1", "fail_every=1", "fail_on=1"] {
            let path = format!("{ECHO}?key={style}-{injection}&{injection}&retry_after=3");
            let bytes = raw(s.addr, GET, &path, &h, EMPTY_TEXT).await.unwrap();
            assert!(String::from_utf8_lossy(&bytes)
                .to_ascii_lowercase()
                .contains(RETRY_AFTER));
            let (status, body) = decode(&bytes);
            assert_failure(
                (status, serde_json::from_slice(&body).unwrap()),
                if style == HTTP { 500 } else { 200 },
                500,
                INJECTED,
                INJECTED_FAILURE,
            );
        }
    }
}

/// 登录、刷新各类失败遵循风格及附录消息。
#[tokio::test]
async fn auth_endpoint_failures_in_both_styles() {
    let s = server().await;
    for (style, status) in STYLES {
        let h = headers(style, None);
        assert_failure(
            json(
                s.addr,
                POST,
                LOGIN,
                &h,
                json!({(USERNAME):USER,"password":"wrong"}),
            )
            .await,
            status,
            401,
            "bad_credentials",
            "invalid username or password",
        );
        assert_failure(
            json(
                s.addr,
                POST,
                REFRESH,
                &h,
                json!({(REFRESH_TOKEN):"unknown"}),
            )
            .await,
            status,
            401,
            INVALID,
            "invalid refresh token",
        );
        let expired = login(&s, json!({(USERNAME):USER,"refresh_ttl_ms":0})).await;
        assert_failure(
            json(s.addr, POST, REFRESH, &h, expired).await,
            status,
            401,
            EXPIRED,
            "refresh token expired",
        );
        let first = login(&s, json!({(USERNAME):USER})).await;
        let (success, next) = json(s.addr, POST, REFRESH, &h, first.clone()).await;
        assert_eq!(success, 200);
        assert_eq!(next[CODE], 0);
        assert_failure(
            json(s.addr, POST, REFRESH, &h, first).await,
            status,
            401,
            "reused",
            "refresh token reused",
        );
        assert_failure(
            json(
                s.addr,
                GET,
                PRIVATE,
                &headers(style, next[DATA][ACCESS_TOKEN].as_str()),
                EMPTY,
            )
            .await,
            status,
            401,
            REVOKED,
            TOKEN_REVOKED,
        );
        let path = format!("{REFRESH}?key=refresh-{style}&fail_times=1");
        let tokens = login(&s, json!({(USERNAME):USER})).await;
        assert_failure(
            json(s.addr, POST, &path, &h, tokens.clone()).await,
            if style == HTTP { 500 } else { 200 },
            500,
            INJECTED,
            INJECTED_FAILURE,
        );
        assert_eq!(json(s.addr, POST, &path, &h, tokens).await.1[CODE], 0);
    }
}

/// 严格轮换使全部旧 access 失效，默认轮换保留旧 access。
#[tokio::test]
async fn strict_rotation_revokes_previous_access() {
    let s = server().await;
    for strict in [false, true] {
        let first = login(&s, json!({(USERNAME):USER,"strict_rotation":strict})).await;
        let (_, second) = json(s.addr, POST, REFRESH, RUN_HEADER, first.clone()).await;
        let (_, third) = json(s.addr, POST, REFRESH, RUN_HEADER, second[DATA].clone()).await;
        for (style, status) in STYLES {
            for old in [&first, &second[DATA]] {
                let response = json(
                    s.addr,
                    GET,
                    PRIVATE,
                    &headers(style, old[ACCESS_TOKEN].as_str()),
                    EMPTY,
                )
                .await;
                if strict {
                    assert_failure(response, status, 401, REVOKED, TOKEN_REVOKED);
                } else {
                    assert_eq!((response.0, response.1[CODE].as_i64()), (200, Some(0)));
                }
            }
            assert_eq!(
                json(
                    s.addr,
                    GET,
                    PRIVATE,
                    &headers(style, third[DATA][ACCESS_TOKEN].as_str()),
                    EMPTY
                )
                .await
                .1[CODE],
                0
            );
        }
    }
}

/// 无 refresh 用户的登录响应不包含该字段。
#[tokio::test]
async fn no_refresh_user_omits_refresh_token() {
    let s = server().await;
    let tokens = login(&s, json!({(USERNAME):"no_refresh_alice"})).await;
    assert!(tokens.get(REFRESH_TOKEN).is_none());
    assert_eq!(
        json(
            s.addr,
            GET,
            PRIVATE,
            &headers(HTTP, tokens[ACCESS_TOKEN].as_str()),
            EMPTY
        )
        .await
        .1[CODE],
        0
    );
}

/// 两种 scope 的吊销数量、用户隔离、刷新恢复与默认全部吊销。
#[tokio::test]
async fn revoke_scopes_and_counts() {
    let s = server().await;
    for scope in [Some(SCOPE_ACCESS), Some(SCOPE_ALL), None] {
        let first = login(&s, json!({(USERNAME):USER})).await;
        let second = login(&s, json!({(USERNAME):USER})).await;
        let other = login(&s, json!({(USERNAME):"unaffected"})).await;
        let mut body = json!({(USERNAME):USER});
        if let Some(scope) = scope {
            body[SCOPE] = json!(scope);
        }
        let (_, result) = json(s.addr, POST, REVOKE, RUN_HEADER, body.clone()).await;
        assert_eq!(
            result[DATA][REVOKED],
            if scope == Some(SCOPE_ACCESS) { 2 } else { 4 }
        );
        assert_eq!(
            json(s.addr, POST, REVOKE, RUN_HEADER, body).await.1[DATA][REVOKED],
            0
        );
        for tokens in [first, second] {
            assert_failure(
                json(
                    s.addr,
                    GET,
                    PRIVATE,
                    &headers(HTTP, tokens[ACCESS_TOKEN].as_str()),
                    EMPTY,
                )
                .await,
                401,
                401,
                REVOKED,
                TOKEN_REVOKED,
            );
            let response = json(s.addr, POST, REFRESH, RUN_HEADER, tokens).await;
            if scope == Some(SCOPE_ACCESS) {
                assert_eq!(response.1[CODE], 0);
                json(s.addr, POST, LOGOUT, RUN_HEADER, response.1[DATA].clone()).await;
            } else {
                assert_failure(response, 401, 401, REVOKED, TOKEN_REVOKED);
            }
        }
        assert_eq!(
            json(
                s.addr,
                GET,
                PRIVATE,
                &headers(HTTP, other[ACCESS_TOKEN].as_str()),
                EMPTY
            )
            .await
            .1[CODE],
            0
        );
    }
}

/// 单请求风格覆盖头部，显式业务码和状态推导、日志原因正确。
#[tokio::test]
async fn style_precedence_status_code_and_logs() {
    let s = server().await;
    for (header, query, expected) in [(BUSINESS, HTTP, 401), (HTTP, BUSINESS, 200)] {
        let path = format!("{PRIVATE}?style={query}");
        assert_eq!(
            json(s.addr, GET, &path, &headers(header, None), EMPTY)
                .await
                .0,
            expected
        );
    }
    for (style, _) in STYLES {
        let h = headers(style, None);
        for (query, http_status, code) in [
            ("status=503", 503, 503),
            ("code=1001", 200, 1001),
            ("status=500&code=5001", 500, 5001),
            ("status=200", 200, 0),
            ("status=503&code=0", 503, 0),
        ] {
            let response = json(s.addr, GET, &format!("{ECHO}?{query}"), &h, EMPTY).await;
            assert_eq!(response.0, if style == HTTP { http_status } else { 200 });
            assert_eq!(response.1[CODE], code);
            if http_status != 200 || code != 0 {
                assert_eq!(response.1[DATA][REASON], INJECTED);
            } else {
                assert!(response.1[DATA].get(REASON).is_none());
            }
        }
        assert_eq!(
            json(s.addr, GET, "/t/mock/demo", &h, EMPTY).await.1[DATA]["mock"],
            true
        );
    }
    let (_, stats) = json(s.addr, GET, STATS, EMPTY_TEXT, EMPTY).await;
    let log = stats[DATA]["log"].as_array().unwrap();
    assert_eq!(
        (
            log[0][STYLE].as_str(),
            log[0][STATUS].as_u64(),
            log[0][REASON].as_str()
        ),
        (Some(HTTP), Some(401), Some(MISSING))
    );
    assert_eq!(
        (log[1][STYLE].as_str(), log[1][STATUS].as_u64()),
        (Some(BUSINESS), Some(200))
    );
    assert!(log
        .iter()
        .all(|entry| entry.get(REASON).is_some() && entry.get(STYLE).is_some()));
    assert!(log.iter().any(|entry| entry[REASON] == INJECTED));
    assert!(log
        .iter()
        .any(|entry| entry[REASON].is_null() && entry[CODE] == 0));
}

/// Barrier 失败在两种风格保留序号、名次和重试头。
#[tokio::test]
async fn barrier_failure_styles_and_metadata() {
    let s = server().await;
    for (style, _) in STYLES {
        let path = "/t/barrier/fail?n=1&idx=0&fail=all&retry_after=3";
        let bytes = raw(s.addr, GET, path, &headers(style, None), EMPTY_TEXT)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes)
            .to_ascii_lowercase()
            .contains(RETRY_AFTER));
        let (status, body) = decode(&bytes);
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value[DATA][IDX], 0);
        assert_eq!(value[DATA][RANK], 0);
        assert_failure(
            (status, value),
            if style == HTTP { 500 } else { 200 },
            500,
            INJECTED,
            "barrier fail",
        );
    }
}

/// 登录、毫秒过期、慢刷新失败重试、轮换重用、吊销和跨轮次隔离。
#[tokio::test]
async fn auth_lifecycle() {
    let s = server().await;
    assert_eq!(
        json(
            s.addr,
            POST,
            LOGIN,
            RUN_HEADER,
            json!({(USERNAME):USER,"password":"wrong"})
        )
        .await
        .0,
        401
    );
    let (_, t) = json(
        s.addr,
        POST,
        LOGIN,
        RUN_HEADER,
        json!({(USERNAME):USER,"access_ttl_ms":120}),
    )
    .await;
    let auth = format!(
        "{RUN_HEADER}Authorization: Bearer {}\r\n",
        t[DATA][ACCESS_TOKEN].as_str().unwrap()
    );
    let (_, v) = json(s.addr, GET, PRIVATE, &auth, Value::Null).await;
    assert_eq!(v[DATA]["meta"]["auth"], USER);
    tokio::time::sleep(Duration::from_millis(160)).await;
    let (status, v) = json(s.addr, GET, PRIVATE, &auth, Value::Null).await;
    assert_eq!((status, v[CODE].as_i64().unwrap()), (401, 401));
    let (status, v) = json(
        s.addr,
        GET,
        &format!("{PRIVATE}?style=business"),
        &auth,
        Value::Null,
    )
    .await;
    assert_eq!((status, v[CODE].as_i64().unwrap()), (200, 401));
    let refresh = json!({(REFRESH_TOKEN):t[DATA][REFRESH_TOKEN]});
    let path = format!("{REFRESH}?key=refresh&fail_times=1&delay_ms=20");
    assert_eq!(
        json(s.addr, POST, &path, RUN_HEADER, refresh.clone())
            .await
            .0,
        500
    );
    let (_, next) = json(s.addr, POST, &path, RUN_HEADER, refresh.clone()).await;
    assert!(next[DATA][ACCESS_TOKEN].is_string());
    let (status, reused) = json(s.addr, POST, REFRESH, RUN_HEADER, refresh).await;
    assert_eq!((status, reused[CODE].as_i64().unwrap()), (401, 401));
    let auth = format!(
        "{RUN_HEADER}Authorization: Bearer {}\r\n",
        next[DATA][ACCESS_TOKEN].as_str().unwrap()
    );
    assert_eq!(json(s.addr, GET, PRIVATE, &auth, Value::Null).await.0, 401);
    assert_eq!(
        json(
            s.addr,
            POST,
            REFRESH,
            RUN_HEADER,
            json!({(REFRESH_TOKEN):next[DATA][REFRESH_TOKEN]})
        )
        .await
        .0,
        401
    );
    let (_, t) = json(s.addr, POST, LOGIN, RUN_HEADER, json!({(USERNAME):USER})).await;
    let auth = format!(
        "{RUN_HEADER}Authorization: Bearer {}\r\n",
        t[DATA][ACCESS_TOKEN].as_str().unwrap()
    );
    assert_eq!(
        json(
            s.addr,
            GET,
            &format!("{PRIVATE}?deny=1"),
            &auth,
            Value::Null
        )
        .await
        .0,
        403
    );
    let other = format!(
        "Authorization: Bearer {}\r\nX-Test-Run: other\r\n",
        t[DATA][ACCESS_TOKEN].as_str().unwrap()
    );
    assert_eq!(json(s.addr, GET, PRIVATE, &other, Value::Null).await.0, 401);
    json(s.addr, POST, REVOKE, RUN_HEADER, json!({(USERNAME):USER})).await;
    assert_eq!(json(s.addr, GET, PRIVATE, &auth, Value::Null).await.0, 401);
    let (_, t) = json(s.addr, POST, LOGIN, RUN_HEADER, json!({(USERNAME):USER})).await;
    json(
        s.addr,
        POST,
        LOGOUT,
        RUN_HEADER,
        json!({(REFRESH_TOKEN):t[DATA][REFRESH_TOKEN]}),
    )
    .await;
    let auth = format!(
        "{RUN_HEADER}Authorization: Bearer {}\r\n",
        t[DATA][ACCESS_TOKEN].as_str().unwrap()
    );
    assert_eq!(json(s.addr, GET, PRIVATE, &auth, Value::Null).await.0, 401);
    assert_eq!(
        json(s.addr, GET, "/t/ping", EMPTY_TEXT, Value::Null)
            .await
            .1[CODE],
        0
    );
}

/// 读取一个完整响应且不消费下一个响应的字节，供真实 keep-alive 验证。
async fn one_response(stream: &mut TcpStream) -> (u16, Value) {
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut bytes = Vec::new();
        loop {
            bytes.push(stream.read_u8().await.expect("complete HTTP response"));
            if bytes.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let headers = String::from_utf8(bytes.clone())
            .unwrap()
            .to_ascii_lowercase();
        if headers.contains("transfer-encoding: chunked") {
            loop {
                bytes.push(stream.read_u8().await.unwrap());
                if bytes.ends_with(b"\r\n0\r\n\r\n") {
                    break;
                }
            }
        } else {
            let len: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let offset = bytes.len();
            bytes.resize(offset + len, 0);
            stream.read_exact(&mut bytes[offset..]).await.unwrap();
        }
        let (status, body) = decode(&bytes);
        (status, serde_json::from_slice(&body).unwrap())
    })
    .await
    .unwrap()
}

/// 同一 TCP 上的错误、业务失败、探针与成功连接号一致，新连接递增。
#[tokio::test]
async fn connection_ids_follow_tcp_keep_alive() {
    let s = server().await;
    let mut stream = TcpStream::connect(s.addr).await.unwrap();
    let mut conn = None;
    for (path, status, code) in [
        ("/t/private/a", 401, 401),
        ("/t/echo?status=500&style=business", 200, 500),
        ("/t/echo?read_delay_ms=bad", 400, 400),
        ("/t/ping", 200, 0),
        (STATS, 200, 0),
        (ECHO, 200, 0),
    ] {
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{RUN_HEADER}\r\n").as_bytes(),
            )
            .await
            .unwrap();
        let (actual_status, value) = one_response(&mut stream).await;
        assert_eq!(
            (actual_status, value[CODE].as_i64().unwrap()),
            (status, code)
        );
        let id = value[DATA]["meta"]["conn"].as_u64().unwrap();
        assert!(id > 0);
        assert_eq!(*conn.get_or_insert(id), id);
    }
    let (_, next) = json(s.addr, GET, ECHO, RUN_HEADER, EMPTY).await;
    assert!(next[DATA]["meta"]["conn"].as_u64().unwrap() > conn.unwrap());
    let another_server = server().await;
    let (_, next) = json(another_server.addr, GET, ECHO, RUN_HEADER, EMPTY).await;
    assert!(next[DATA]["meta"]["conn"].as_u64().unwrap() > conn.unwrap());
    let (_, ordinary) = json(s.addr, GET, "/health", "", EMPTY).await;
    assert!(ordinary[DATA].get("meta").is_none());
}

/// 大流在延迟期写不完且无提前 413；小请求正常回显仍等待读延迟。
#[tokio::test]
async fn read_delay_blocks_upload_before_body_limit() {
    let s = server().await;
    let at = Instant::now();
    let (_, value) = json(
        s.addr,
        POST,
        "/t/echo?read_delay_ms=150",
        RUN_HEADER,
        json!({"a":1}),
    )
    .await;
    assert!(at.elapsed() >= Duration::from_millis(150));
    assert_eq!(value[DATA]["request"]["body"]["json"], json!({"a":1}));
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    let mut stream = socket.connect(s.addr).await.unwrap();
    let body = [b'x'; 16 * 1024];
    stream.write_all(format!("POST /t/echo?read_delay_ms=1200 HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n{RUN_HEADER}\r\n", 64 * 1024 * 1024).as_bytes()).await.unwrap();
    let at = Instant::now();
    let write = tokio::time::timeout(Duration::from_millis(250), async {
        for _ in 0..4096 {
            stream.write_all(&body).await?;
        }
        Ok::<_, std::io::Error>(())
    })
    .await;
    assert!(
        write.is_err(),
        "upload should block before body is read, got {write:?}"
    );
    let mut byte = [0];
    assert!(
        tokio::time::timeout(Duration::from_millis(250), stream.read(&mut byte))
            .await
            .is_err(),
        "server read and rejected body before read delay elapsed"
    );
    // 取消的 write_all 可能只写入了少量字节，继续发送以跨过 1MiB 上限。
    let (mut reader, mut writer) = stream.split();
    let (read, _) = tokio::time::timeout(Duration::from_secs(8), async {
        tokio::join!(
            async {
                let mut bytes = Vec::new();
                loop {
                    let byte = reader.read_u8().await?;
                    bytes.push(byte);
                    if bytes.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                Ok::<_, std::io::Error>(bytes)
            },
            async {
                for _ in 0..4096 {
                    if writer.write_all(&body).await.is_err() {
                        break;
                    }
                }
            }
        )
    })
    .await
    .unwrap();
    assert!(at.elapsed() >= Duration::from_millis(1200));
    match read {
        Ok(bytes) => assert!(String::from_utf8(bytes)
            .unwrap()
            .starts_with("HTTP/1.1 413")),
        // 未读完巨型请求即返回 413 时，Windows/Linux 可能先报告套接字中止。
        Err(error) => assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
            ),
            "{error}"
        ),
    }
    let (_, stats) = json(s.addr, GET, STATS, RUN_HEADER, EMPTY).await;
    assert!(stats[DATA]["log"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["status"] == 413));
    for query in ["read_delay_ms=-1", "read_delay_ms=abc", "read_delay_ms=1.5"] {
        let (_, value) = json(
            s.addr,
            POST,
            &format!("/t/echo?{query}&style=business"),
            RUN_HEADER,
            EMPTY,
        )
        .await;
        assert_eq!(value[CODE], 400);
        assert!(value[DATA]["meta"]["conn"].as_u64().is_some());
    }
}

/// 直接发送二进制 multipart，避免 JSON 客户端帮忙改变请求字节。
async fn multipart_request(
    s: &Server,
    path: &str,
    content_type: &str,
    body: &[u8],
    extra: &str,
) -> (u16, Value) {
    let mut stream = TcpStream::connect(s.addr).await.unwrap();
    stream.write_all(format!("POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{RUN_HEADER}{extra}\r\n", body.len()).as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    one_response(&mut stream).await
}

/// 文本、重复字段、二进制、空文件和所有 echo 路径共享解析行为。
#[tokio::test]
async fn multipart_echo_fields_files_and_digest() {
    let s = server().await;
    let token = login(&s, json!({"username":"uploader"})).await;
    let mut body = concat!(
        "--test-boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nold\r\n",
        "--test-boundary\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n示例\r\n",
        "--test-boundary\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"a.txt\"\r\n\r\nabc\r\n",
        "--test-boundary\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"binary.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    ).as_bytes().to_vec();
    body.extend_from_slice(&[0, 255, 128, 13, 10]);
    body.extend_from_slice(b"\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"empty\"; filename=\"\"\r\n\r\n\r\n--test-boundary--\r\n");
    for path in [ECHO, "/t/public/upload", "/t/mock/upload", PRIVATE] {
        let (status, value) = multipart_request(
            &s,
            path,
            "multipart/form-data; boundary=\"test-boundary\"",
            &body,
            &format!(
                "Authorization: Bearer {}\r\n",
                token[ACCESS_TOKEN].as_str().unwrap()
            ),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(
            value[DATA]["multipart"],
            json!({
                "fields":{"title":"示例"},
                "files":[
                    {"name":"upload","filename":"a.txt","size":3,"sha256":"ba7816bf8f01cfea"},
                    {"name":"upload","filename":"binary.bin","size":5,"sha256":"6171db06a1c89b1f"},
                    {"name":"empty","filename":"","size":0,"sha256":"e3b0c44298fc1c14"}
                ]
            })
        );
    }
    let (_, value) = json(s.addr, POST, ECHO, RUN_HEADER, json!({"normal":true})).await;
    assert!(value[DATA].get("multipart").is_none());
}

/// 损坏的 multipart 遵循两种风格，不把部分解析结果当成成功。
#[tokio::test]
async fn malformed_multipart_respects_response_style() {
    let s = server().await;
    for (style, expected) in [(HTTP, 400), (BUSINESS, 200)] {
        for (content_type, body) in [
            ("multipart/form-data", b"bad".as_slice()),
            (
                "multipart/form-data; boundary=b",
                b"--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\ntruncated".as_slice(),
            ),
            (
                "multipart/form-data; boundary=b",
                b"--b\r\nContent-Disposition: form-data\r\n\r\nx\r\n--b--\r\n".as_slice(),
            ),
            (
                "multipart/form-data; boundary=b",
                b"--b\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n\xff\r\n--b--\r\n"
                    .as_slice(),
            ),
        ] {
            let (status, value) = multipart_request(
                &s,
                &format!("/t/echo?style={style}"),
                content_type,
                body,
                "",
            )
            .await;
            assert_eq!(status, expected, "{value}");
            assert_eq!(value[CODE], 400);
            assert!(value[DATA]["meta"]["conn"].as_u64().is_some());
        }
    }
}

/// 执行三请求 barrier，返回按响应落定的 idx、rank、业务码。
async fn barrier(s: &Server, query: &str) -> Vec<(usize, usize, i64)> {
    let mut tasks = tokio::task::JoinSet::new();
    for idx in 0..3 {
        let addr = s.addr;
        let path = format!("/t/barrier/g?n=3&idx={idx}&gap_ms=35&{query}");
        tasks.spawn(async move {
            let (_, v) = json(addr, GET, &path, RUN_HEADER, Value::Null).await;
            (
                v[DATA][IDX].as_u64().unwrap() as usize,
                v[DATA][RANK].as_u64().unwrap() as usize,
                v[CODE].as_i64().unwrap(),
            )
        });
    }
    let mut results = Vec::new();
    while let Some(value) = tasks.join_next().await {
        results.push(value.unwrap());
    }
    results
}

/// asc、desc、shuffle、given、部分/全部失败、超时 partial 与自动复用。
#[tokio::test]
async fn barrier_orders() {
    let s = server().await;
    assert_eq!(
        barrier(&s, "order=asc").await,
        [(0, 0, 0), (1, 1, 0), (2, 2, 0)]
    );
    assert_eq!(
        barrier(&s, "order=desc&fail=1").await,
        [(2, 0, 0), (1, 1, 500), (0, 2, 0)]
    );
    let a = barrier(&s, "order=shuffle&seed=17").await;
    let b = barrier(&s, "order=shuffle&seed=17").await;
    assert_eq!(a, b);
    assert_eq!(
        barrier(&s, "order=given&seq=1,2,0&fail=all").await,
        [(1, 0, 500), (2, 1, 500), (0, 2, 500)]
    );
    let (_, v) = json(
        s.addr,
        GET,
        "/t/barrier/partial?n=3&idx=2&wait_ms=25",
        RUN_HEADER,
        Value::Null,
    )
    .await;
    assert_eq!(v[DATA]["meta"]["partial"], true);
    assert_eq!(v[DATA][RANK], 0);
    assert_eq!(
        json(
            s.addr,
            GET,
            "/t/barrier/bad?n=65&idx=0",
            RUN_HEADER,
            Value::Null
        )
        .await
        .0,
        400
    );
}

/// 全部 fault 的真实字节表现、取消记录、计数、回显及通用控制。
#[tokio::test]
async fn faults_and_observation() {
    let s = server().await;
    let (_, v) = json(
        s.addr,
        "PATCH",
        "/t/echo?run=integration&data=%7B%22x%22:1%7D&size=8",
        "X-Custom: yes\r\nX-Hide: test\r\nCookie: hidden\r\n",
        json!({"body":true}),
    )
    .await;
    assert_eq!(v[DATA]["request"]["body"]["json"], json!({"body":true}));
    assert!(v[DATA]["request"]["headers"].get("cookie").is_none());
    assert_eq!(v[DATA]["payload"], json!({"x":1}));
    assert_eq!(v[DATA]["pad"].as_str().unwrap().len(), 8);
    for (fault, expected) in [
        ("empty", EMPTY_TEXT.as_bytes()),
        ("bad_json", b"{invalid json".as_slice()),
    ] {
        let bytes = raw(
            s.addr,
            GET,
            &format!("/t/echo?fault={fault}"),
            RUN_HEADER,
            EMPTY_TEXT,
        )
        .await
        .unwrap();
        assert_eq!(decode(&bytes).1, expected);
    }
    let bytes = raw(s.addr, GET, "/t/echo?fault=html", RUN_HEADER, EMPTY_TEXT)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("text/html"));
    assert!(decode(&bytes).1.starts_with(b"<!doctype"));
    let (_, bare) = json(
        s.addr,
        GET,
        "/t/echo?fault=no_envelope",
        RUN_HEADER,
        Value::Null,
    )
    .await;
    assert_eq!(bare, json!([1, 2, 3]));
    let bytes = raw(s.addr, GET, "/t/echo?fault=close", RUN_HEADER, EMPTY_TEXT).await;
    assert!(
        bytes.is_err() || bytes.unwrap().is_empty(),
        "close must not send headers"
    );
    let mut stream = TcpStream::connect(s.addr).await.unwrap();
    send(
        &mut stream,
        GET,
        "/t/echo?fault=half_body&size=4096",
        RUN_HEADER,
        EMPTY_TEXT,
    )
    .await
    .unwrap();
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes).await;
    assert!(bytes.starts_with(b"HTTP/1.1 200"));
    assert!(
        !bytes.ends_with(b"0\r\n\r\n"),
        "half body must be truncated"
    );
    let at = Instant::now();
    let (_, v) = json(
        s.addr,
        GET,
        "/t/echo?fault=slow_body&chunk_ms=20&size=4096",
        RUN_HEADER,
        Value::Null,
    )
    .await;
    assert_eq!(v[DATA]["pad"].as_str().unwrap().len(), 4096);
    assert!(at.elapsed() >= Duration::from_millis(80));
    let mut stream = TcpStream::connect(s.addr).await.unwrap();
    send(
        &mut stream,
        GET,
        "/t/echo?fault=hang&key=cancel",
        RUN_HEADER,
        EMPTY_TEXT,
    )
    .await
    .unwrap();
    let mut first = [0u8; 1];
    assert!(
        tokio::time::timeout(Duration::from_millis(60), stream.read(&mut first))
            .await
            .is_err()
    );
    drop(stream);
    let mut aborted = false;
    for _ in 0..30 {
        let (_, v) = json(s.addr, GET, STATS, EMPTY_TEXT, Value::Null).await;
        aborted = v[DATA]["log"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["key"] == "cancel" && row["aborted"] == true);
        if aborted {
            assert_eq!(v[DATA]["inflight"], 0);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        aborted,
        "disconnect must cancel handler future and record aborted"
    );
    let bytes = raw(
        s.addr,
        GET,
        "/t/echo?key=flaky&fail_every=2&retry_after=3&header=x-extra:ok",
        RUN_HEADER,
        EMPTY_TEXT,
    )
    .await
    .unwrap();
    assert_eq!(decode(&bytes).0, 200);
    let bytes = raw(
        s.addr,
        GET,
        "/t/echo?key=flaky&fail_every=2&retry_after=3",
        RUN_HEADER,
        EMPTY_TEXT,
    )
    .await
    .unwrap();
    assert_eq!(decode(&bytes).0, 500);
    assert!(String::from_utf8_lossy(&bytes)
        .to_ascii_lowercase()
        .contains(RETRY_AFTER));
    json(s.addr, DELETE, STATS, EMPTY_TEXT, Value::Null).await;
    assert_eq!(
        json(s.addr, GET, STATS, EMPTY_TEXT, Value::Null).await.1[DATA]["total"],
        0
    );
    json(
        s.addr,
        GET,
        "/t/echo?run=encoded%20run",
        EMPTY_TEXT,
        Value::Null,
    )
    .await;
    json(
        s.addr,
        DELETE,
        "/t/runs/encoded%20run",
        EMPTY_TEXT,
        Value::Null,
    )
    .await;
    assert_eq!(
        json(
            s.addr,
            GET,
            "/t/runs/encoded%20run",
            EMPTY_TEXT,
            Value::Null
        )
        .await
        .1[DATA]["total"],
        0
    );
}
