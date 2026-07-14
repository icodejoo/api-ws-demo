use axum::body::Bytes;
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::IntoResponse;

use crate::compressed_assets::{self, StaticAsset};

fn asset_response(asset: StaticAsset) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(asset.content_type),
    );
    if let Some(encoding) = asset.content_encoding {
        headers.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static(encoding),
        );
    }
    (headers, Bytes::from_static(asset.bytes))
}

pub async fn json_gzip() -> impl IntoResponse {
    asset_response(compressed_assets::JSON_GZIP)
}

pub async fn json_zstd() -> impl IntoResponse {
    asset_response(compressed_assets::JSON_ZSTD)
}

pub async fn msgpack_plain() -> impl IntoResponse {
    asset_response(compressed_assets::MSGPACK_PLAIN)
}

pub async fn msgpack_gzip() -> impl IntoResponse {
    asset_response(compressed_assets::MSGPACK_GZIP)
}

pub async fn msgpack_zstd() -> impl IntoResponse {
    asset_response(compressed_assets::MSGPACK_ZSTD)
}
