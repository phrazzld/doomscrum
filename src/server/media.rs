//! Render media streaming with content allowlisting and HTTP Range support.

use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{error_response, AppCtx};

/// Parse a `Range: bytes=start-end` header against a body of `len` bytes.
/// Returns the inclusive byte range to serve. Only single ranges supported.
fn parse_byte_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let range = match (start.trim(), end.trim()) {
        ("", suffix) => {
            // last N bytes
            let n: u64 = suffix.parse().ok()?;
            (len.saturating_sub(n.min(len)), len.saturating_sub(1))
        }
        (start, "") => (start.parse().ok()?, len.saturating_sub(1)),
        (start, end) => (
            start.parse().ok()?,
            end.parse::<u64>().ok()?.min(len.saturating_sub(1)),
        ),
    };
    (len > 0 && range.0 <= range.1 && range.0 < len).then_some(range)
}

/// What the media route may serve out of `renders/{sha}/`, by filename.
/// Render MP4s and their caption-artifact sidecars only — provenance JSON
/// and anything else stay unreachable.
fn media_content_type(file: &str) -> Option<&'static str> {
    if file.ends_with(".captions.json") {
        Some("application/json")
    } else if file.ends_with(".mp4") {
        Some("video/mp4")
    } else {
        None
    }
}

/// Serve render MP4s (and caption-artifact sidecars) with HTTP Range support
/// — browsers' media stacks require 206 responses to start playback and to
/// seek/loop. Stream from disk so a range request never buffers the whole
/// render.
pub(super) async fn media(
    State(ctx): State<AppCtx>,
    UrlPath((sha, file)): UrlPath<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Response {
    let safe = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    let content_type = media_content_type(&file);
    if !safe(&sha) || !safe(&file) || content_type.is_none() || file.contains("..") {
        return error_response(StatusCode::FORBIDDEN, "forbidden");
    }
    let content_type = content_type.expect("checked above");
    let path = ctx.renders_dir().join(&sha).join(&file);
    let Ok(metadata) = tokio::fs::metadata(&path).await else {
        return error_response(StatusCode::NOT_FOUND, "no such render");
    };
    if !metadata.is_file() {
        return error_response(StatusCode::NOT_FOUND, "no such render");
    }
    let len = metadata.len();
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|v| parse_byte_range(v, len));
    match range {
        None => {
            let Ok(file) = tokio::fs::File::open(&path).await else {
                return error_response(StatusCode::NOT_FOUND, "no such render");
            };
            media_stream_response(
                StatusCode::OK,
                content_type,
                len,
                None,
                Body::from_stream(ReaderStream::new(file)),
            )
        }
        Some(Some((start, end))) => {
            let Ok(mut file) = tokio::fs::File::open(&path).await else {
                return error_response(StatusCode::NOT_FOUND, "no such render");
            };
            if let Err(err) = file.seek(std::io::SeekFrom::Start(start)).await {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, err);
            }
            let body_len = end - start + 1;
            media_stream_response(
                StatusCode::PARTIAL_CONTENT,
                content_type,
                body_len,
                Some(format!("bytes {start}-{end}/{len}")),
                Body::from_stream(ReaderStream::new(file.take(body_len))),
            )
        }
        Some(None) => media_stream_response(
            StatusCode::RANGE_NOT_SATISFIABLE,
            content_type,
            0,
            Some(format!("bytes */{len}")),
            Body::empty(),
        ),
    }
}

fn media_stream_response(
    status: StatusCode,
    content_type: &'static str,
    content_len: u64,
    content_range: Option<String>,
    body: Body,
) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONTENT_LENGTH, content_len.to_string());
    if let Some(content_range) = content_range {
        builder = builder.header(header::CONTENT_RANGE, content_range);
    }
    builder.body(body).unwrap_or_else(|err| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("building media response: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{media_content_type, parse_byte_range};

    #[test]
    fn media_route_serves_mp4s_and_caption_sidecars_only() {
        assert_eq!(media_content_type("render-1.mp4"), Some("video/mp4"));
        assert_eq!(
            media_content_type("render-1.captions.json"),
            Some("application/json")
        );
        // Render provenance JSON must stay unreachable over HTTP.
        assert_eq!(media_content_type("render-1.json"), None);
        assert_eq!(media_content_type("captions.json"), None);
        assert_eq!(media_content_type("secrets.env"), None);
    }

    #[test]
    fn byte_ranges_cover_browser_patterns() {
        assert_eq!(parse_byte_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_byte_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_byte_range("bytes=90-200", 100), Some((90, 99)));
        assert_eq!(parse_byte_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_byte_range("bytes=100-", 100), None);
        assert_eq!(parse_byte_range("bytes=5-2", 100), None);
        assert_eq!(parse_byte_range("garbage", 100), None);
        assert_eq!(parse_byte_range("bytes=0-", 0), None);
    }
}
