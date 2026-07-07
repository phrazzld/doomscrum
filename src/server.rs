use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
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
use crate::render::ledger::{self, CostEntry};
use crate::render::pipeline::render_spec;
use crate::render::wallet::{
    self, cap_breach, pending_daily_spend, pending_total_spend, render_plan, CapBreach, RenderPlan,
    Wallet,
};
use crate::secrets;

// Re-exported so `server::total_spend` & friends stay stable for `main.rs` and
// the test suite; the implementations now live in `render::wallet`.
pub use crate::render::wallet::{daily_spend, next_daily_reset_at, planned_fal_spend, total_spend};

const INDEX_HTML: &str = include_str!("../assets/index.html");
const MANIFEST: &str = include_str!("../assets/manifest.webmanifest");
const ICON_192: &[u8] = include_bytes!("../assets/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../assets/icon-512.png");
const VIBE_RATINGS: &[&str] = &["cursed", "brainrot", "solid", "corporate"];

/// One spec's in-flight render state in the `cooking` map. The feed API
/// serializes only `label` (`"cooking"` | `"failed: …"`) — the retry
/// bookkeeping stays server-side.
#[derive(Clone)]
pub struct CookEntry {
    /// "cooking" while in flight; "failed: …" after a failure.
    pub label: String,
    /// Render starts for this spec so far — the bounded-retry budget.
    pub attempts: u32,
    /// When the last attempt failed (drives the retry backoff).
    pub failed_at: Option<std::time::Instant>,
}

impl CookEntry {
    fn cooking(attempts: u32) -> Self {
        Self {
            label: "cooking".into(),
            attempts,
            failed_at: None,
        }
    }

    fn is_cooking(&self) -> bool {
        self.label == "cooking"
    }
}

/// How a failed just-in-time render becomes eligible for another attempt.
#[derive(Clone, Copy)]
struct RetryPolicy {
    max_attempts: u32,
    backoff: std::time::Duration,
}

/// Why a windowed spec is being (re)rendered — decides how strict the wallet
/// treatment is in [`maybe_prefetch`].
#[derive(Debug, PartialEq, Eq)]
enum PrefetchReason {
    /// No render at all yet.
    Fresh,
    /// A degraded (budget-exhausted) fixture stands in; upgrade it to a real
    /// render — but ONLY when the budget actually allows one, otherwise the
    /// feed would re-render the same fixture on every poll.
    DegradedUpgrade,
    /// The previous attempt failed; retry within the bounded budget.
    FailedRetry { attempts: u32 },
}

#[derive(Clone)]
pub struct AppCtx {
    pub cfg: Config,
    /// Project root (where doomscrum.toml lives).
    pub root: PathBuf,
    /// The currently synced repo — switchable at runtime via /api/repo.
    repo_sel: Arc<std::sync::RwLock<PathBuf>>,
    /// In-flight single-spec AI renders: prd_id -> [`CookEntry`].
    /// UI-triggered renders run detached so a page refresh can't abort a
    /// paid job; the feed poll reads this map for progress/failure, and the
    /// prefetch loop reads the attempt count for bounded retry.
    cooking: Arc<std::sync::Mutex<std::collections::HashMap<String, CookEntry>>>,
    /// Concurrency limiter for agent dispatches. Receipts are created before
    /// acquiring a permit so excess swipes are durable and visible as queued.
    dispatch_slots: Arc<Semaphore>,
    /// Serializes dedupe + receipt creation inside this server process.
    dispatch_create_lock: Arc<AsyncMutex<()>>,
    /// Serializes a dispatch's queued→cancelled (undo) and queued→agent_running
    /// (start) transitions, shared into every `Dispatcher` so they can't race.
    dispatch_state_lock: Arc<std::sync::Mutex<()>>,
    /// In-flight paid-render reservations (reserve on approval, release on
    /// completion). Opaque handle; the lifecycle lives in `render::wallet`.
    wallet: Wallet,
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
            dispatch_state_lock: Arc::new(std::sync::Mutex::new(())),
            wallet: Wallet::new(),
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
            state_lock: self.dispatch_state_lock.clone(),
        })
    }

    pub fn renders_dir(&self) -> PathBuf {
        self.state_dir().join("renders")
    }

    pub fn events_path(&self) -> PathBuf {
        self.state_dir().join("events.ndjson")
    }

    /// The durable append-only cost ledger. Deliberately OUTSIDE the renders
    /// dir: wiping `.doomscrum/renders` must not reset the spend meter.
    pub fn ledger_path(&self) -> PathBuf {
        self.state_dir().join("costs.ndjson")
    }

    /// The complete paid-spend record every wallet gate reads: the durable
    /// ledger unioned with any render provenance it does not know about.
    pub fn spend_entries(&self, renders: &[VideoRender]) -> Vec<CostEntry> {
        let recorded = ledger::read_all(&self.ledger_path()).unwrap_or_default();
        ledger::spend_entries(recorded, renders)
    }

    /// Boot-time reconcile: rebuild runtime truth from disk instead of
    /// starting empty. A crash kills the detached tasks that owned in-flight
    /// dispatches, so their receipts would stay frozen (`agent_running` /
    /// `opening_pr` / `queued`) forever and GC would keep protecting their
    /// orphaned worktrees. Flip each to `failed` and append a durable
    /// `dispatch_failed` event. (In-flight renders need no adoption: paid
    /// provenance + the cost ledger are written atomically-enough at
    /// completion, and a crashed job's reservation dies with the process.)
    pub fn reconcile_on_boot(&self) -> anyhow::Result<Vec<crate::dispatch::DispatchReceipt>> {
        let reconciled = self.dispatcher().reconcile_stranded()?;
        for receipt in &reconciled {
            let _ = events::append(
                &self.events_path(),
                &receipt.prd_id,
                &receipt.prd_sha256,
                "dispatch_failed",
                Some(format!(
                    "dispatch {} stranded by restart — reconciled on boot",
                    receipt.id
                )),
            );
        }
        Ok(reconciled)
    }

    async fn release_render_reservation(&self, id: Option<&str>) {
        self.wallet.release(id).await;
    }

    pub fn scan(&self) -> anyhow::Result<Vec<PrdSource>> {
        backlog::scan(
            &self.repo(),
            &self.cfg.repo.backlog_dir,
            self.cfg.feed.max_items,
        )
    }

    pub fn scan_all(&self) -> anyhow::Result<Vec<PrdSource>> {
        backlog::scan(&self.repo(), &self.cfg.repo.backlog_dir, usize::MAX)
    }

    fn fal_key(&self) -> Option<String> {
        secrets::get(&["FAL_API_KEY", "FAL_KEY"]).or_else(|| self.stored_key("FAL_API_KEY"))
    }

    /// Keys entered through the in-app key sheet (`POST /api/keys`). Operator-
    /// level, so they live under the base state dir (like the recents file),
    /// not the per-repo state dir — a key follows the operator across repos.
    fn keys_path(&self) -> PathBuf {
        self.root.join(&self.cfg.repo.state_dir).join("keys.json")
    }

    /// A key previously stored via the in-app key sheet. Env and `~/.secrets`
    /// take precedence (see [`Self::fal_key`]); this is the zero-terminal path.
    pub fn stored_key(&self, name: &str) -> Option<String> {
        let raw = std::fs::read_to_string(self.keys_path()).ok()?;
        let map: std::collections::HashMap<String, String> = serde_json::from_str(&raw).ok()?;
        map.get(name).filter(|v| !v.trim().is_empty()).cloned()
    }

    /// Persist an in-app key. Owner-only file permissions; the value is never
    /// logged or echoed (route responses report only `*_configured` booleans).
    pub fn store_key(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let path = self.keys_path();
        std::fs::create_dir_all(path.parent().expect("keys path has a parent"))?;
        let mut map: std::collections::HashMap<String, String> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        map.insert(name.to_string(), value.to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
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
        .route("/manifest.webmanifest", get(manifest))
        .route("/icon-192.png", get(icon_192))
        .route("/icon-512.png", get(icon_512))
        .route("/api/state", get(api_state))
        .route("/api/generate", post(api_generate))
        .route("/api/vibe", post(api_vibe))
        .route("/api/swipe", post(api_swipe))
        .route("/api/spec/{prd_id}", get(api_spec))
        .route("/api/dispatches", get(api_dispatches))
        .route("/api/dispatch/{id}/log", get(api_dispatch_log))
        .route("/api/dispatch/{id}/cancel", post(api_dispatch_cancel))
        .route("/api/repo", get(api_repo_get).post(api_repo_set))
        .route("/api/keys", post(api_keys_set))
        .route("/api/egress", get(api_egress))
        .route("/media/{sha}/{file}", get(media))
        .with_state(ctx)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// PWA installability (backlog 015): manifest + icons make the feed
/// add-to-home-screen installable, so the couch/phone scenario is a real
/// app icon, not a browser tab.
async fn manifest() -> Response {
    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        MANIFEST,
    )
        .into_response()
}

async fn icon_192() -> Response {
    ([(header::CONTENT_TYPE, "image/png")], ICON_192).into_response()
}

async fn icon_512() -> Response {
    ([(header::CONTENT_TYPE, "image/png")], ICON_512).into_response()
}

/// `GET /api/egress` — runtime data-egress disclosure. Names exactly what
/// spec-derived text is sent to OpenRouter (scriptwriter) and fal.ai (render
/// prompt), with the source code path for each. The feed UI surfaces this in
/// its disclosure panel so the operator sees the enumeration, not just README
/// prose. (backlog 022, security lane.)
async fn api_egress() -> Response {
    Json(json!({
        "payloads": crate::egress::payloads(),
        "summary": crate::egress::summary(),
    }))
    .into_response()
}

async fn api_repo_get(State(ctx): State<AppCtx>) -> Response {
    Json(json!({
        "current": ctx.repo().to_string_lossy(),
        "name": ctx.repo().file_name().map(|n| n.to_string_lossy().to_string()),
        "recents": ctx.recent_repos(),
        // The UI's empty-backlog on-ramp names the exact spec path; it reads
        // the configured dir here instead of hardcoding "backlog.d".
        "backlog_dir": ctx.cfg.repo.backlog_dir,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct KeyBody {
    provider: String,
    key: String,
}

/// `POST /api/keys` — the in-app key sheet. Stores a provider key under the
/// state dir so enabling real renders never requires leaving the app. The
/// response quotes the per-render price and the starter budget the wallet
/// already enforces; the key value itself is never echoed back or logged.
async fn api_keys_set(State(ctx): State<AppCtx>, Json(body): Json<KeyBody>) -> Response {
    if body.provider != "fal" {
        return error_response(
            StatusCode::BAD_REQUEST,
            format!(
                "unknown key provider '{}' — only 'fal' has an in-app key surface",
                body.provider
            ),
        );
    }
    let key = body.key.trim();
    if key.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "empty key — paste your fal.ai API key (dashboard → keys)",
        );
    }
    if let Err(err) = ctx.store_key("FAL_API_KEY", key) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not store key: {err:#}"),
        );
    }
    Json(json!({
        "fal_configured": ctx.fal_key().is_some(),
        "price_per_render_usd": crate::providers::fal::avg_unit_cost(&ctx.cfg.video),
        "daily_cap_usd": ctx.cfg.video.max_daily_spend_usd,
        "cap_usd": ctx.cfg.video.max_total_spend_usd,
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

/// Specs in the viewport window `[cursor, cursor + depth)` that need a
/// render, each with its [`PrefetchReason`]:
/// - no render yet → `Fresh` (a ready revisit replays the cache — no second
///   spend);
/// - latest render is a budget-degraded fixture → `DegradedUpgrade` (the
///   caller only acts on it when the wallet allows a real render);
/// - last attempt failed → `FailedRetry` once the backoff has elapsed and the
///   attempt budget remains; an in-flight or attempt-exhausted spec is
///   skipped (idempotent across the feed's poll, and no retry storm).
///
/// Specs deeper than the window are never returned, so they cost nothing
/// until the cursor approaches them.
fn prefetch_window<'a>(
    prds: &'a [PrdSource],
    renders: &[VideoRender],
    cooking: &std::collections::HashMap<String, CookEntry>,
    cursor: usize,
    depth: usize,
    retry: RetryPolicy,
) -> Vec<(&'a PrdSource, PrefetchReason)> {
    let start = cursor.min(prds.len());
    let end = cursor.saturating_add(depth).min(prds.len());
    prds[start..end]
        .iter()
        .filter_map(|prd| {
            if let Some(entry) = cooking.get(&prd.id) {
                if entry.is_cooking() {
                    return None; // in flight — idempotent across polls
                }
                let backoff_elapsed = entry
                    .failed_at
                    .is_none_or(|at| at.elapsed() >= retry.backoff);
                if entry.attempts < retry.max_attempts && backoff_elapsed {
                    return Some((
                        prd,
                        PrefetchReason::FailedRetry {
                            attempts: entry.attempts,
                        },
                    ));
                }
                return None; // exhausted or still backing off — failure stays badged
            }
            match latest_render(&prd.id, renders) {
                None => Some((prd, PrefetchReason::Fresh)),
                Some(render) if render.degraded_reason.is_some() => {
                    Some((prd, PrefetchReason::DegradedUpgrade))
                }
                Some(_) => None, // cached real render — replay, no re-spend
            }
        })
        .collect()
}

/// Await a detached render task, converting a panic into a normal `Err` so
/// the caller's cleanup (cooking map, failure event, reservation release)
/// always runs — a panicked task must never silently pin a reservation.
async fn await_render_task(
    handle: tokio::task::JoinHandle<anyhow::Result<VideoRender>>,
) -> anyhow::Result<VideoRender> {
    match handle.await {
        Ok(outcome) => outcome,
        Err(join_err) => Err(anyhow::anyhow!("render task panicked: {join_err}")),
    }
}

/// Run one render detached: a page refresh or a fast feed poll must never abort
/// a job that may cost money. Updates `cooking` on completion, tags a degraded
/// substitute so the feed can badge it, appends a durable `render_failed`
/// event on failure (so failures survive restart instead of dying with the
/// in-memory map), and releases the reservation — even if the render task
/// panics. The caller marks `cooking` before spawning so the job is visible
/// on the next poll.
fn spawn_render_job(
    ctx: &AppCtx,
    prd: PrdSource,
    provider: String,
    reservation_id: Option<String>,
    degraded_reason: Option<String>,
) {
    let bg = ctx.clone();
    tokio::spawn(async move {
        // Inner spawn: a panic in the render pipeline surfaces as a JoinError
        // here instead of killing this supervisor, so cleanup always runs.
        let task = {
            let ctx = bg.clone();
            let provider = provider.clone();
            let prd = prd.clone();
            tokio::spawn(async move { render_spec(&ctx, &provider, &prd).await })
        };
        let outcome = await_render_task(task).await;
        {
            let mut map = bg.cooking.lock().expect("cooking lock");
            match &outcome {
                Ok(_) => {
                    map.remove(&prd.id);
                }
                Err(err) => {
                    let attempts = map.get(&prd.id).map(|e| e.attempts).unwrap_or(1);
                    map.insert(
                        prd.id.clone(),
                        CookEntry {
                            label: format!("failed: {err:#}"),
                            attempts,
                            failed_at: Some(std::time::Instant::now()),
                        },
                    );
                }
            }
        }
        if let Err(err) = &outcome {
            // Durable failure record: the in-memory map dies with the process,
            // the events ledger does not.
            let _ = events::append(
                &bg.events_path(),
                &prd.id,
                &prd.sha256,
                "render_failed",
                Some(format!("{err:#}")),
            );
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
    let retry = RetryPolicy {
        max_attempts: ctx.cfg.feed.render_max_attempts,
        backoff: std::time::Duration::from_secs(ctx.cfg.feed.render_retry_backoff_sec),
    };
    // Spend truth comes from the durable ledger (unioned with provenance),
    // never from surviving render JSONs alone.
    let entries = ctx.spend_entries(renders);

    let mut reservations = ctx.wallet.lock().await;
    let mut spent_total = total_spend(&entries) + pending_total_spend(&reservations);
    let mut spent_today = daily_spend(&entries, now) + pending_daily_spend(&reservations, now);
    let mut cooking = ctx.cooking.lock().expect("cooking lock");

    // (spec, render provider, reservation id, degraded badge) — decided and
    // reserved synchronously, then spawned after the locks drop. The attempt
    // count rides on the CookEntry; the failure path reads it back from there.
    let mut jobs: Vec<(PrdSource, &'static str, Option<String>, Option<String>)> = Vec::new();
    for (prd, reason) in prefetch_window(prds, renders, &cooking, cursor, depth, retry) {
        let cost = crate::providers::fal::unit_cost(&ctx.cfg.video.with_pipeline(&prd.sha256));
        let plan = render_plan(
            &provider,
            fal_key,
            cost,
            spent_total,
            spent_today,
            cap_total,
            cap_daily,
        );
        // A degraded fixture only upgrades when the budget actually allows a
        // real render; anything else would re-render the same fixture (or
        // burn a poll) every time the spec crosses the viewport.
        if reason == PrefetchReason::DegradedUpgrade && !matches!(plan, RenderPlan::Real { .. }) {
            continue;
        }
        let attempt = match reason {
            PrefetchReason::FailedRetry { attempts } => attempts + 1,
            _ => 1,
        };
        match plan {
            RenderPlan::Real { cost } => {
                spent_total += cost;
                spent_today += cost;
                let id = crate::providers::cache_distinct_render_id(&prd.sha256);
                reservations.push(wallet::RenderReservation {
                    id: id.clone(),
                    amount_usd: cost,
                    created_at: now,
                });
                cooking.insert(prd.id.clone(), CookEntry::cooking(attempt));
                jobs.push((prd.clone(), "fal", Some(id), None));
            }
            RenderPlan::DegradedFake => {
                cooking.insert(prd.id.clone(), CookEntry::cooking(attempt));
                jobs.push((
                    prd.clone(),
                    "fake",
                    None,
                    Some("render budget exhausted".into()),
                ));
            }
            RenderPlan::Fake => {
                cooking.insert(prd.id.clone(), CookEntry::cooking(attempt));
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
    let all_prds = match ctx.scan_all() {
        Ok(p) => p,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let renders = load_renders(&ctx.renders_dir()).unwrap_or_default();
    let dispatcher = ctx.dispatcher();
    let receipts = match dispatcher.reconcile_pr_states() {
        Ok(receipts) => receipts,
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    // rel_path → current sha, so a prior implement receipt for a now-re-shaped
    // spec (same path, different sha) can be badged superseded.
    let current_shas: std::collections::HashMap<String, String> = all_prds
        .iter()
        .map(|p| (p.rel_path.clone(), p.sha256.clone()))
        .collect();
    let events = events::read_all(&ctx.events_path()).unwrap_or_default();
    let mut scored: Vec<(PrdSource, crate::readiness::Readiness)> = all_prds
        .into_iter()
        .map(|prd| {
            let readiness = crate::readiness::evaluate(&prd, &events, &receipts);
            (prd, readiness)
        })
        .collect();
    scored.sort_by(|(a_prd, a_readiness), (b_prd, b_readiness)| {
        b_readiness
            .score
            .partial_cmp(&a_readiness.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a_prd.priority.cmp(&b_prd.priority))
            .then_with(|| a_prd.rel_path.cmp(&b_prd.rel_path))
    });
    scored.truncate(ctx.cfg.feed.max_items);
    let prds: Vec<PrdSource> = scored.iter().map(|(prd, _)| prd.clone()).collect();
    let now = Utc::now();
    let reservations = ctx.wallet.snapshot().await;
    let pending_usd = pending_total_spend(&reservations);
    let pending_daily_usd = pending_daily_spend(&reservations, now);
    let spend_entries = ctx.spend_entries(&renders);

    let items: Vec<Value> = scored
        .iter()
        .map(|(prd, readiness)| {
            let render = latest_render(&prd.id, &renders);
            let vibe_rating = render
                .as_ref()
                .and_then(|render| latest_vibe_rating(&prd.id, &render.id, &events));
            let dispatch = receipts.iter().find(|r| r.prd_id == prd.id);
            // A prior implement PR for an older version of this same spec file…
            let has_stale_implement = receipts.iter().any(|r| {
                r.prd_rel_path == prd.rel_path && crate::dispatch::is_superseded(r, &current_shas)
            });
            // …but once the spec has a fresh implement of its own, the stale one
            // is just history, not the current state — don't badge superseded.
            let current_is_implement =
                dispatch.is_some_and(|d| matches!(d.kind, DispatchKind::Implement));
            let superseded = has_stale_implement && !current_is_implement;
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
            // Word-synced captions (backlog 024): point the feed at the
            // persisted caption artifact when one exists, and declare whether
            // the model already burns captions into the frame so the overlay
            // never doubles seedance's native ones.
            let captions = render.as_ref().and_then(|r| {
                crate::providers::caption_artifact_url(&ctx.renders_dir(), r).map(|url| {
                    json!({
                        "url": url,
                        "native": crate::providers::fal::model_renders_native_captions(&r.model),
                    })
                })
            });
            json!({
                "prd": {
                    "id": prd.id,
                    "sha256": prd.sha256,
                    "title": prd.title,
                    "path": prd.rel_path,
                    "priority": prd.priority,
                },
                "render": render,
                "captions": captions,
                "vibe_rating": vibe_rating,
                "dispatch": dispatch.map(|d| json!({
                    "id": d.id,
                    "kind": d.kind,
                    "status": d.status,
                    "branch": d.branch,
                    "pr_url": d.pr_url,
                    "pr_state": d.pr_state,
                    "pr_state_at": d.pr_state_at,
                    "note": d.note,
                    "diff_lines": d.diff_lines,
                    "plan": d.plan,
                    "review": d.diff_lines.map(crate::dispatch::review_size),
                })),
                "readiness": readiness,
                "superseded": superseded,
                "status": status,
            })
        })
        .collect();

    // Command/query separation: only a cursor-bearing request (a feed viewer)
    // prefetches its viewport. A bare /api/state query stays a read with no spend.
    if let Some(cursor) = q.cursor {
        maybe_prefetch(&ctx, &prds, &renders, cursor).await;
    }

    // The API serializes cooking as prd_id -> label ("cooking" | "failed: …");
    // the retry bookkeeping stays server-side.
    let cooking_labels: std::collections::BTreeMap<String, String> = ctx
        .cooking
        .lock()
        .expect("cooking lock")
        .iter()
        .map(|(id, entry)| (id.clone(), entry.label.clone()))
        .collect();

    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "items": items,
            "cooking": cooking_labels,
            "video_provider": ctx.cfg.video.provider,
            "fal_configured": ctx.fal_key().is_some(),
            "max_items": ctx.cfg.feed.max_items,
            "spend": {
                "total_usd": total_spend(&spend_entries),
                "cap_usd": ctx.cfg.video.max_total_spend_usd,
                "pending_usd": pending_usd,
                "daily_usd": daily_spend(&spend_entries, now),
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
    let prds = match ctx.scan_all() {
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
    let prds = match if body.prd_id.is_some() {
        ctx.scan_all()
    } else {
        ctx.scan()
    } {
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
            .filter(|(_, entry)| entry.is_cooking())
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
        // Spend truth from the durable ledger (unioned with provenance) — a
        // wiped renders dir must not reopen the wallet.
        let spend_entries = ctx.spend_entries(&existing);
        let spent = total_spend(&spend_entries);
        let planned = planned_fal_spend(&ctx.cfg.video, &targets);
        if planned > 0.0 && body.confirmed_cost != Some(true) {
            let now = Utc::now();
            let reservations = ctx.wallet.snapshot().await;
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
                    "daily_spent_usd": daily_spend(&spend_entries, now),
                    "daily_pending_usd": pending_daily_spend(&reservations, now),
                    "daily_cap_usd": ctx.cfg.video.max_daily_spend_usd,
                    "daily_reset_at": next_daily_reset_at(now),
                })),
            )
                .into_response();
        }
        let now = Utc::now();
        let mut reservations = ctx.wallet.lock().await;
        let pending_total = pending_total_spend(&reservations);
        let pending_daily = pending_daily_spend(&reservations, now);
        let cap = ctx.cfg.video.max_total_spend_usd;
        let today = daily_spend(&spend_entries, now);
        let daily_cap = ctx.cfg.video.max_daily_spend_usd;
        // One gate. `cap_breach` owns the arithmetic; this match owns the
        // HTTP shaping (402 lifetime, 429 daily — same precedence as before).
        match cap_breach(
            spent + pending_total,
            today + pending_daily,
            planned,
            cap,
            daily_cap,
        ) {
            CapBreach::Lifetime => {
                return error_response(
                    StatusCode::PAYMENT_REQUIRED,
                    format!(
                        "spend cap: ${spent:.2} already spent + ${pending_total:.2} pending + ${planned:.2} planned for {} render(s) \
                         exceeds max_total_spend_usd ${cap:.2} — raise it in doomscrum.toml [video]",
                        targets.len()
                    ),
                );
            }
            CapBreach::Daily => {
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
            CapBreach::None => {}
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
            reservations.push(wallet::RenderReservation {
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
            // A stale "failed: …" entry does not block an explicit re-cook —
            // the operator asked, so the attempt budget resets.
            let present = cooking.get(&prd.id).is_some_and(CookEntry::is_cooking);
            if !present {
                cooking.insert(prd.id.clone(), CookEntry::cooking(1));
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
        match render_spec(&ctx, &provider_name, &prd).await {
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

#[derive(Deserialize)]
struct SwipeBody {
    prd_id: String,
    /// "implement" (right) | "shape" (explicit action) | "skip" (left/up)
    action: String,
}

async fn api_swipe(State(ctx): State<AppCtx>, Json(body): Json<SwipeBody>) -> Response {
    let prds = match ctx.scan_all() {
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
            let undo = std::time::Duration::from_secs(ctx.cfg.agent.undo_window_sec);
            tokio::spawn(async move {
                // Mis-swipe undo window: sit queued and cancellable without
                // holding a slot. A cancel during this window flips the receipt
                // to "cancelled"; run() re-reads it and bails before any git.
                if !undo.is_zero() {
                    tokio::time::sleep(undo).await;
                }
                let Ok(_permit) = slots.acquire_owned().await else {
                    return;
                };
                // Inner spawn: run() itself never panics the pipeline, but if
                // it ever does, the receipt must not stay frozen at
                // `agent_running` — mark it failed so the feed and GC see a
                // terminal status instead of a permanently "cooking" agent.
                let receipt_id = queued.id.clone();
                let runner = dispatcher.clone();
                if tokio::spawn(async move { runner.run(queued, prd).await })
                    .await
                    .is_err()
                {
                    let _ =
                        dispatcher.mark_failed(&receipt_id, "dispatch task panicked — reconciled");
                }
            });
            Json(json!({ "dispatch": receipt })).into_response()
        }
    }
}

fn active_dispatch_status(status: &str) -> bool {
    matches!(status, "queued" | "agent_running" | "opening_pr")
}

async fn api_spec(State(ctx): State<AppCtx>, UrlPath(prd_id): UrlPath<String>) -> Response {
    match ctx.scan_all() {
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

/// Mis-swipe undo: cancel a dispatch that is still in its queued window, before
/// the agent touches git. Rejects once the agent has started.
async fn api_dispatch_cancel(State(ctx): State<AppCtx>, UrlPath(id): UrlPath<String>) -> Response {
    match ctx.dispatcher().cancel(&id) {
        Ok(true) => Json(json!({ "cancelled": true })).into_response(),
        Ok(false) => (
            StatusCode::CONFLICT,
            Json(json!({ "cancelled": false, "reason": "already started or unknown" })),
        )
            .into_response(),
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
    use super::{
        await_render_task, latest_render, log_tail, media_content_type, parse_byte_range,
        prefetch_window, render_plan, CookEntry, PrefetchReason, RenderPlan, RetryPolicy, ICON_192,
        ICON_512, MANIFEST,
    };
    use crate::backlog::PrdSource;
    use crate::providers::VideoRender;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    const NO_RETRY_YET: RetryPolicy = RetryPolicy {
        max_attempts: 3,
        backoff: Duration::from_secs(3600), // effectively "never" within a test
    };

    const RETRY_NOW: RetryPolicy = RetryPolicy {
        max_attempts: 3,
        backoff: Duration::ZERO,
    };

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
    fn pwa_manifest_declares_an_installable_app() {
        let manifest: serde_json::Value = serde_json::from_str(MANIFEST).expect("valid JSON");
        assert_eq!(manifest["display"], "standalone");
        assert_eq!(manifest["start_url"], "/");
        assert!(manifest["name"].as_str().is_some_and(|n| !n.is_empty()));
        let icons = manifest["icons"].as_array().expect("icons array");
        let sizes: Vec<&str> = icons.iter().filter_map(|i| i["sizes"].as_str()).collect();
        assert!(sizes.contains(&"192x192"));
        assert!(sizes.contains(&"512x512"));
        for icon in icons {
            let src = icon["src"].as_str().expect("icon src");
            assert!(src == "/icon-192.png" || src == "/icon-512.png");
        }
    }

    #[test]
    fn pwa_icons_are_real_pngs() {
        const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
        assert!(ICON_192.starts_with(PNG_MAGIC));
        assert!(ICON_512.starts_with(PNG_MAGIC));
    }

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

    fn ids(win: &[(&PrdSource, PrefetchReason)]) -> Vec<String> {
        win.iter().map(|(p, _)| p.id.clone()).collect()
    }

    fn failed_entry(attempts: u32) -> CookEntry {
        CookEntry {
            label: "failed: boom".into(),
            attempts,
            failed_at: Some(std::time::Instant::now()),
        }
    }

    #[test]
    fn prefetch_window_covers_only_depth_specs_ahead_of_cursor() {
        let prds = vec![prd("a"), prd("b"), prd("c"), prd("d"), prd("e")];
        let renders: Vec<VideoRender> = vec![];
        let cooking = HashMap::new();
        // cursor 0, depth 3 -> the top three; d and e (deeper) cost nothing.
        let win = prefetch_window(&prds, &renders, &cooking, 0, 3, NO_RETRY_YET);
        assert_eq!(ids(&win), vec!["a", "b", "c"]);
        assert!(win.iter().all(|(_, r)| *r == PrefetchReason::Fresh));
        // cursor advances -> the window slides, bringing d and e into view.
        let win2 = prefetch_window(&prds, &renders, &cooking, 2, 3, NO_RETRY_YET);
        assert_eq!(ids(&win2), vec!["c", "d", "e"]);
    }

    #[test]
    fn prefetch_window_skips_already_rendered_and_cooking_specs() {
        let prds = vec![prd("a"), prd("b"), prd("c")];
        let renders = vec![ready_render_for("a")]; // a is cached — revisit, no re-spend
        let mut cooking = HashMap::new();
        cooking.insert("b".to_string(), CookEntry::cooking(1)); // b is in flight
        let win = prefetch_window(&prds, &renders, &cooking, 0, 3, NO_RETRY_YET);
        assert_eq!(ids(&win), vec!["c"]); // only c still needs a render
    }

    #[test]
    fn prefetch_window_is_empty_for_zero_depth_or_cursor_past_end() {
        let prds = vec![prd("a"), prd("b")];
        let renders: Vec<VideoRender> = vec![];
        let cooking = HashMap::new();
        assert!(prefetch_window(&prds, &renders, &cooking, 0, 0, NO_RETRY_YET).is_empty());
        assert!(prefetch_window(&prds, &renders, &cooking, 9, 3, NO_RETRY_YET).is_empty());
    }

    #[test]
    fn prefetch_window_offers_a_degraded_fixture_for_upgrade() {
        // A budget-degraded fixture is not "done": it re-enters the window as
        // an upgrade candidate (the caller only acts when budget allows).
        let prds = vec![prd("a")];
        let mut degraded = ready_render_for("a");
        degraded.degraded_reason = Some("render budget exhausted".into());
        let win = prefetch_window(&prds, &[degraded], &HashMap::new(), 0, 1, NO_RETRY_YET);
        assert_eq!(ids(&win), vec!["a"]);
        assert_eq!(win[0].1, PrefetchReason::DegradedUpgrade);
    }

    #[test]
    fn prefetch_window_retries_a_failed_render_after_backoff_within_attempt_budget() {
        let prds = vec![prd("a")];
        let renders: Vec<VideoRender> = vec![];
        let mut cooking = HashMap::new();
        cooking.insert("a".to_string(), failed_entry(1));
        // Backoff not yet elapsed → skipped (no retry storm on every poll).
        assert!(prefetch_window(&prds, &renders, &cooking, 0, 1, NO_RETRY_YET).is_empty());
        // Backoff elapsed → retried, carrying the attempt count.
        let win = prefetch_window(&prds, &renders, &cooking, 0, 1, RETRY_NOW);
        assert_eq!(ids(&win), vec!["a"]);
        assert_eq!(win[0].1, PrefetchReason::FailedRetry { attempts: 1 });
        // Attempt budget exhausted → the failure sticks, no more paid retries.
        cooking.insert("a".to_string(), failed_entry(RETRY_NOW.max_attempts));
        assert!(prefetch_window(&prds, &renders, &cooking, 0, 1, RETRY_NOW).is_empty());
    }

    #[tokio::test]
    async fn a_panicked_render_task_surfaces_as_a_normal_error() {
        // The supervisor path: a panic inside the detached render task must
        // come back as Err (so cleanup — cooking map, failure event,
        // reservation release — runs), never a silent swallow.
        let handle = tokio::spawn(async { panic!("render blew up") });
        let err = await_render_task(handle).await.unwrap_err();
        assert!(err.to_string().contains("panicked"), "{err:#}");
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
