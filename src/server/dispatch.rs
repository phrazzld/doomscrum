//! Agent dispatch HTTP routes: swipes, spec source, dispatch list, cancellation,
//! and redacted agent-log tails.

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::dispatch::{load_receipts, DispatchKind};
use crate::{events, secrets};

use super::{error_response, AppCtx};

#[derive(Deserialize)]
pub(super) struct SwipeBody {
    prd_id: String,
    /// "implement" (right) | "shape" (explicit action) | "skip" (left/up)
    action: String,
}

pub(super) async fn api_swipe(State(ctx): State<AppCtx>, Json(body): Json<SwipeBody>) -> Response {
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

pub(super) async fn api_spec(
    State(ctx): State<AppCtx>,
    UrlPath(prd_id): UrlPath<String>,
) -> Response {
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
pub(super) async fn api_dispatch_cancel(
    State(ctx): State<AppCtx>,
    UrlPath(id): UrlPath<String>,
) -> Response {
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

pub(super) async fn api_dispatches(State(ctx): State<AppCtx>) -> Response {
    match load_receipts(&ctx.dispatcher().dispatches_dir) {
        Ok(receipts) => Json(json!({ "dispatches": receipts })).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// Tail of one dispatch's agent log — what the feed shows while an agent
/// is cooking and when it flops. Receipts persist after every stage, so
/// this is pure surfacing.
pub(super) async fn api_dispatch_log(
    State(ctx): State<AppCtx>,
    UrlPath(id): UrlPath<String>,
) -> Response {
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

#[cfg(test)]
mod tests {
    use super::log_tail;

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
}
