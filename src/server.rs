use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::backlog::{self, PrdSource};
use crate::config::Config;
use crate::dispatch::{load_receipts, DispatchKind, Dispatcher};
use crate::distill::{compile_storyboard, distill};
use crate::events;
use crate::providers::{fake::FakeProvider, fal::FalProvider, load_renders, Provider, VideoRender};
use crate::secrets;

const INDEX_HTML: &str = include_str!("../assets/index.html");

#[derive(Clone)]
pub struct AppCtx {
    pub cfg: Config,
    /// Project root (where specifi.toml lives).
    pub root: PathBuf,
    pub dispatcher: Arc<Dispatcher>,
}

impl AppCtx {
    pub fn new(root: PathBuf, cfg: Config) -> Self {
        let repo = root.join(&cfg.repo.path);
        let state_dir = root.join(&cfg.repo.state_dir);
        let dispatcher = Arc::new(Dispatcher {
            repo,
            dispatches_dir: state_dir.join("dispatches"),
            worktrees_dir: state_dir.join("worktrees"),
            agent: cfg.agent.clone(),
        });
        Self {
            cfg,
            root,
            dispatcher,
        }
    }

    pub fn repo(&self) -> PathBuf {
        self.root.join(&self.cfg.repo.path)
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join(&self.cfg.repo.state_dir)
    }

    pub fn renders_dir(&self) -> PathBuf {
        self.state_dir().join("renders")
    }

    pub fn events_path(&self) -> PathBuf {
        self.state_dir().join("events.ndjson")
    }

    pub fn scan(&self) -> anyhow::Result<Vec<PrdSource>> {
        backlog::scan(
            &self.repo(),
            &self.cfg.repo.backlog_dir,
            self.cfg.feed.max_items,
        )
    }

    fn fal_key(&self) -> Option<String> {
        secrets::get(&["FAL_API_KEY", "FAL_KEY"])
    }

    pub fn provider(&self, name: &str) -> anyhow::Result<Provider> {
        match name {
            "fake" => Ok(Provider::Fake(FakeProvider)),
            "fal" => {
                let key = self.fal_key().ok_or_else(|| {
                    anyhow::anyhow!("FAL_API_KEY or FAL_KEY not configured (env or ~/.secrets)")
                })?;
                Ok(Provider::Fal(FalProvider::from_config(
                    &self.cfg.video,
                    key,
                )))
            }
            other => anyhow::bail!("unknown video provider '{other}' (expected fake|fal)"),
        }
    }
}

pub fn router(ctx: AppCtx) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(api_state))
        .route("/api/generate", post(api_generate))
        .route("/api/swipe", post(api_swipe))
        .route("/api/spec/{prd_id}", get(api_spec))
        .route("/api/dispatches", get(api_dispatches))
        .route("/media/{sha}/{file}", get(media))
        .with_state(ctx)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

fn error_response(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

/// Latest render for a spec, preferring real provider output over fixtures.
fn latest_render(prd_id: &str, renders: &[VideoRender]) -> Option<VideoRender> {
    let ready: Vec<&VideoRender> = renders
        .iter()
        .filter(|r| r.prd_id == prd_id && r.status == "ready")
        .collect();
    ready
        .iter()
        .find(|r| r.provider != "fake-local")
        .or_else(|| ready.first())
        .map(|r| (*r).clone())
}

async fn api_state(State(ctx): State<AppCtx>) -> Response {
    let prds = match ctx.scan() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let renders = load_renders(&ctx.renders_dir()).unwrap_or_default();
    let receipts = load_receipts(&ctx.dispatcher.dispatches_dir).unwrap_or_default();
    let events = events::read_all(&ctx.events_path()).unwrap_or_default();

    let items: Vec<Value> = prds
        .iter()
        .map(|prd| {
            let render = latest_render(&prd.id, &renders);
            let dispatch = receipts.iter().find(|r| r.prd_id == prd.id);
            let skipped = events
                .iter()
                .rfind(|e| e.prd_id == prd.id)
                .is_some_and(|e| e.kind == "skip");
            let status = match (dispatch, skipped, &render) {
                (Some(d), _, _) => d.status.clone(),
                (None, true, _) => "skipped".into(),
                (None, false, Some(_)) => "rendered".into(),
                (None, false, None) => "new".into(),
            };
            json!({
                "prd": {
                    "id": prd.id,
                    "sha256": prd.sha256,
                    "title": prd.title,
                    "path": prd.rel_path,
                    "priority": prd.priority,
                },
                "render": render,
                "dispatch": dispatch.map(|d| json!({
                    "id": d.id,
                    "kind": d.kind,
                    "status": d.status,
                    "branch": d.branch,
                    "pr_url": d.pr_url,
                    "note": d.note,
                })),
                "status": status,
            })
        })
        .collect();

    Json(json!({
        "items": items,
        "video_provider": ctx.cfg.video.provider,
        "fal_configured": ctx.fal_key().is_some(),
        "max_items": ctx.cfg.feed.max_items,
    }))
    .into_response()
}

#[derive(Deserialize, Default)]
struct GenerateBody {
    provider: Option<String>,
    prd_id: Option<String>,
    force: Option<bool>,
}

async fn api_generate(State(ctx): State<AppCtx>, body: Option<Json<GenerateBody>>) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let provider_name = body
        .provider
        .unwrap_or_else(|| ctx.cfg.video.provider.clone());
    let provider = match ctx.provider(&provider_name) {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, err),
    };
    let prds = match ctx.scan() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let existing = load_renders(&ctx.renders_dir()).unwrap_or_default();
    let force = body.force.unwrap_or(false);

    let mut rendered = Vec::new();
    for prd in prds {
        if let Some(id) = &body.prd_id {
            if &prd.id != id {
                continue;
            }
        }
        let has_render = existing
            .iter()
            .any(|r| r.prd_id == prd.id && r.provider == provider.name());
        if has_render && !force {
            continue;
        }
        let storyboard = compile_storyboard(&prd, &distill(&prd), ctx.cfg.video.max_duration_sec);
        let storyboards_dir = ctx.state_dir().join("storyboards");
        let _ = std::fs::create_dir_all(&storyboards_dir);
        let _ = std::fs::write(
            storyboards_dir.join(format!("{}.json", prd.sha256)),
            serde_json::to_string_pretty(&storyboard).unwrap_or_default(),
        );
        match provider.render(&storyboard, &ctx.renders_dir()).await {
            Ok(render) => {
                let _ = events::append(
                    &ctx.events_path(),
                    &prd.id,
                    &prd.sha256,
                    "rendered",
                    Some(format!("{}/{}", render.provider, render.model)),
                );
                rendered.push(render);
            }
            Err(err) => {
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!("render failed for '{}': {err:#}", prd.title),
                )
            }
        }
    }
    Json(json!({ "renders": rendered })).into_response()
}

#[derive(Deserialize)]
struct SwipeBody {
    prd_id: String,
    /// "implement" (right) | "shape" (left) | "skip" (up)
    action: String,
}

async fn api_swipe(State(ctx): State<AppCtx>, Json(body): Json<SwipeBody>) -> Response {
    let prds = match ctx.scan() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some(prd) = prds.into_iter().find(|p| p.id == body.prd_id) else {
        return error_response(StatusCode::NOT_FOUND, "spec not found");
    };

    let kind = match body.action.as_str() {
        "implement" => Some(DispatchKind::Implement),
        "shape" => Some(DispatchKind::Shape),
        "skip" => None,
        other => {
            return error_response(
                StatusCode::BAD_REQUEST,
                format!("unknown action '{other}' (expected implement|shape|skip)"),
            )
        }
    };

    match kind {
        None => match events::append(&ctx.events_path(), &prd.id, &prd.sha256, "skip", None) {
            Ok(event) => Json(json!({ "event": event })).into_response(),
            Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Some(kind) => {
            let receipt = match ctx.dispatcher.create(&prd, kind) {
                Ok(r) => r,
                Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
            };
            let event_kind = match kind {
                DispatchKind::Implement => "dispatch_implement",
                DispatchKind::Shape => "dispatch_shape",
            };
            let _ = events::append(
                &ctx.events_path(),
                &prd.id,
                &prd.sha256,
                event_kind,
                Some(format!("dispatch {}", receipt.id)),
            );
            let dispatcher = ctx.dispatcher.clone();
            let queued = receipt.clone();
            tokio::spawn(async move {
                dispatcher.run(queued, prd).await;
            });
            Json(json!({ "dispatch": receipt })).into_response()
        }
    }
}

async fn api_spec(State(ctx): State<AppCtx>, UrlPath(prd_id): UrlPath<String>) -> Response {
    match ctx.scan() {
        Ok(prds) => match prds.into_iter().find(|p| p.id == prd_id) {
            Some(prd) => Json(json!({
                "id": prd.id,
                "sha256": prd.sha256,
                "path": prd.rel_path,
                "title": prd.title,
                "raw": prd.raw,
            }))
            .into_response(),
            None => error_response(StatusCode::NOT_FOUND, "spec not found"),
        },
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

async fn api_dispatches(State(ctx): State<AppCtx>) -> Response {
    match load_receipts(&ctx.dispatcher.dispatches_dir) {
        Ok(receipts) => Json(json!({ "dispatches": receipts })).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

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

/// Serve render MP4s with HTTP Range support — browsers' media stacks
/// require 206 responses to start playback and to seek/loop.
async fn media(
    State(ctx): State<AppCtx>,
    UrlPath((sha, file)): UrlPath<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Response {
    let safe = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    if !safe(&sha) || !safe(&file) || !file.ends_with(".mp4") || file.contains("..") {
        return error_response(StatusCode::FORBIDDEN, "forbidden");
    }
    let path = ctx.renders_dir().join(&sha).join(&file);
    let Ok(bytes) = std::fs::read(&path) else {
        return error_response(StatusCode::NOT_FOUND, "no such render");
    };
    let len = bytes.len() as u64;
    let base_headers = [
        (header::CONTENT_TYPE, "video/mp4".to_string()),
        (header::ACCEPT_RANGES, "bytes".to_string()),
        (header::CACHE_CONTROL, "no-cache".to_string()),
    ];
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|v| parse_byte_range(v, len));
    match range {
        None => (StatusCode::OK, base_headers, bytes).into_response(),
        Some(Some((start, end))) => {
            let slice = bytes[start as usize..=end as usize].to_vec();
            (
                StatusCode::PARTIAL_CONTENT,
                base_headers,
                [(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))],
                slice,
            )
                .into_response()
        }
        Some(None) => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [(header::CONTENT_RANGE, format!("bytes */{len}"))],
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_byte_range;

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
