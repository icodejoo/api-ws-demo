//! 按 HTTP/1.x 请求边界改写 Host 头，供明文客户端访问按 Host 路由的 TLS 上游。
use std::mem;

/// 请求头最大缓冲；超过后放弃改写，整条连接原样透传。
const MAX_HEAD: usize = 64 * 1024;
/// 请求头结束标记。
const HEAD_END: &[u8] = b"\r\n\r\n";

/// 当前正在解析的位置。
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// 等待完整请求头。
    Head,
    /// 定长请求体，剩余字节数。
    Body(u64),
    /// 分块体的块大小行。
    SizeLine,
    /// 分块数据（含末尾 CRLF），剩余字节数。
    Data(u64),
    /// 末块之后的 trailer，当前行的有效字节数。
    Trailer(usize),
    /// 升级（WebSocket）或无法解析，之后原样透传。
    Pass,
}

/// 有状态的请求流改写器，同一连接上的每个请求都会被改写。
pub(super) struct HostRewriter {
    /// 目标 Host 值。
    host: String,
    /// 解析状态。
    state: State,
    /// 请求头或块大小行的暂存。
    buf: Vec<u8>,
}

impl HostRewriter {
    /// 创建改写器，`host` 是要写入的 Host 头值。
    pub(super) fn new(host: &str) -> Self {
        Self {
            host: host.to_string(),
            state: State::Head,
            buf: Vec::new(),
        }
    }

    /// 喂入一段客户端字节，返回应转发给上游的字节；未凑齐的请求头会暂存，到齐后一并输出。
    pub(super) fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 32);
        let mut rest = data;
        while !rest.is_empty() {
            match self.state {
                State::Pass => {
                    out.extend_from_slice(rest);
                    break;
                }
                State::Body(n) | State::Data(n) => {
                    let take = n.min(rest.len() as u64) as usize;
                    out.extend_from_slice(&rest[..take]);
                    rest = &rest[take..];
                    let left = n - take as u64;
                    self.state = match (self.state, left) {
                        (State::Body(_), 0) => State::Head,
                        (State::Data(_), 0) => State::SizeLine,
                        (State::Body(_), n) => State::Body(n),
                        (_, n) => State::Data(n),
                    };
                }
                State::SizeLine => {
                    let end = rest.iter().position(|b| *b == b'\n');
                    let take = end.map_or(rest.len(), |p| p + 1);
                    self.buf.extend_from_slice(&rest[..take]);
                    out.extend_from_slice(&rest[..take]);
                    rest = &rest[take..];
                    if end.is_some() {
                        let size = chunk_size(&self.buf);
                        self.buf.clear();
                        self.state = match size {
                            Some(0) => State::Trailer(0),
                            Some(n) => State::Data(n + 2),
                            None => State::Pass,
                        };
                    }
                }
                State::Trailer(mut len) => {
                    let mut used = rest.len();
                    let mut finished = false;
                    for (i, b) in rest.iter().enumerate() {
                        match b {
                            b'\n' if len == 0 => {
                                used = i + 1;
                                finished = true;
                                break;
                            }
                            b'\n' => len = 0,
                            b'\r' => {}
                            _ => len += 1,
                        }
                    }
                    self.state = if finished {
                        State::Head
                    } else {
                        State::Trailer(len)
                    };
                    out.extend_from_slice(&rest[..used]);
                    rest = &rest[used..];
                }
                State::Head => {
                    let scan_from = self.buf.len().saturating_sub(HEAD_END.len() - 1);
                    self.buf.extend_from_slice(rest);
                    let Some(at) = find(&self.buf[scan_from..], HEAD_END)
                        .map(|i| i + scan_from + HEAD_END.len())
                    else {
                        if self.buf.len() > MAX_HEAD {
                            out.append(&mut self.buf);
                            self.state = State::Pass;
                        }
                        break;
                    };
                    // buf 中超出请求头的部分一定来自 rest 的尾部，留给下一轮。
                    let extra = self.buf.len() - at;
                    rest = &rest[rest.len() - extra..];
                    let head = mem::take(&mut self.buf);
                    let (bytes, next) = rewrite_head(&head[..at], &self.host);
                    out.extend_from_slice(&bytes);
                    self.state = next;
                }
            }
        }
        out
    }
}

/// 在字节串里查找子串。
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 解析分块大小行（十六进制，忽略扩展）；格式不对返回 None。
fn chunk_size(line: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(line).ok()?;
    let digits = text.split([';', '\r', '\n']).next()?.trim();
    u64::from_str_radix(digits, 16).ok()
}

/// 改写完整请求头，返回新头与随后的解析状态。
fn rewrite_head(head: &[u8], host: &str) -> (Vec<u8>, State) {
    let mut out = Vec::with_capacity(head.len() + host.len());
    let mut length = 0u64;
    let mut chunked = false;
    let mut upgrade = false;
    let mut first = true;
    let mut rest = head;
    while let Some(i) = find(rest, b"\r\n") {
        let line = &rest[..i];
        rest = &rest[i + 2..];
        if first {
            first = false;
            upgrade = line.starts_with(b"CONNECT ");
            out.extend_from_slice(line);
            out.extend_from_slice(b"\r\n");
            continue;
        }
        if line.is_empty() {
            out.extend_from_slice(b"\r\n");
            break;
        }
        let (name, value) = match line.iter().position(|b| *b == b':') {
            Some(c) => (line[..c].trim_ascii(), line[c + 1..].trim_ascii()),
            None => (&line[..0], &line[..0]),
        };
        if name.eq_ignore_ascii_case(b"host") {
            out.extend_from_slice(b"Host: ");
            out.extend_from_slice(host.as_bytes());
            out.extend_from_slice(b"\r\n");
            continue;
        }
        if name.eq_ignore_ascii_case(b"content-length") {
            length = std::str::from_utf8(value)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if name.eq_ignore_ascii_case(b"transfer-encoding") {
            chunked = value
                .to_ascii_lowercase()
                .windows(7)
                .any(|w| w == b"chunked");
        } else if name.eq_ignore_ascii_case(b"upgrade") {
            upgrade = true;
        }
        out.extend_from_slice(line);
        out.extend_from_slice(b"\r\n");
    }
    let next = if upgrade {
        State::Pass
    } else if chunked {
        State::SizeLine
    } else if length > 0 {
        State::Body(length)
    } else {
        State::Head
    };
    (out, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 逐字节喂入，拼出全部输出。
    fn drip(r: &mut HostRewriter, data: &[u8]) -> Vec<u8> {
        data.iter().flat_map(|b| r.feed(&[*b])).collect()
    }

    /// 请求头被拆成任意小片也能改写 Host，其余头不变。
    #[test]
    fn rewrites_split_head() {
        let mut r = HostRewriter::new("api.example.com");
        let out = drip(
            &mut r,
            b"GET /a HTTP/1.1\r\nhOsT: 127.0.0.1:5\r\nX-A: 1\r\n\r\n",
        );
        assert_eq!(
            out,
            b"GET /a HTTP/1.1\r\nHost: api.example.com\r\nX-A: 1\r\n\r\n"
        );
    }

    /// keep-alive：定长体里长得像 Host 头的内容不改，下一个请求照改。
    #[test]
    fn keep_alive_and_body() {
        let mut r = HostRewriter::new("h");
        let body = "Host: keep\r\n\r\n";
        let n = body.len();
        let req = format!(
            "POST /x HTTP/1.1\r\nHost: a\r\nContent-Length: {n}\r\n\r\n{body}GET /y HTTP/1.1\r\nHost: b\r\n\r\n"
        );
        let out = String::from_utf8(r.feed(req.as_bytes())).unwrap();
        assert_eq!(
            out,
            format!("POST /x HTTP/1.1\r\nHost: h\r\nContent-Length: {n}\r\n\r\n{body}GET /y HTTP/1.1\r\nHost: h\r\n\r\n")
        );
    }

    /// 分块请求体（含 trailer）按块跳过，之后的请求照改。
    #[test]
    fn chunked_body() {
        let mut r = HostRewriter::new("h");
        let req = b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nHost:\r\n0\r\nX-T: 1\r\n\r\nGET / HTTP/1.1\r\nHost: b\r\n\r\n";
        let want = b"POST / HTTP/1.1\r\nHost: h\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nHost:\r\n0\r\nX-T: 1\r\n\r\nGET / HTTP/1.1\r\nHost: h\r\n\r\n";
        assert_eq!(drip(&mut r, req), want);
    }

    /// 升级请求之后原样透传；超长头放弃改写。
    #[test]
    fn upgrade_and_oversize() {
        let mut r = HostRewriter::new("h");
        let out =
            r.feed(b"GET /ws HTTP/1.1\r\nHost: a\r\nUpgrade: websocket\r\n\r\nHost: raw\r\n\r\n");
        assert_eq!(
            out,
            b"GET /ws HTTP/1.1\r\nHost: h\r\nUpgrade: websocket\r\n\r\nHost: raw\r\n\r\n"
        );
        let mut r = HostRewriter::new("h");
        let big = vec![b'a'; MAX_HEAD + 1];
        assert_eq!(r.feed(&big), big);
        assert_eq!(r.state, State::Pass);
    }
}
