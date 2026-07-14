//! Static, build-time pre-compressed/pre-encoded test payloads, shared by both
//! the STOMP static topics (`stomp::connection`) and the matching HTTP
//! endpoints (`compressed_http`). Every asset here was generated once,
//! offline, and embedded via `include_bytes!` — the server never spends CPU
//! compressing or encoding anything for these at request time.

#[derive(Clone, Copy)]
pub struct StaticAsset {
    pub bytes: &'static [u8],
    pub content_type: &'static str,
    pub content_encoding: Option<&'static str>,
}

const JSON_GZ: &[u8] = include_bytes!("../assets/compressed_sample.json.gz");
const JSON_ZST: &[u8] = include_bytes!("../assets/compressed_sample.json.zst");
const MSGPACK: &[u8] = include_bytes!("../assets/compressed_sample.msgpack");
const MSGPACK_GZ: &[u8] = include_bytes!("../assets/compressed_sample.msgpack.gz");
const MSGPACK_ZST: &[u8] = include_bytes!("../assets/compressed_sample.msgpack.zst");

pub const JSON_GZIP: StaticAsset = StaticAsset {
    bytes: JSON_GZ,
    content_type: "application/json",
    content_encoding: Some("gzip"),
};
pub const JSON_ZSTD: StaticAsset = StaticAsset {
    bytes: JSON_ZST,
    content_type: "application/json",
    content_encoding: Some("zstd"),
};
pub const MSGPACK_PLAIN: StaticAsset = StaticAsset {
    bytes: MSGPACK,
    content_type: "application/msgpack",
    content_encoding: None,
};
pub const MSGPACK_GZIP: StaticAsset = StaticAsset {
    bytes: MSGPACK_GZ,
    content_type: "application/msgpack",
    content_encoding: Some("gzip"),
};
pub const MSGPACK_ZSTD: StaticAsset = StaticAsset {
    bytes: MSGPACK_ZST,
    content_type: "application/msgpack",
    content_encoding: Some("zstd"),
};

/// Looks an asset up by its STOMP destination name (e.g. `/topic/compressed`).
pub fn lookup_by_topic(dest: &str) -> Option<StaticAsset> {
    Some(match dest {
        "/topic/compressed" => JSON_GZIP,
        "/topic/compressed-zstd" => JSON_ZSTD,
        "/topic/compressed-mp" => MSGPACK_PLAIN,
        "/topic/compressed-mp-gzip" => MSGPACK_GZIP,
        "/topic/compressed-mp-zstd" => MSGPACK_ZSTD,
        _ => return None,
    })
}
