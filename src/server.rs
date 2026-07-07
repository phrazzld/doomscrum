use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use crate::backlog::{self, PrdSource};
use crate::config::Config;
use crate::dispatch::Dispatcher;
use crate::events;
use crate::providers::{fake::FakeProvider, fal::FalProvider, Provider, VideoRender};
use crate::render::ledger::{self, CostEntry};
use crate::render::wallet::Wallet;
use crate::secrets;

// Re-exported so `server::total_spend` & friends stay stable for `main.rs` and
// the test suite; the implementations now live in `render::wallet`.
pub use crate::render::wallet::{daily_spend, next_daily_reset_at, planned_fal_spend, total_spend};

mod dispatch;
mod feed;
mod media;

use dispatch::{api_dispatch_cancel, api_dispatches, api_dispatch_log, api_spec, api_swipe};
use feed::{api_generate, api_state, api_vibe};
use media::media;

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

#[cfg(test)]
mod tests {
    use super::{ICON_192, ICON_512, MANIFEST};

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
}
