//! Feed serving and just-in-time render orchestration: `GET /api/state`,
//! `POST /api/generate`, `POST /api/vibe`, and the prefetch pipeline that
//! keeps the viewport window warm as the cursor advances.

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::backlog::PrdSource;
use crate::dispatch::DispatchKind;
use crate::events;
use crate::providers::{compare_render_freshness, load_renders, VideoRender};
use crate::render::pipeline::render_spec;
use crate::render::wallet::{
    self, cap_breach, pending_daily_spend, pending_total_spend, render_plan, CapBreach, RenderPlan,
};

use super::{
    daily_spend, error_response, next_daily_reset_at, planned_fal_spend, render_provider_id,
    total_spend, AppCtx, CookEntry, VIBE_RATINGS,
};

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

/// Latest ready render for a spec.
fn latest_render(prd_id: &str, renders: &[VideoRender]) -> Option<VideoRender> {
    renders
        .iter()
        .filter(|r| r.prd_id == prd_id && r.status == "ready")
        .max_by(|a, b| compare_render_freshness(a, b))
        .cloned()
}

#[derive(Deserialize, Default)]
pub(super) struct StateQuery {
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
    let mut reservations = ctx.wallet.lock().await;
    let mut cooking = ctx.cooking.lock().expect("cooking lock");
    // Scheduling uses a fresh render snapshot while the cooking lock is held.
    // Otherwise a render can complete after /api/state loads `renders`, remove
    // its cooking entry, and leave prefetch_window looking at stale degraded
    // data that schedules the just-upgraded spec again.
    let scheduling_renders = load_renders(&ctx.renders_dir()).unwrap_or_else(|_| renders.to_vec());
    // Spend truth comes from the durable ledger (unioned with provenance),
    // never from surviving render JSONs alone.
    let entries = ctx.spend_entries(&scheduling_renders);
    let mut spent_total = total_spend(&entries) + pending_total_spend(&reservations);
    let mut spent_today = daily_spend(&entries, now) + pending_daily_spend(&reservations, now);

    // (spec, render provider, reservation id, degraded badge) — decided and
    // reserved synchronously, then spawned after the locks drop. The attempt
    // count rides on the CookEntry; the failure path reads it back from there.
    let mut jobs: Vec<(PrdSource, &'static str, Option<String>, Option<String>)> = Vec::new();
    for (prd, reason) in prefetch_window(prds, &scheduling_renders, &cooking, cursor, depth, retry)
    {
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

pub(super) async fn api_state(State(ctx): State<AppCtx>, Query(q): Query<StateQuery>) -> Response {
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
pub(super) struct VibeBody {
    prd_id: String,
    render_id: String,
    rating: String,
}

pub(super) async fn api_vibe(State(ctx): State<AppCtx>, Json(body): Json<VibeBody>) -> Response {
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
pub(super) struct GenerateBody {
    provider: Option<String>,
    prd_id: Option<String>,
    force: Option<bool>,
    confirmed_cost: Option<bool>,
}

pub(super) async fn api_generate(
    State(ctx): State<AppCtx>,
    body: Option<Json<GenerateBody>>,
) -> Response {
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

#[cfg(test)]
mod tests {
    use super::{
        await_render_task, latest_render, prefetch_window, render_plan, CookEntry, PrefetchReason,
        RenderPlan, RetryPolicy,
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
