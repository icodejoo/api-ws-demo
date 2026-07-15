#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StompCommand {
    Connect,
    Stomp,
    Send,
    Subscribe,
    Unsubscribe,
    Ack,
    Nack,
    Begin,
    Commit,
    Abort,
    Disconnect,
    Connected,
    Message,
    Receipt,
    Error,
}

impl StompCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connect => "CONNECT",
            Self::Stomp => "STOMP",
            Self::Send => "SEND",
            Self::Subscribe => "SUBSCRIBE",
            Self::Unsubscribe => "UNSUBSCRIBE",
            Self::Ack => "ACK",
            Self::Nack => "NACK",
            Self::Begin => "BEGIN",
            Self::Commit => "COMMIT",
            Self::Abort => "ABORT",
            Self::Disconnect => "DISCONNECT",
            Self::Connected => "CONNECTED",
            Self::Message => "MESSAGE",
            Self::Receipt => "RECEIPT",
            Self::Error => "ERROR",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "CONNECT" => Self::Connect,
            "STOMP" => Self::Stomp,
            "SEND" => Self::Send,
            "SUBSCRIBE" => Self::Subscribe,
            "UNSUBSCRIBE" => Self::Unsubscribe,
            "ACK" => Self::Ack,
            "NACK" => Self::Nack,
            "BEGIN" => Self::Begin,
            "COMMIT" => Self::Commit,
            "ABORT" => Self::Abort,
            "DISCONNECT" => Self::Disconnect,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StompFrame {
    pub command: StompCommand,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct FrameParseError(pub String);

impl StompFrame {
    pub fn new(command: StompCommand) -> Self {
        Self {
            command,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Parses a single frame (without the trailing NUL) from raw bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, FrameParseError> {
        let raw = trim_leading_eol(raw);
        let nl = raw
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| FrameParseError("missing command line".into()))?;
        let command_str = std::str::from_utf8(&raw[..nl])
            .map_err(|_| FrameParseError("non-utf8 command".into()))?
            .trim();
        let command = StompCommand::parse(command_str)
            .ok_or_else(|| FrameParseError(format!("unknown command '{command_str}'")))?;

        let mut idx = nl + 1;
        let mut headers = Vec::new();
        loop {
            let line_end = raw[idx..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| FrameParseError("missing header/body separator".into()))?;
            // STOMP 1.2 allows CRLF line endings: strip a trailing '\r' so the
            // blank-line separator (\r\n) reads as empty and header values don't
            // carry a stray '\r'.
            let mut content_end = line_end;
            if content_end > 0 && raw[idx + content_end - 1] == b'\r' {
                content_end -= 1;
            }
            if content_end == 0 {
                idx += line_end + 1;
                break;
            }
            let line = std::str::from_utf8(&raw[idx..idx + content_end])
                .map_err(|_| FrameParseError("non-utf8 header".into()))?;
            if let Some((k, v)) = line.split_once(':') {
                headers.push((decode_header(k), decode_header(v)));
            }
            idx += line_end + 1;
        }

        let content_length = headers
            .iter()
            .find(|(k, _)| k == "content-length")
            .and_then(|(_, v)| v.parse::<usize>().ok());

        let body = match content_length {
            Some(len) if idx + len <= raw.len() => raw[idx..idx + len].to_vec(),
            _ => {
                let nul = raw[idx..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(raw.len() - idx);
                raw[idx..idx + nul].to_vec()
            }
        };

        Ok(Self {
            command,
            headers,
            body,
        })
    }

    /// Serializes to wire format, including the trailing NUL terminator.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(self.command.as_str().as_bytes());
        out.push(b'\n');
        for (k, v) in &self.headers {
            out.extend_from_slice(encode_header(k).as_bytes());
            out.push(b':');
            out.extend_from_slice(encode_header(v).as_bytes());
            out.push(b'\n');
        }
        out.push(b'\n');
        out.extend_from_slice(&self.body);
        out.push(0);
        out
    }
}

fn trim_leading_eol(raw: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < raw.len() && (raw[i] == b'\n' || raw[i] == b'\r') {
        i += 1;
    }
    &raw[i..]
}

fn decode_header(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('c') => out.push(':'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn encode_header(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace(':', "\\c")
}

/// Wraps everything a connection's outgoing channel can carry: either a full
/// STOMP frame, or a bare heartbeat — which per spec is NOT a frame at all,
/// just a lone `\n` byte with no command/headers/body.
pub enum OutgoingItem {
    Frame(StompFrame),
    Heartbeat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_connect() {
        let frame = StompFrame::new(StompCommand::Connect)
            .header("accept-version", "1.2")
            .header("host", "localhost");
        let bytes = frame.serialize();
        let parsed = StompFrame::parse(&bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(parsed.command, StompCommand::Connect);
        assert_eq!(parsed.get("accept-version"), Some("1.2"));
        assert_eq!(parsed.get("host"), Some("localhost"));
    }

    #[test]
    fn parses_crlf_line_endings() {
        // STOMP 1.2 permits CRLF EOL; clients that use it must not be rejected.
        let raw = b"CONNECT\r\naccept-version:1.2\r\nhost:localhost\r\n\r\n";
        let parsed = StompFrame::parse(raw).unwrap();
        assert_eq!(parsed.command, StompCommand::Connect);
        assert_eq!(parsed.get("accept-version"), Some("1.2"));
        assert_eq!(parsed.get("host"), Some("localhost"));
    }

    #[test]
    fn round_trip_send_with_content_length_and_embedded_nul() {
        let body = vec![1u8, 0u8, 2u8, 3u8];
        let mut frame = StompFrame::new(StompCommand::Send)
            .header("destination", "/topic/test")
            .header("content-length", body.len().to_string());
        frame.body = body.clone();
        let bytes = frame.serialize();
        // strip trailing NUL terminator before parse, as connection.rs does
        let parsed = StompFrame::parse(&bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(parsed.command, StompCommand::Send);
        assert_eq!(parsed.get("destination"), Some("/topic/test"));
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn header_escaping_round_trip() {
        let frame = StompFrame::new(StompCommand::Send)
            .header("destination", "/topic/a:b\nc\\d");
        let bytes = frame.serialize();
        let parsed = StompFrame::parse(&bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(parsed.get("destination"), Some("/topic/a:b\nc\\d"));
    }

    #[test]
    fn rejects_unknown_command() {
        let err = StompFrame::parse(b"BOGUS\n\n").unwrap_err();
        assert!(err.0.contains("unknown command"));
    }
}
