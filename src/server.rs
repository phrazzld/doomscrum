use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as UrlPath, Query, State};
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
use crate::providers::{
    compare_render_freshness, fake::FakeProvider, fal::FalProvider, load_renders, Provider,
    VideoRender,
};
use crate::secrets;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const VIBE_RATINGS: &[&str] = &["cursed", "brainrot", "solid", "corporate"];

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
        base.join("repos").join(format!(
            "{}-{}",
            crate::util::slug(&name),
            crate::util::short(&crate::util::sha256_hex(s.as_bytes()))
        ))
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
        .route("/api/vibe", post(api_vibe))
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

/// Latest ready render for a spec.
fn latest_render(prd_id: &str, renders: &[VideoRender]) -> Option<VideoRender> {
    renders
        .iter()
        .filter(|r| r.prd_id == prd_id && r.status == "ready")
        .max_by(|a, b| compare_render_freshness(a, b))
        .cloned()
}

#[derive(Deserialize, Default)]
struct StateQuery {
    /// The viewport cursor: the feed index the user is currently on. Drives
    /// just-in-time prefetch of the specs ahead of it. Absent = a bare query
    /// (no viewport), so no prefetch and no spend — only a feed viewer sends it.
    cursor: Option<usize>,
}

/// Specs in the viewport window `[cursor, cursor + depth)` that still need a
/// render: no ready render yet (a revisit replays the cached render — no
/// second spend) and not already cooking (idempotent across the feed's poll).
/// Specs deeper than the window are never returned, so they cost nothing until
/// the cursor approaches them.
fn prefetch_window<'a>(
    prds: &'a [PrdSource],
    renders: &[VideoRender],
    cooking: &std::collections::HashMap<String, String>,
    cursor: usize,
    depth: usize,
) -> Vec<&'a PrdSource> {
    let start = cursor.min(prds.len());
    let end = cursor.saturating_add(depth).min(prds.len());
    prds[start..end]
        .iter()
        .filter(|prd| latest_render(&prd.id, renders).is_none())
        .filter(|prd| !cooking.contains_key(&prd.id))
        .collect()
}

/// How a windowed spec gets rendered. The wallet gate refuses over-cap real
/// renders, but the feed must never go dark — so an over-budget spec degrades
/// to a free fixture badged with the reason instead of failing the request.
#[derive(Debug, PartialEq)]
enum RenderPlan {
    Real { cost: f64 },
    DegradedFake,
    Fake,
    Skip,
}

/// Decide how to render one window spec. `fal` over the lifetime or daily cap
/// degrades to a badged fixture (oracle: the feed survives an exhausted wallet);
/// `fal` within budget renders for real when a key is present, else is left for
/// an explicit generate; a free provider just renders.
fn render_plan(
    provider: &str,
    fal_key_present: bool,
    cost: f64,
    spent_total: f64,
    spent_today: f64,
    cap_total: f64,
    cap_daily: f64,
) -> RenderPlan {
    if provider != "fal" {
        return RenderPlan::Fake;
    }
    if spent_total + cost > cap_total || spent_today + cost > cap_daily {
        RenderPlan::DegradedFake
    } else if fal_key_present {
        RenderPlan::Real { cost }
    } else {
        RenderPlan::Skip
    }
}

/// Run one render detached: a page refresh or a fast feed poll must never abort
/// a job that may cost money. Updates `cooking` on completion, tags a degraded
/// substitute so the feed can badge it, and releases the reservation. The caller
/// marks `cooking` before spawning so the job is visible on the next poll.
fn spawn_render_job(
    ctx: &AppCtx,
    prd: PrdSource,
    provider: String,
    reservation_id: Option<String>,
    degraded_reason: Option<String>,
) {
    let bg = ctx.clone();
    tokio::spawn(async move {
        let outcome = render_one(&bg, &provider, &prd).await;
        {
            let mut map = bg.cooking.lock().expect("cooking lock");
            match &outcome {
                Ok(_) => {
                    map.remove(&prd.id);
                }
                Err(err) => {
                    map.insert(prd.id.clone(), format!("failed: {err:#}"));
                }
            }
        }
        // Tag a degraded substitute so the feed badges it (overwrites the render
        // JSON the provider just wrote at the same path).
        if let (Ok(render), Some(reason)) = (&outcome, &degraded_reason) {
            let mut tagged = render.clone();
            tagged.degraded_reason = Some(reason.clone());
            let _ = crate::providers::save_render(&bg.renders_dir(), &tagged);
        }
        bg.release_render_reservation(reservation_id.as_deref())
            .await;
    });
}

/// Just-in-time render the viewport window: keep the next `prefetch_depth` specs
/// warm as the cursor advances, under the same wallet caps as `/api/generate`.
/// Fire-and-forget — renders run detached so serving the feed never blocks on
/// generation, and `cooking` makes re-polls idempotent.
async fn maybe_prefetch(ctx: &AppCtx, prds: &[PrdSource], renders: &[VideoRender], cursor: usize) {
    let depth = ctx.cfg.feed.prefetch_depth;
    if depth == 0 {
        return;
    }
    let provider = ctx.cfg.video.provider.clone();
    let fal_key = ctx.fal_key().is_some();
    let now = Utc::now();
    let cap_total = ctx.cfg.video.max_total_spend_usd;
    let cap_daily = ctx.cfg.video.max_daily_spend_usd;

    let mut reservations = ctx.render_reservations.lock().await;
    let mut spent_total = total_spend(renders) + pending_total_spend(&reservations);
    let mut spent_today = daily_spend(renders, now) + pending_daily_spend(&reservations, now);
    let mut cooking = ctx.cooking.lock().expect("cooking lock");

    // (spec, render provider, reservation id, degraded badge) — decided and
    // reserved synchronously, then spawned after the locks drop.
    let mut jobs: Vec<(PrdSource, &'static str, Option<String>, Option<String>)> = Vec::new();
    for prd in prefetch_window(prds, renders, &cooking, cursor, depth) {
        let cost = crate::providers::fal::unit_cost(&ctx.cfg.video.with_pipeline(&prd.sha256));
        match render_plan(
            &provider,
            fal_key,
            cost,
            spent_total,
            spent_today,
            cap_total,
            cap_daily,
        ) {
            RenderPlan::Real { cost } => {
                spent_total += cost;
                spent_today += cost;
                let id = crate::providers::cache_distinct_render_id(&prd.sha256);
                reservations.push(RenderReservation {
                    id: id.clone(),
                    amount_usd: cost,
                    created_at: now,
                });
                cooking.insert(prd.id.clone(), "cooking".into());
                jobs.push((prd.clone(), "fal", Some(id), None));
            }
            RenderPlan::DegradedFake => {
                cooking.insert(prd.id.clone(), "cooking".into());
                jobs.push((
                    prd.clone(),
                    "fake",
                    None,
                    Some("render budget exhausted".into()),
                ));
            }
            RenderPlan::Fake => {
                cooking.insert(prd.id.clone(), "cooking".into());
                jobs.push((prd.clone(), "fake", None, None));
            }
            RenderPlan::Skip => {}
        }
    }
    drop(cooking);
    drop(reservations);

    for (prd, pname, reservation_id, degraded) in jobs {
        spawn_render_job(ctx, prd, pname.to_string(), reservation_id, degraded);
    }
}

async fn api_state(State(ctx): State<AppCtx>, Query(q): Query<StateQuery>) -> Response {
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
            let vibe_rating = render
                .as_ref()
                .and_then(|render| latest_vibe_rating(&prd.id, &render.id, &events));
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
                "vibe_rating": vibe_rating,
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

    // Command/query separation: only a cursor-bearing request (a feed viewer)
    // prefetches its viewport. A bare /api/state query stays a read with no spend.
    if let Some(cursor) = q.cursor {
        maybe_prefetch(&ctx, &prds, &renders, cursor).await;
    }

    (
        [(header::CACHE_CONTROL, "no-store")],
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
        })),
    )
        .into_response()
}

fn latest_vibe_rating(prd_id: &str, render_id: &str, events: &[events::Event]) -> Option<String> {
    events
        .iter()
        .rev()
        .find(|event| {
            event.kind == "vibe_rating"
                && event.prd_id == prd_id
                && event.render_id.as_deref() == Some(render_id)
        })
        .and_then(|event| event.rating.clone())
}

#[derive(Deserialize)]
struct VibeBody {
    prd_id: String,
    render_id: String,
    rating: String,
}

async fn api_vibe(State(ctx): State<AppCtx>, Json(body): Json<VibeBody>) -> Response {
    if !VIBE_RATINGS.contains(&body.rating.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!(
                    "unknown vibe rating '{}' (expected one of: {})",
                    body.rating,
                    VIBE_RATINGS.join(", ")
                ),
                "ratings": VIBE_RATINGS,
            })),
        )
            .into_response();
    }
    let prds = match ctx.scan() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some(prd) = prds.into_iter().find(|p| p.id == body.prd_id) else {
        return error_response(StatusCode::NOT_FOUND, "spec not found");
    };
    let renders = load_renders(&ctx.renders_dir()).unwrap_or_default();
    let Some(render) = renders
        .iter()
        .find(|r| r.prd_id == prd.id && r.id == body.render_id && r.status == "ready")
    else {
        return error_response(StatusCode::NOT_FOUND, "ready render not found for spec");
    };
    match events::append_rating(
        &ctx.events_path(),
        &prd.id,
        &prd.sha256,
        &render.id,
        &body.rating,
    ) {
        Ok(event) => Json(json!({ "event": event, "rating": body.rating })).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
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
        // Atomic claim against the LIVE cooking map: a concurrent prefetch may
        // have started this spec since active_cooking was sampled above. Whoever
        // inserts first wins; the loser drops its reservation and dedupes, so a
        // spec is never double-submitted to the paid provider.
        let already_cooking = {
            let mut cooking = ctx.cooking.lock().expect("cooking lock");
            let present = cooking.contains_key(&prd.id);
            if !present {
                cooking.insert(prd.id.clone(), "cooking".into());
            }
            present
        };
        if already_cooking {
            ctx.release_render_reservation(render_reservation_id.as_deref())
                .await;
            return Json(json!({ "started": true, "deduped": true })).into_response();
        }
        spawn_render_job(
            &ctx,
            prd,
            provider_name.clone(),
            render_reservation_id.clone(),
            None,
        );
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
                );
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
    let tail = log_tail(&raw, 14);
    let failing_stage = receipt
        .stages
        .iter()
        .rev()
        .find(|s| !s.ok)
        .map(|s| s.name.clone());
    Json(json!({
        "id": receipt.id,
        "status": receipt.status,
        // `note` and the log file are already redacted at the persistence/write
        // boundary (see dispatch::redacted_receipt and run_cmd), so receipts
        // loaded here carry no raw secrets.
        "note": receipt.note,
        "failing_stage": failing_stage,
        "tail": tail,
    }))
    .into_response()
}

/// Last `n` lines of an agent log, secrets masked. The write path already
/// scrubs, so redacting here is defense in depth: a log written before this
/// shipped — or any future non-local `/log` bind — still cannot serve a key.
fn log_tail(raw: &str, n: usize) -> Vec<String> {
    let redacted = secrets::redact_env(raw);
    let mut lines: Vec<String> = redacted.lines().rev().take(n).map(str::to_string).collect();
    lines.reverse();
    lines
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
    use super::{
        latest_render, log_tail, parse_byte_range, prefetch_window, render_plan, RenderPlan,
    };
    use crate::backlog::PrdSource;
    use crate::providers::VideoRender;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn prd(id: &str) -> PrdSource {
        PrdSource {
            id: id.into(),
            sha256: format!("sha-{id}"),
            rel_path: format!("backlog.d/{id}.md"),
            abs_path: PathBuf::new(),
            title: format!("Spec {id}"),
            priority: 0,
            raw: String::new(),
        }
    }

    fn ready_render_for(prd_id: &str) -> VideoRender {
        let mut r = render("rid", "fake-local", "ready", "2026-01-01T00:00:00Z");
        r.prd_id = prd_id.into();
        r
    }

    fn render(id: &str, provider: &str, status: &str, created_at: &str) -> VideoRender {
        let asset_file = format!("{id}.mp4");
        VideoRender {
            id: id.into(),
            prd_id: "prd-1".into(),
            prd_sha256: "sha-1".into(),
            storyboard_id: format!("{id}-storyboard"),
            provider: provider.into(),
            model: "test-model".into(),
            native_audio: true,
            status: status.into(),
            asset_url: format!("/media/sha-1/{asset_file}"),
            asset_file,
            caption_artifact_file: None,
            degraded_reason: None,
            provider_job_id: Some(format!("{id}-job")),
            cost_estimate_usd: 0.0,
            latency_ms: 1,
            created_at: created_at.into(),
        }
    }

    #[test]
    fn prefetch_window_covers_only_depth_specs_ahead_of_cursor() {
        let prds = vec![prd("a"), prd("b"), prd("c"), prd("d"), prd("e")];
        let renders: Vec<VideoRender> = vec![];
        let cooking = HashMap::new();
        // cursor 0, depth 3 -> the top three; d and e (deeper) cost nothing.
        let win: Vec<&str> = prefetch_window(&prds, &renders, &cooking, 0, 3)
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(win, vec!["a", "b", "c"]);
        // cursor advances -> the window slides, bringing d and e into view.
        let win2: Vec<&str> = prefetch_window(&prds, &renders, &cooking, 2, 3)
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(win2, vec!["c", "d", "e"]);
    }

    #[test]
    fn prefetch_window_skips_already_rendered_and_cooking_specs() {
        let prds = vec![prd("a"), prd("b"), prd("c")];
        let renders = vec![ready_render_for("a")]; // a is cached — revisit, no re-spend
        let mut cooking = HashMap::new();
        cooking.insert("b".to_string(), "cooking".to_string()); // b is in flight
        let win: Vec<&str> = prefetch_window(&prds, &renders, &cooking, 0, 3)
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(win, vec!["c"]); // only c still needs a render
    }

    #[test]
    fn prefetch_window_is_empty_for_zero_depth_or_cursor_past_end() {
        let prds = vec![prd("a"), prd("b")];
        let renders: Vec<VideoRender> = vec![];
        let cooking = HashMap::new();
        assert!(prefetch_window(&prds, &renders, &cooking, 0, 0).is_empty());
        assert!(prefetch_window(&prds, &renders, &cooking, 9, 3).is_empty());
    }

    #[test]
    fn render_plan_degrades_over_budget_and_reals_within_it() {
        // fal over the lifetime cap -> badged fixture instead of failing the feed.
        assert_eq!(
            render_plan("fal", true, 1.0, 24.5, 0.0, 25.0, 5.0),
            RenderPlan::DegradedFake
        );
        // fal over the daily cap -> degrade too.
        assert_eq!(
            render_plan("fal", true, 1.0, 0.0, 4.5, 25.0, 5.0),
            RenderPlan::DegradedFake
        );
        // fal within budget + key -> real render, cost carried for the reservation.
        assert_eq!(
            render_plan("fal", true, 1.0, 0.0, 0.0, 25.0, 5.0),
            RenderPlan::Real { cost: 1.0 }
        );
        // fal within budget but no key -> leave it for an explicit generate.
        assert_eq!(
            render_plan("fal", false, 1.0, 0.0, 0.0, 25.0, 5.0),
            RenderPlan::Skip
        );
        // exactly at the cap still renders for real (strict > means == is in budget).
        assert_eq!(
            render_plan("fal", true, 0.5, 24.5, 0.0, 25.0, 5.0),
            RenderPlan::Real { cost: 0.5 }
        );
        // a free provider always renders, never wallet-gated.
        assert_eq!(
            render_plan("fake", false, 0.0, 0.0, 0.0, 25.0, 5.0),
            RenderPlan::Fake
        );
    }

    #[test]
    fn log_tail_masks_secrets_and_keeps_last_lines() {
        let raw = "line one\nagent printed sk-or-v1-DEADBEEF12345678\nlast line";
        let tail = log_tail(raw, 14);
        assert_eq!(tail.len(), 3);
        assert!(
            tail.iter()
                .all(|l| !l.contains("sk-or-v1-DEADBEEF12345678")),
            "{tail:?}"
        );
        assert!(tail.iter().any(|l| l.contains("[REDACTED]")), "{tail:?}");
        assert_eq!(tail[0], "line one");
        assert_eq!(tail[2], "last line");
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

    #[test]
    fn latest_render_selects_newest_ready_even_when_fixture_beats_real_provider() {
        let renders = vec![
            render("old-real", "fal", "ready", "2026-01-01T00:00:00.000Z"),
            render(
                "new-fixture",
                "fake-local",
                "ready",
                "2026-01-01T00:00:01.000Z",
            ),
        ];

        let selected = latest_render("prd-1", &renders).unwrap();

        assert_eq!(selected.id, "new-fixture");
        assert_eq!(selected.provider, "fake-local");
    }

    #[test]
    fn latest_render_ignores_failed_renders_and_ties_by_render_id() {
        let renders = vec![
            render("000-old", "fal", "ready", "2026-01-01T00:00:00.000Z"),
            render("999-new", "fal", "ready", "2026-01-01T00:00:00.000Z"),
            render("latest-failed", "fal", "failed", "2026-01-01T00:00:01.000Z"),
        ];

        let selected = latest_render("prd-1", &renders).unwrap();

        assert_eq!(selected.id, "999-new");
    }
}
