use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use tokio_util::io::ReaderStream;

use crate::backlog::{self, PrdSource};
use crate::config::Config;
use crate::dispatch::{load_receipts, DispatchKind, Dispatcher};
use crate::events;
use crate::providers::{fake::FakeProvider, fal::FalProvider, load_renders, Provider, VideoRender};
use crate::secrets;

const INDEX_HTML: &str = include_str!("../assets/index.html");

#[derive(Clone)]
pub struct AppCtx {
    pub cfg: Config,
    /// Project root (where doomscrum.toml lives).
    pub root: PathBuf,
    /// The currently synced repo — switchable at runtime via /api/repo.
    repo_sel: Arc<std::sync::RwLock<PathBuf>>,
    /// In-flight single-spec AI renders: prd_id -> "cooking" | "failed: …".
    /// UI-triggered renders run detached so a page refresh can't abort a
    /// paid job; the feed poll reads this map for progress/failure.
    cooking: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    /// Concurrency limiter for agent dispatches. Receipts are created before
    /// acquiring a permit so excess swipes are durable and visible as queued.
    dispatch_slots: Arc<Semaphore>,
    /// Serializes dedupe + receipt creation inside this server process.
    dispatch_create_lock: Arc<AsyncMutex<()>>,
    /// Paid render spend that has been approved and started but not yet
    /// persisted as render provenance.
    render_reservations: Arc<AsyncMutex<Vec<RenderReservation>>>,
}

#[derive(Clone)]
struct RenderReservation {
    id: String,
    amount_usd: f64,
    created_at: DateTime<Utc>,
}

impl AppCtx {
    pub fn new(root: PathBuf, cfg: Config) -> Self {
        let repo = root.join(&cfg.repo.path);
        let repo = repo.canonicalize().unwrap_or(repo);
        let slots = cfg.agent.max_concurrent_dispatches.max(1);
        Self {
            cfg,
            root,
            repo_sel: Arc::new(std::sync::RwLock::new(repo)),
            cooking: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            dispatch_slots: Arc::new(Semaphore::new(slots)),
            dispatch_create_lock: Arc::new(AsyncMutex::new(())),
            render_reservations: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    pub fn repo(&self) -> PathBuf {
        self.repo_sel.read().expect("repo lock").clone()
    }

    /// The repo named in doomscrum.toml — its state stays in the legacy
    /// flat layout so existing renders/dispatches survive.
    fn default_repo(&self) -> PathBuf {
        let repo = self.root.join(&self.cfg.repo.path);
        repo.canonicalize().unwrap_or(repo)
    }

    /// Per-repo state: the configured repo keeps `<root>/<state_dir>`;
    /// any other synced repo gets `<root>/<state_dir>/repos/<slug>-<hash>`
    /// so renders, events, and dispatches never bleed across repos.
    pub fn state_dir(&self) -> PathBuf {
        let base = self.root.join(&self.cfg.repo.state_dir);
        let current = self.repo();
        if current == self.default_repo() {
            return base;
        }
        let s = current.to_string_lossy();
        let name = current
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".into());
        base.join("repos")
            .join(format!("{}-{}", crate::util::slug(&name), crate::util::short(&crate::util::sha256_hex(s.as_bytes()))))
    }

    /// Switch the synced repo. Validates the path and records it in the
    /// recents file. The feed, renders, and dispatches all follow.
    pub fn set_repo(&self, path: &str) -> anyhow::Result<PathBuf> {
        let expanded = if let Some(rest) = path.strip_prefix("~/") {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
        } else {
            PathBuf::from(path)
        };
        let repo = expanded
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("no such directory {path:?}: {e}"))?;
        let backlog = repo.join(&self.cfg.repo.backlog_dir);
        anyhow::ensure!(
            backlog.is_dir(),
            "{} has no {}/ — not a syncable backlog repo",
            repo.display(),
            self.cfg.repo.backlog_dir
        );
        *self.repo_sel.write().expect("repo lock") = repo.clone();
        let _ = self.remember_repo(&repo);
        Ok(repo)
    }

    fn recents_path(&self) -> PathBuf {
        self.root.join(&self.cfg.repo.state_dir).join("repos.json")
    }

    pub fn recent_repos(&self) -> Vec<String> {
        std::fs::read_to_string(self.recents_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn remember_repo(&self, repo: &std::path::Path) -> anyhow::Result<()> {
        let mut recents = self.recent_repos();
        let entry = repo.to_string_lossy().to_string();
        recents.retain(|r| r != &entry);
        recents.insert(0, entry);
        recents.truncate(8);
        std::fs::create_dir_all(self.root.join(&self.cfg.repo.state_dir))?;
        std::fs::write(self.recents_path(), serde_json::to_string_pretty(&recents)?)?;
        Ok(())
    }

    /// Dispatcher for the currently synced repo.
    pub fn dispatcher(&self) -> Arc<Dispatcher> {
        let state_dir = self.state_dir();
        Arc::new(Dispatcher {
            repo: self.repo(),
            dispatches_dir: state_dir.join("dispatches"),
            worktrees_dir: state_dir.join("worktrees"),
            agent: self.cfg.agent.clone(),
        })
    }

    pub fn renders_dir(&self) -> PathBuf {
        self.state_dir().join("renders")
    }

    pub fn events_path(&self) -> PathBuf {
        self.state_dir().join("events.ndjson")
    }

    async fn release_render_reservation(&self, id: Option<&str>) {
        let Some(id) = id else {
            return;
        };
        let mut reservations = self.render_reservations.lock().await;
        reservations.retain(|r| r.id != id);
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
        self.provider_with(name, &self.cfg.video)
    }

    /// Build a provider for one specific pipeline (model + duration) —
    /// the render mix resolves a different `VideoConfig` per spec.
    pub fn provider_with(
        &self,
        name: &str,
        video: &crate::config::VideoConfig,
    ) -> anyhow::Result<Provider> {
        match name {
            "fake" => Ok(Provider::Fake(FakeProvider)),
            "fal" => {
                let key = self.fal_key().ok_or_else(|| {
                    anyhow::anyhow!("FAL_API_KEY or FAL_KEY not configured (env or ~/.secrets)")
                })?;
                Ok(Provider::Fal(FalProvider::from_config(video, key)))
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
        .route("/api/dispatch/{id}/log", get(api_dispatch_log))
        .route("/api/repo", get(api_repo_get).post(api_repo_set))
        .route("/media/{sha}/{file}", get(media))
        .with_state(ctx)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_repo_get(State(ctx): State<AppCtx>) -> Response {
    Json(json!({
        "current": ctx.repo().to_string_lossy(),
        "name": ctx.repo().file_name().map(|n| n.to_string_lossy().to_string()),
        "recents": ctx.recent_repos(),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct RepoBody {
    path: String,
}

async fn api_repo_set(State(ctx): State<AppCtx>, Json(body): Json<RepoBody>) -> Response {
    match ctx.set_repo(&body.path) {
        Ok(repo) => Json(json!({
            "current": repo.to_string_lossy(),
            "name": repo.file_name().map(|n| n.to_string_lossy().to_string()),
            "recents": ctx.recent_repos(),
        }))
        .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, format!("{err:#}")),
    }
}

fn error_response(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

/// Total estimated spend on real renders, summed from provenance on disk.
pub fn total_spend(renders: &[VideoRender]) -> f64 {
    let sum = renders
        .iter()
        .filter(|r| r.provider == "fal")
        .map(|r| r.cost_estimate_usd)
        .sum();
    clean_money(sum)
}

/// Spend on real renders whose provenance timestamp falls on the UTC date of
/// `now`. The reset boundary is UTC so it is stable across operator machines.
pub fn daily_spend(renders: &[VideoRender], now: DateTime<Utc>) -> f64 {
    let today = now.date_naive();
    let sum = renders
        .iter()
        .filter(|r| r.provider == "fal")
        .filter(|r| {
            DateTime::parse_from_rfc3339(&r.created_at)
                .map(|dt| dt.with_timezone(&Utc).date_naive() == today)
                .unwrap_or(false)
        })
        .map(|r| r.cost_estimate_usd)
        .sum();
    clean_money(sum)
}

fn clean_money(value: f64) -> f64 {
    if value.abs() < f64::EPSILON {
        0.0
    } else {
        value
    }
}

fn pending_total_spend(reservations: &[RenderReservation]) -> f64 {
    clean_money(reservations.iter().map(|r| r.amount_usd).sum())
}

fn pending_daily_spend(reservations: &[RenderReservation], now: DateTime<Utc>) -> f64 {
    let today = now.date_naive();
    clean_money(
        reservations
            .iter()
            .filter(|r| r.created_at.date_naive() == today)
            .map(|r| r.amount_usd)
            .sum(),
    )
}

pub fn next_daily_reset_at(now: DateTime<Utc>) -> String {
    let tomorrow = now
        .date_naive()
        .succ_opt()
        .unwrap_or_else(|| now.date_naive());
    let reset = tomorrow.and_hms_opt(0, 0, 0).unwrap();
    DateTime::<Utc>::from_naive_utc_and_offset(reset, Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn planned_fal_spend(video: &crate::config::VideoConfig, prds: &[PrdSource]) -> f64 {
    prds.iter()
        .map(|p| crate::providers::fal::unit_cost(&video.with_pipeline(&p.sha256)))
        .sum()
}

pub fn render_provider_id(provider_name: &str) -> anyhow::Result<&'static str> {
    match provider_name {
        "fake" => Ok("fake-local"),
        "fal" => Ok("fal"),
        other => anyhow::bail!("unknown video provider '{other}' (expected fake|fal)"),
    }
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
    let receipts = load_receipts(&ctx.dispatcher().dispatches_dir).unwrap_or_default();
    let events = events::read_all(&ctx.events_path()).unwrap_or_default();
    let now = Utc::now();
    let reservations = ctx.render_reservations.lock().await.clone();
    let pending_usd = pending_total_spend(&reservations);
    let pending_daily_usd = pending_daily_spend(&reservations, now);

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
        "cooking": *ctx.cooking.lock().expect("cooking lock"),
        "video_provider": ctx.cfg.video.provider,
        "fal_configured": ctx.fal_key().is_some(),
        "max_items": ctx.cfg.feed.max_items,
        "spend": {
            "total_usd": total_spend(&renders),
            "cap_usd": ctx.cfg.video.max_total_spend_usd,
            "pending_usd": pending_usd,
            "daily_usd": daily_spend(&renders, now),
            "daily_pending_usd": pending_daily_usd,
            "daily_cap_usd": ctx.cfg.video.max_daily_spend_usd,
            "daily_reset_at": next_daily_reset_at(now),
            "price_per_render_usd": crate::providers::fal::avg_unit_cost(&ctx.cfg.video),
        },
    }))
    .into_response()
}

#[derive(Deserialize, Default)]
struct GenerateBody {
    provider: Option<String>,
    prd_id: Option<String>,
    force: Option<bool>,
    confirmed_cost: Option<bool>,
}

async fn api_generate(State(ctx): State<AppCtx>, body: Option<Json<GenerateBody>>) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let provider_name = body
        .provider
        .unwrap_or_else(|| ctx.cfg.video.provider.clone());
    let render_provider = match render_provider_id(&provider_name) {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, err),
    };
    let prds = match ctx.scan() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let existing = load_renders(&ctx.renders_dir()).unwrap_or_default();
    let force = body.force.unwrap_or(false);

    let active_cooking: BTreeSet<String> = if provider_name == "fal" {
        ctx.cooking
            .lock()
            .expect("cooking lock")
            .iter()
            .filter(|(_, status)| status.as_str() == "cooking")
            .map(|(prd_id, _)| prd_id.clone())
            .collect()
    } else {
        BTreeSet::new()
    };
    if body
        .prd_id
        .as_ref()
        .is_some_and(|id| active_cooking.contains(id))
    {
        return Json(json!({ "started": true, "deduped": true })).into_response();
    }

    let targets: Vec<_> = prds
        .into_iter()
        .filter(|prd| body.prd_id.as_ref().is_none_or(|id| &prd.id == id))
        .filter(|prd| !active_cooking.contains(&prd.id))
        .filter(|prd| {
            force
                || !existing
                    .iter()
                    .any(|r| r.prd_id == prd.id && r.provider == render_provider)
        })
        .collect();

    let mut render_reservation_id: Option<String> = None;
    // Wallet guards: quote first, then refuse real generation that would blow
    // either the lifetime or daily cap. This runs before provider construction
    // so budget failures do not require a FAL key.
    if provider_name == "fal" {
        let spent = total_spend(&existing);
        let planned = planned_fal_spend(&ctx.cfg.video, &targets);
        if planned > 0.0 && body.confirmed_cost != Some(true) {
            let now = Utc::now();
            let reservations = ctx.render_reservations.lock().await.clone();
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": format!(
                        "confirm estimated real render cost: ${planned:.2} for {} render(s)",
                        targets.len()
                    ),
                    "requires_confirmation": true,
                    "planned_usd": planned,
                    "render_count": targets.len(),
                    "price_per_render_usd": crate::providers::fal::avg_unit_cost(&ctx.cfg.video),
                    "pending_usd": pending_total_spend(&reservations),
                    "daily_spent_usd": daily_spend(&existing, now),
                    "daily_pending_usd": pending_daily_spend(&reservations, now),
                    "daily_cap_usd": ctx.cfg.video.max_daily_spend_usd,
                    "daily_reset_at": next_daily_reset_at(now),
                })),
            )
                .into_response();
        }
        let now = Utc::now();
        let mut reservations = ctx.render_reservations.lock().await;
        let pending_total = pending_total_spend(&reservations);
        let pending_daily = pending_daily_spend(&reservations, now);
        let cap = ctx.cfg.video.max_total_spend_usd;
        if spent + pending_total + planned > cap {
            return error_response(
                StatusCode::PAYMENT_REQUIRED,
                format!(
                    "spend cap: ${spent:.2} already spent + ${pending_total:.2} pending + ${planned:.2} planned for {} render(s) \
                     exceeds max_total_spend_usd ${cap:.2} — raise it in doomscrum.toml [video]",
                    targets.len()
                ),
            );
        }
        let today = daily_spend(&existing, now);
        let daily_cap = ctx.cfg.video.max_daily_spend_usd;
        if today + pending_daily + planned > daily_cap {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": format!(
                        "daily render budget: ${today:.2} already spent today + ${pending_daily:.2} pending + ${planned:.2} planned for {} render(s) \
                         exceeds max_daily_spend_usd ${daily_cap:.2}",
                        targets.len()
                    ),
                    "daily_spent_usd": today,
                    "daily_pending_usd": pending_daily,
                    "planned_usd": planned,
                    "daily_cap_usd": daily_cap,
                    "reset_at": next_daily_reset_at(now),
                })),
            )
                .into_response();
        }
        if planned > 0.0 {
            let id = crate::util::sha256_hex(
                format!(
                    "{}:{planned}:{now}",
                    targets
                        .iter()
                        .map(|p| p.sha256.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .as_bytes(),
            );
            reservations.push(RenderReservation {
                id: id.clone(),
                amount_usd: planned,
                created_at: now,
            });
            render_reservation_id = Some(id);
        }
    }

    // Single-spec paid render (the card's "cook with AI" button): run
    // detached so a page refresh cannot abort a job that costs money. The
    // feed poll watches ctx.cooking for progress and failure.
    if body.prd_id.is_some() && provider_name == "fal" {
        let Some(prd) = targets.into_iter().next() else {
            return error_response(StatusCode::CONFLICT, "already rendered (use force)");
        };
        let id = prd.id.clone();
        ctx.cooking
            .lock()
            .expect("cooking lock")
            .insert(id.clone(), "cooking".into());
        let bg = ctx.clone();
        let pname = provider_name.clone();
        let reservation_id = render_reservation_id.clone();
        tokio::spawn(async move {
            let outcome = render_one(&bg, &pname, &prd).await;
            {
                let mut map = bg.cooking.lock().expect("cooking lock");
                match outcome {
                    Ok(_) => {
                        map.remove(&id);
                    }
                    Err(err) => {
                        map.insert(id, format!("failed: {err:#}"));
                    }
                }
            }
            bg.release_render_reservation(reservation_id.as_deref())
                .await;
        });
        return Json(json!({ "started": true })).into_response();
    }

    let mut rendered = Vec::new();
    for prd in targets {
        match render_one(&ctx, &provider_name, &prd).await {
            Ok(render) => rendered.push(render),
            Err(err) => {
                ctx.release_render_reservation(render_reservation_id.as_deref())
                    .await;
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    format!("render failed for '{}': {err:#}", prd.title),
                )
            }
        }
    }
    ctx.release_render_reservation(render_reservation_id.as_deref())
        .await;
    Json(json!({ "renders": rendered })).into_response()
}

/// Script + storyboard + render + event for one spec.
async fn render_one(
    ctx: &AppCtx,
    provider_name: &str,
    prd: &PrdSource,
) -> anyhow::Result<VideoRender> {
    let vcfg = ctx.cfg.video.with_pipeline(&prd.sha256);
    let provider = ctx.provider_with(provider_name, &vcfg)?;
    let script_key = crate::secrets::get(&["OPENROUTER_API_KEY"]);
    let storyboard = crate::scriptwriter::storyboard(
        &ctx.cfg.script,
        script_key.as_deref(),
        prd,
        provider.clip_duration(vcfg.max_duration_sec),
        &ctx.state_dir().join("scripts"),
        provider_name != "fake",
    )
    .await
    .map_err(|err| anyhow::anyhow!("scriptwriter failed: {err:#}"))?;
    let storyboards_dir = ctx.state_dir().join("storyboards");
    let _ = std::fs::create_dir_all(&storyboards_dir);
    let _ = std::fs::write(
        storyboards_dir.join(format!("{}.json", prd.sha256)),
        serde_json::to_string_pretty(&storyboard).unwrap_or_default(),
    );
    let render = provider.render(&storyboard, &ctx.renders_dir()).await?;
    let _ = events::append(
        &ctx.events_path(),
        &prd.id,
        &prd.sha256,
        "rendered",
        Some(format!("{}/{}", render.provider, render.model)),
    );
    Ok(render)
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
            let dispatcher = ctx.dispatcher();
            let receipt = {
                let _guard = ctx.dispatch_create_lock.lock().await;
                if let Ok(receipts) = load_receipts(&dispatcher.dispatches_dir) {
                    if let Some(existing) = receipts.into_iter().find(|r| {
                        r.prd_id == prd.id && r.kind == kind && active_dispatch_status(&r.status)
                    }) {
                        return Json(json!({ "dispatch": existing, "deduped": true }))
                            .into_response();
                    }
                }
                match dispatcher.create(&prd, kind) {
                    Ok(r) => r,
                    Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
                }
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
            let dispatcher = ctx.dispatcher();
            let slots = ctx.dispatch_slots.clone();
            let queued = receipt.clone();
            tokio::spawn(async move {
                let Ok(_permit) = slots.acquire_owned().await else {
                    return;
                };
                dispatcher.run(queued, prd).await;
            });
            Json(json!({ "dispatch": receipt })).into_response()
        }
    }
}

fn active_dispatch_status(status: &str) -> bool {
    matches!(status, "queued" | "agent_running" | "opening_pr")
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
    match load_receipts(&ctx.dispatcher().dispatches_dir) {
        Ok(receipts) => Json(json!({ "dispatches": receipts })).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// Tail of one dispatch's agent log — what the feed shows while an agent
/// is cooking and when it flops. Receipts persist after every stage, so
/// this is pure surfacing.
async fn api_dispatch_log(State(ctx): State<AppCtx>, UrlPath(id): UrlPath<String>) -> Response {
    let receipts = load_receipts(&ctx.dispatcher().dispatches_dir).unwrap_or_default();
    let Some(receipt) = receipts.into_iter().find(|r| r.id == id) else {
        return error_response(StatusCode::NOT_FOUND, "dispatch not found");
    };
    let raw = std::fs::read_to_string(&receipt.agent_log).unwrap_or_default();
    let tail: Vec<&str> = raw.lines().rev().take(14).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    let failing_stage = receipt
        .stages
        .iter()
        .rev()
        .find(|s| !s.ok)
        .map(|s| s.name.clone());
    Json(json!({
        "id": receipt.id,
        "status": receipt.status,
        "note": receipt.note,
        "failing_stage": failing_stage,
        "tail": tail,
    }))
    .into_response()
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
/// require 206 responses to start playback and to seek/loop. Stream from disk
/// so a range request never buffers the whole render.
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
                body_len,
                Some(format!("bytes {start}-{end}/{len}")),
                Body::from_stream(ReaderStream::new(file.take(body_len))),
            )
        }
        Some(None) => media_stream_response(
            StatusCode::RANGE_NOT_SATISFIABLE,
            0,
            Some(format!("bytes */{len}")),
            Body::empty(),
        ),
    }
}

fn media_stream_response(
    status: StatusCode,
    content_len: u64,
    content_range: Option<String>,
    body: Body,
) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "video/mp4")
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
