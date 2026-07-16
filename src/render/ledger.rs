//! The durable cost ledger: an append-only ndjson record of every real
//! (paid) render, written the moment provenance exists and never rewritten.
//!
//! Render provenance JSONs live under `.doomscrum/renders/` next to their
//! MP4s — wiping that directory (or a future pruning policy) would reset the
//! spend meter while the money stayed spent. The ledger lives OUTSIDE the
//! renders dir (`costs.ndjson` at the state-dir root) so the wallet gate's
//! spend arithmetic survives a render wipe. Reads union the ledger with any
//! render provenance not yet in it (pre-ledger history, or a render whose
//! ledger append failed), deduped by render id, so no spend is ever counted
//! twice or lost.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::providers::VideoRender;

/// One paid render's spend, as recorded at render time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    pub render_id: String,
    pub prd_id: String,
    pub prd_sha256: String,
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub created_at: String,
}

impl CostEntry {
    pub fn from_render(render: &VideoRender) -> Self {
        Self {
            render_id: render.id.clone(),
            prd_id: render.prd_id.clone(),
            prd_sha256: render.prd_sha256.clone(),
            provider: render.provider.clone(),
            model: render.model.clone(),
            cost_usd: render.cost_estimate_usd,
            created_at: render.created_at.clone(),
        }
    }
}

/// Append one entry. Append-only: the file is only ever opened in append
/// mode, so a crash mid-write can at worst truncate the final line (which
/// `read_all` skips), never rewrite history.
pub fn append(ledger_path: &Path, entry: &CostEntry) -> Result<()> {
    if let Some(parent) = ledger_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .with_context(|| format!("opening {}", ledger_path.display()))?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

/// All ledger entries. A missing file is an empty ledger; unparseable lines
/// (torn final write) are skipped.
pub fn read_all(ledger_path: &Path) -> Result<Vec<CostEntry>> {
    match std::fs::read_to_string(ledger_path) {
        Ok(raw) => Ok(raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

/// The complete paid-spend record: every ledger entry, plus any paid render
/// provenance the ledger does not know about (renders from before the ledger
/// shipped, or whose ledger append failed), deduped by render id. This is the
/// ONE spend source every wallet gate reads.
pub fn spend_entries(ledger: Vec<CostEntry>, renders: &[VideoRender]) -> Vec<CostEntry> {
    let known: std::collections::HashSet<&str> =
        ledger.iter().map(|e| e.render_id.as_str()).collect();
    let missing: Vec<CostEntry> = renders
        .iter()
        .filter(|r| r.cost_estimate_usd > 0.0 && !known.contains(r.id.as_str()))
        .map(CostEntry::from_render)
        .collect();
    let mut entries = ledger;
    entries.extend(missing);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(render_id: &str, cost: f64, created_at: &str) -> CostEntry {
        CostEntry {
            render_id: render_id.into(),
            prd_id: "prd-1".into(),
            prd_sha256: "sha-1".into(),
            provider: "fal".into(),
            model: "test-model".into(),
            cost_usd: cost,
            created_at: created_at.into(),
        }
    }

    fn fal_render(id: &str, cost: f64) -> VideoRender {
        VideoRender {
            id: id.into(),
            prd_id: "prd-1".into(),
            prd_sha256: "sha-1".into(),
            storyboard_id: format!("{id}-sb"),
            provider: "fal".into(),
            model: "test-model".into(),
            native_audio: true,
            status: "ready".into(),
            asset_file: format!("{id}.mp4"),
            asset_url: format!("/media/sha-1/{id}.mp4"),
            caption_artifact_file: None,
            degraded_reason: None,
            provider_job_id: None,
            cost_estimate_usd: cost,
            latency_ms: 1,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn append_then_read_roundtrip_and_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/costs.ndjson");
        assert!(read_all(&path).unwrap().is_empty());
        append(&path, &entry("r1", 1.2, "2026-01-01T00:00:00Z")).unwrap();
        append(&path, &entry("r2", 0.6, "2026-01-02T00:00:00Z")).unwrap();
        let entries = read_all(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].render_id, "r1");
        assert_eq!(entries[1].cost_usd, 0.6);
    }

    #[test]
    fn spend_entries_unions_ledger_with_unknown_render_provenance() {
        // r1 is in both (deduped, ledger wins); r2 only on disk (pre-ledger
        // history — still counted); r3 only in the ledger (renders dir wiped —
        // the money stays spent).
        let ledger = vec![
            entry("r1", 1.2, "2026-01-01T00:00:00Z"),
            entry("r3", 0.5, "2026-01-03T00:00:00Z"),
        ];
        let renders = vec![fal_render("r1", 1.2), fal_render("r2", 0.9)];
        let entries = spend_entries(ledger, &renders);
        let mut ids: Vec<&str> = entries.iter().map(|e| e.render_id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["r1", "r2", "r3"]);
        let total: f64 = entries.iter().map(|e| e.cost_usd).sum();
        assert!((total - 2.6).abs() < 1e-9, "{total}");
    }

    #[test]
    fn spend_entries_ignores_free_renders() {
        let mut free = fal_render("free-1", 0.0);
        free.provider = "fake-local".into();
        let entries = spend_entries(Vec::new(), &[free]);
        assert!(entries.is_empty());
    }
}
