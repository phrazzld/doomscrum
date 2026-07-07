//! The render wallet: spend accounting, the budget gate, and in-flight
//! reservations.
//!
//! The product's entire safety story is spend-vs-cap arithmetic, so it lives in
//! exactly ONE place — [`cap_breach`] — consumed by every gate: `/api/generate`,
//! the feed prefetch, and CLI `generate`. The reservation lifecycle (reserve on
//! approval, release on completion) is owned by [`Wallet`]; `AppCtx` holds it as
//! an opaque handle.

use std::sync::Arc;

use chrono::{DateTime, SecondsFormat, Utc};
use tokio::sync::Mutex as AsyncMutex;

use crate::backlog::PrdSource;
use crate::config::VideoConfig;
use crate::render::ledger::CostEntry;

/// Paid render spend that has been approved and started but not yet persisted as
/// render provenance — counted against the cap so concurrent jobs can't overshoot.
#[derive(Clone)]
pub struct RenderReservation {
    pub id: String,
    pub amount_usd: f64,
    pub created_at: DateTime<Utc>,
}

fn clean_money(value: f64) -> f64 {
    if value.abs() < f64::EPSILON {
        0.0
    } else {
        value
    }
}

/// Total estimated spend on real renders, summed from the durable cost
/// record ([`crate::render::ledger::spend_entries`] — the ledger unioned with
/// any provenance it does not know about). Never read spend from surviving
/// render JSONs alone: wiping `.doomscrum/renders` must not reset the meter.
pub fn total_spend(entries: &[CostEntry]) -> f64 {
    let sum = entries
        .iter()
        .filter(|e| e.provider == "fal")
        .map(|e| e.cost_usd)
        .sum();
    clean_money(sum)
}

/// Spend on real renders whose recorded timestamp falls on the UTC date of
/// `now`. The reset boundary is UTC so it is stable across operator machines.
pub fn daily_spend(entries: &[CostEntry], now: DateTime<Utc>) -> f64 {
    let today = now.date_naive();
    let sum = entries
        .iter()
        .filter(|e| e.provider == "fal")
        .filter(|e| {
            DateTime::parse_from_rfc3339(&e.created_at)
                .map(|dt| dt.with_timezone(&Utc).date_naive() == today)
                .unwrap_or(false)
        })
        .map(|e| e.cost_usd)
        .sum();
    clean_money(sum)
}

pub fn pending_total_spend(reservations: &[RenderReservation]) -> f64 {
    clean_money(reservations.iter().map(|r| r.amount_usd).sum())
}

pub fn pending_daily_spend(reservations: &[RenderReservation], now: DateTime<Utc>) -> f64 {
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

pub fn planned_fal_spend(video: &VideoConfig, prds: &[PrdSource]) -> f64 {
    prds.iter()
        .map(|p| crate::providers::fal::unit_cost(&video.with_pipeline(&p.sha256)))
        .sum()
}

/// Which cap (if any) a spend of `amount` would breach. **THE** single site
/// where spend-vs-cap arithmetic lives; every gate routes through it. Lifetime
/// is checked before daily so callers can surface distinct status codes.
#[derive(Debug, PartialEq, Eq)]
pub enum CapBreach {
    None,
    Lifetime,
    Daily,
}

pub fn cap_breach(
    spent_total: f64,
    spent_today: f64,
    amount: f64,
    cap_total: f64,
    cap_daily: f64,
) -> CapBreach {
    if spent_total + amount > cap_total {
        CapBreach::Lifetime
    } else if spent_today + amount > cap_daily {
        CapBreach::Daily
    } else {
        CapBreach::None
    }
}

/// How a windowed spec gets rendered. The wallet gate refuses over-cap real
/// renders, but the feed must never go dark — so an over-budget spec degrades
/// to a free fixture badged with the reason instead of failing the request.
#[derive(Debug, PartialEq)]
pub enum RenderPlan {
    Real { cost: f64 },
    DegradedFake,
    Fake,
    Skip,
}

/// Decide how to render one window spec. `fal` over the lifetime or daily cap
/// degrades to a badged fixture (oracle: the feed survives an exhausted wallet);
/// `fal` within budget renders for real when a key is present, else is left for
/// an explicit generate; a free provider just renders.
pub fn render_plan(
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
    match cap_breach(spent_total, spent_today, cost, cap_total, cap_daily) {
        CapBreach::None => {
            if fal_key_present {
                RenderPlan::Real { cost }
            } else {
                RenderPlan::Skip
            }
        }
        _ => RenderPlan::DegradedFake,
    }
}

/// In-flight paid-render reservations. The reservation lifecycle lives here;
/// `AppCtx` holds this as an opaque handle.
#[derive(Clone)]
pub struct Wallet {
    reservations: Arc<AsyncMutex<Vec<RenderReservation>>>,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            reservations: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    /// Lock the reservation ledger for an atomic plan-then-reserve block — the
    /// prefetch path decides and reserves under one lock so two concurrent
    /// serves can't both commit the same budget.
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, Vec<RenderReservation>> {
        self.reservations.lock().await
    }

    pub async fn reserve(&self, id: String, amount_usd: f64, now: DateTime<Utc>) {
        self.reservations.lock().await.push(RenderReservation {
            id,
            amount_usd,
            created_at: now,
        });
    }

    /// Release a reservation once its render is persisted (or abandoned).
    /// `None` is a no-op so callers needn't branch on whether they reserved.
    pub async fn release(&self, id: Option<&str>) {
        let Some(id) = id else {
            return;
        };
        self.reservations.lock().await.retain(|r| r.id != id);
    }

    /// A clone of the current reservations for read-only accounting.
    pub async fn snapshot(&self) -> Vec<RenderReservation> {
        self.reservations.lock().await.clone()
    }
}

impl Default for Wallet {
    fn default() -> Self {
        Self::new()
    }
}
