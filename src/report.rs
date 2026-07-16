//! `doomscrum report` — "spent today, cooking now, what broke" from one
//! command. Pure rendering over disk truth (specs, the durable cost ledger,
//! render provenance, dispatch receipts, the events ledger): spend vs caps,
//! per-day / per-model / per-spec rollups, a render + dispatch status
//! breakdown naming recent failures, and queue depth vs free slots.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::backlog::PrdSource;
use crate::config::VideoConfig;
use crate::dispatch::DispatchReceipt;
use crate::events::Event;
use crate::providers::VideoRender;
use crate::render::ledger::CostEntry;
use crate::render::wallet::{daily_spend, next_daily_reset_at, total_spend};

pub struct ReportInputs<'a> {
    pub specs: &'a [PrdSource],
    /// The complete paid-spend record (`ledger::spend_entries`) — the ledger
    /// unioned with provenance, so the numbers survive a renders-dir wipe.
    pub entries: &'a [CostEntry],
    pub renders: &'a [VideoRender],
    pub receipts: &'a [DispatchReceipt],
    pub events: &'a [Event],
    pub video: &'a VideoConfig,
    pub max_concurrent_dispatches: usize,
    pub now: DateTime<Utc>,
}

fn pct(spent: f64, cap: f64) -> String {
    if cap <= 0.0 {
        return "-".into();
    }
    format!("{:.0}%", (spent / cap) * 100.0)
}

/// created_at (RFC3339) → UTC date string, or None if unparseable.
fn utc_date(created_at: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(created_at)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).date_naive().to_string())
}

pub fn render(inputs: &ReportInputs) -> String {
    let mut out = String::new();
    let push = |out: &mut String, line: String| {
        out.push_str(&line);
        out.push('\n');
    };

    push(&mut out, format!("specs={}", inputs.specs.len()));

    // --- spend --------------------------------------------------------------
    let total = total_spend(inputs.entries);
    let today = daily_spend(inputs.entries, inputs.now);
    push(&mut out, "\n== spend (durable ledger) ==".into());
    push(
        &mut out,
        format!(
            "total  ${total:.2} / cap ${:.2} ({})",
            inputs.video.max_total_spend_usd,
            pct(total, inputs.video.max_total_spend_usd)
        ),
    );
    push(
        &mut out,
        format!(
            "today  ${today:.2} / cap ${:.2} ({}) — resets {}",
            inputs.video.max_daily_spend_usd,
            pct(today, inputs.video.max_daily_spend_usd),
            next_daily_reset_at(inputs.now)
        ),
    );

    let mut by_day: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut by_model: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    let mut by_spec: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    for e in inputs.entries {
        if let Some(day) = utc_date(&e.created_at) {
            let slot = by_day.entry(day).or_default();
            slot.0 += e.cost_usd;
            slot.1 += 1;
        }
        let slot = by_model.entry(e.model.as_str()).or_default();
        slot.0 += e.cost_usd;
        slot.1 += 1;
        let slot = by_spec.entry(e.prd_id.as_str()).or_default();
        slot.0 += e.cost_usd;
        slot.1 += 1;
    }
    if !by_day.is_empty() {
        push(&mut out, "per-day (most recent 7):".into());
        for (day, (usd, n)) in by_day.iter().rev().take(7) {
            push(&mut out, format!("  {day}  ${usd:.2}  ({n} render(s))"));
        }
    }
    if !by_model.is_empty() {
        push(&mut out, "per-model:".into());
        for (model, (usd, n)) in &by_model {
            push(&mut out, format!("  ${usd:<7.2} {model}  ({n} render(s))"));
        }
    }
    if !by_spec.is_empty() {
        push(&mut out, "per-spec (top 10 by spend):".into());
        let mut specs: Vec<(&&str, &(f64, usize))> = by_spec.iter().collect();
        specs.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));
        for (prd_id, (usd, n)) in specs.into_iter().take(10) {
            let title = inputs
                .specs
                .iter()
                .find(|p| p.id == **prd_id)
                .map(|p| p.title.clone())
                .unwrap_or_else(|| crate::util::short(prd_id).to_string());
            push(&mut out, format!("  ${usd:<7.2} {title}  ({n} render(s))"));
        }
    }

    // --- renders --------------------------------------------------------------
    let ready = inputs
        .renders
        .iter()
        .filter(|r| r.status == "ready")
        .count();
    let failed = inputs
        .renders
        .iter()
        .filter(|r| r.status != "ready")
        .count();
    let degraded = inputs
        .renders
        .iter()
        .filter(|r| r.degraded_reason.is_some())
        .count();
    push(&mut out, "\n== renders ==".into());
    push(
        &mut out,
        format!(
            "renders={} ready={ready} failed={failed} degraded={degraded}",
            inputs.renders.len()
        ),
    );
    let render_failures: Vec<&Event> = inputs
        .events
        .iter()
        .rev()
        .filter(|e| e.kind == "render_failed")
        .take(3)
        .collect();
    if !render_failures.is_empty() {
        push(&mut out, "recent render failures:".into());
        for e in render_failures {
            push(
                &mut out,
                format!(
                    "  {} {}: {}",
                    e.created_at,
                    crate::util::short(&e.prd_id),
                    e.note.as_deref().unwrap_or("(no detail)")
                ),
            );
        }
    }

    // --- dispatches --------------------------------------------------------------
    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    for r in inputs.receipts {
        *by_status.entry(r.status.as_str()).or_default() += 1;
    }
    let count = |s: &str| by_status.get(s).copied().unwrap_or(0);
    push(&mut out, "\n== dispatches ==".into());
    push(
        &mut out,
        format!(
            "dispatches={} queued={} running={} opening_pr={} pr_opened={} completed_local={} failed={} cancelled={}",
            inputs.receipts.len(),
            count("queued"),
            count("agent_running"),
            count("opening_pr"),
            count("pr_opened"),
            count("completed_local"),
            count("failed"),
            count("cancelled"),
        ),
    );
    let in_use = count("agent_running") + count("opening_pr");
    let slots = inputs.max_concurrent_dispatches.max(1);
    push(
        &mut out,
        format!(
            "queue depth: {} queued, {}/{} slot(s) in use, {} free",
            count("queued"),
            in_use,
            slots,
            slots.saturating_sub(in_use)
        ),
    );
    let dispatch_failures: Vec<&DispatchReceipt> = inputs
        .receipts
        .iter()
        .filter(|r| r.status == "failed")
        .take(3)
        .collect();
    if !dispatch_failures.is_empty() {
        push(&mut out, "recent dispatch failures:".into());
        for r in dispatch_failures {
            push(
                &mut out,
                format!(
                    "  {} {}: {}",
                    r.updated_at,
                    r.prd_title,
                    r.note.as_deref().unwrap_or("(no note)")
                ),
            );
        }
    }
    push(&mut out, "recent dispatches:".into());
    for d in inputs.receipts.iter().take(10) {
        push(
            &mut out,
            format!(
                "  [{}] {:?} {} -> {} {}",
                d.status,
                d.kind,
                d.prd_title,
                d.branch,
                d.pr_url.clone().unwrap_or_default()
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{DispatchKind, DispatchReceipt};
    use std::path::PathBuf;

    fn entry(render_id: &str, prd_id: &str, model: &str, cost: f64, at: &str) -> CostEntry {
        CostEntry {
            render_id: render_id.into(),
            prd_id: prd_id.into(),
            prd_sha256: format!("sha-{prd_id}"),
            provider: "fal".into(),
            model: model.into(),
            cost_usd: cost,
            created_at: at.into(),
        }
    }

    fn receipt(status: &str, note: Option<&str>) -> DispatchReceipt {
        DispatchReceipt {
            id: format!("id-{status}-{}", note.unwrap_or("")),
            prd_id: "prd-a".into(),
            prd_sha256: "sha-a".into(),
            prd_title: "Spec A".into(),
            prd_rel_path: "backlog.d/a.md".into(),
            kind: DispatchKind::Implement,
            branch: "doomscrum/impl-a".into(),
            worktree: "/tmp/w".into(),
            status: status.into(),
            stages: Vec::new(),
            diff_lines: None,
            plan: None,
            pr_url: None,
            pr_state: None,
            pr_state_at: None,
            note: note.map(String::from),
            agent_log: "/tmp/log".into(),
            created_at: "2026-07-06T00:00:00Z".into(),
            updated_at: "2026-07-06T00:00:00Z".into(),
        }
    }

    fn spec(id: &str, title: &str) -> PrdSource {
        PrdSource {
            id: id.into(),
            sha256: format!("sha-{id}"),
            rel_path: format!("backlog.d/{id}.md"),
            abs_path: PathBuf::new(),
            title: title.into(),
            priority: 0,
            raw: String::new(),
            issue_number: None,
        }
    }

    #[test]
    fn report_rolls_up_spend_per_day_model_and_spec_and_names_failures() {
        let now = DateTime::parse_from_rfc3339("2026-07-06T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let entries = vec![
            entry("r1", "prd-a", "fal-ai/veo", 1.2, "2026-07-06T01:00:00Z"),
            entry("r2", "prd-a", "fal-ai/veo", 1.2, "2026-07-05T01:00:00Z"),
            entry("r3", "prd-b", "fal-ai/ltx", 0.4, "2026-07-06T02:00:00Z"),
        ];
        let specs = vec![spec("prd-a", "Alpha Spec"), spec("prd-b", "Beta Spec")];
        let receipts = vec![
            receipt("queued", None),
            receipt("agent_running", None),
            receipt(
                "failed",
                Some("stage 'agent' failed with exit code Some(1)"),
            ),
        ];
        let events = vec![crate::events::Event {
            id: "e1".into(),
            prd_id: "prd-b".into(),
            prd_sha256: "sha-b".into(),
            kind: "render_failed".into(),
            note: Some("fal job failed".into()),
            render_id: None,
            rating: None,
            created_at: "2026-07-06T03:00:00Z".into(),
        }];
        let video = VideoConfig::default(); // caps: $25 lifetime, $5 daily
        let report = render(&ReportInputs {
            specs: &specs,
            entries: &entries,
            renders: &[],
            receipts: &receipts,
            events: &events,
            video: &video,
            max_concurrent_dispatches: 2,
            now,
        });

        // total + today vs caps, with % consumed and the reset time
        assert!(
            report.contains("total  $2.80 / cap $25.00 (11%)"),
            "{report}"
        );
        assert!(
            report.contains("today  $1.60 / cap $5.00 (32%)"),
            "{report}"
        );
        assert!(report.contains("resets 2026-07-07T00:00:00Z"), "{report}");
        // per-day / per-model / per-spec rollups
        assert!(
            report.contains("2026-07-06  $1.60  (2 render(s))"),
            "{report}"
        );
        assert!(
            report.contains("2026-07-05  $1.20  (1 render(s))"),
            "{report}"
        );
        assert!(report.contains("fal-ai/veo  (2 render(s))"), "{report}");
        assert!(report.contains("Alpha Spec  (2 render(s))"), "{report}");
        assert!(report.contains("Beta Spec  (1 render(s))"), "{report}");
        // status breakdown + queue depth vs slots
        assert!(report.contains("queued=1 running=1"), "{report}");
        assert!(
            report.contains("queue depth: 1 queued, 1/2 slot(s) in use, 1 free"),
            "{report}"
        );
        // recent failures are named, both render- and dispatch-side
        assert!(report.contains("fal job failed"), "{report}");
        assert!(report.contains("stage 'agent' failed"), "{report}");
    }

    #[test]
    fn report_handles_an_empty_project() {
        let report = render(&ReportInputs {
            specs: &[],
            entries: &[],
            renders: &[],
            receipts: &[],
            events: &[],
            video: &VideoConfig::default(),
            max_concurrent_dispatches: 2,
            now: Utc::now(),
        });
        assert!(report.contains("specs=0"), "{report}");
        assert!(report.contains("total  $0.00"), "{report}");
        assert!(report.contains("queue depth: 0 queued, 0/2 slot(s) in use"));
    }
}
