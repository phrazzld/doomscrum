use serde::Serialize;

use crate::backlog::PrdSource;
use crate::dispatch::{DispatchKind, DispatchReceipt};
use crate::events::Event;

#[derive(Debug, Clone, Serialize)]
pub struct Readiness {
    pub score: f64,
    pub signals: Vec<String>,
}

impl Default for Readiness {
    fn default() -> Self {
        Self {
            score: 0.0,
            signals: Vec::new(),
        }
    }
}

/// Deterministic per-spec readiness from generated state only. Source specs
/// remain authoritative; receipts and events are destroyable learning.
pub fn evaluate(prd: &PrdSource, events: &[Event], receipts: &[DispatchReceipt]) -> Readiness {
    let mut readiness = Readiness::default();

    for receipt in receipts.iter().filter(|r| r.prd_sha256 == prd.sha256) {
        match receipt.pr_state.as_deref() {
            Some("merged") => {
                let delta = if receipt.kind == DispatchKind::Shape {
                    1.25
                } else {
                    4.0
                };
                readiness.score += delta;
                readiness
                    .signals
                    .push(if receipt.kind == DispatchKind::Shape {
                        "merged_shape_pr".into()
                    } else {
                        "merged_pr".into()
                    });
            }
            Some("closed") => {
                readiness.score -= 3.0;
                readiness.signals.push("closed_pr".into());
            }
            Some("open") => {
                readiness.score += 0.5;
                readiness.signals.push("open_pr".into());
            }
            _ if receipt.status == "failed" => {
                readiness.score -= 1.0;
                readiness.signals.push("failed_dispatch".into());
            }
            _ if receipt.status == "pr_opened" => {
                readiness.score += 0.25;
                readiness.signals.push("pr_opened".into());
            }
            _ => {}
        }
    }

    if events
        .iter()
        .any(|e| e.prd_sha256 == prd.sha256 && matches!(e.kind.as_str(), "dispatch_shape"))
    {
        readiness.score += 0.25;
        readiness.signals.push("shape_dispatched".into());
    }

    if let Some(rating) = events
        .iter()
        .rev()
        .find(|e| e.prd_sha256 == prd.sha256 && e.kind == "vibe_rating")
        .and_then(|e| e.rating.as_deref())
    {
        match rating {
            "cursed" => {
                readiness.score += 0.6;
                readiness.signals.push("vibe:cursed".into());
            }
            "brainrot" => {
                readiness.score += 0.5;
                readiness.signals.push("vibe:brainrot".into());
            }
            "solid" => {
                readiness.score += 0.25;
                readiness.signals.push("vibe:solid".into());
            }
            "corporate" => {
                readiness.score -= 0.2;
                readiness.signals.push("vibe:corporate".into());
            }
            _ => {}
        }
    }

    readiness
}
