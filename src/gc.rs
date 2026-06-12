use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use crate::dispatch::{load_receipts, DispatchReceipt};
use crate::providers::{compare_render_freshness, load_renders, VideoRender};
use crate::server::AppCtx;

#[derive(Debug, Clone)]
pub struct GcOptions {
    pub dry_run: bool,
    pub worktree_max_age_days: u64,
    pub events_max_bytes: u64,
    pub events_keep_bytes: u64,
}

impl Default for GcOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            worktree_max_age_days: 7,
            events_max_bytes: 5_000_000,
            events_keep_bytes: 200_000,
        }
    }
}

impl GcOptions {
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedPath {
    pub path: PathBuf,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsRotation {
    pub path: PathBuf,
    pub backup_path: PathBuf,
    pub original_bytes: u64,
    pub kept_bytes: u64,
    pub rotated: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GcReport {
    pub dry_run: bool,
    pub render_assets: Vec<RemovedPath>,
    pub worktrees: Vec<RemovedPath>,
    pub git_worktree_prune_ran: bool,
    pub events_rotation: Option<EventsRotation>,
}

pub fn collect(ctx: &AppCtx, options: GcOptions) -> Result<GcReport> {
    let mut report = GcReport {
        dry_run: options.dry_run,
        ..GcReport::default()
    };

    prune_render_assets(ctx, &options, &mut report)?;
    prune_worktrees(ctx, &options, &mut report)?;
    rotate_events(ctx, &options, &mut report)?;

    Ok(report)
}

impl GcReport {
    pub fn render_cli(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("gc dry_run={}\n", self.dry_run));
        out.push_str(&format!("render_assets={}\n", self.render_assets.len()));
        for entry in &self.render_assets {
            out.push_str(&format!(
                "  {} {}\n",
                if entry.deleted {
                    "deleted"
                } else {
                    "would_delete"
                },
                entry.path.display()
            ));
        }
        out.push_str(&format!(
            "git_worktree_prune={}\n",
            if self.git_worktree_prune_ran {
                "ran"
            } else if self.dry_run {
                "would_run"
            } else {
                "skipped"
            }
        ));
        out.push_str(&format!("worktrees={}\n", self.worktrees.len()));
        for entry in &self.worktrees {
            out.push_str(&format!(
                "  {} {}\n",
                if entry.deleted {
                    "deleted"
                } else {
                    "would_delete"
                },
                entry.path.display()
            ));
        }
        if let Some(rotation) = &self.events_rotation {
            out.push_str(&format!(
                "events {} original_bytes={} kept_bytes={} backup={}\n",
                if rotation.rotated {
                    "rotated=true"
                } else {
                    "would_rotate"
                },
                rotation.original_bytes,
                rotation.kept_bytes,
                rotation.backup_path.display()
            ));
        } else {
            out.push_str("events rotated=false\n");
        }
        out
    }
}

fn prune_render_assets(ctx: &AppCtx, options: &GcOptions, report: &mut GcReport) -> Result<()> {
    let renders = load_renders(&ctx.renders_dir())?;
    let mut keep: BTreeSet<String> = BTreeSet::new();
    let mut newest: BTreeMap<(&str, &str), &VideoRender> = BTreeMap::new();

    for render in renders.iter().filter(|render| render.status == "ready") {
        let key = (render.prd_id.as_str(), render.provider.as_str());
        match newest.get(&key) {
            Some(existing)
                if compare_render_freshness(render, existing) != std::cmp::Ordering::Greater => {}
            _ => {
                newest.insert(key, render);
            }
        }
    }

    for render in newest.values() {
        keep.insert(render.id.clone());
    }

    for render in renders
        .iter()
        .filter(|render| render.status == "ready" && !keep.contains(&render.id))
    {
        let asset = ctx
            .renders_dir()
            .join(&render.prd_sha256)
            .join(&render.asset_file);
        if !asset.exists() {
            continue;
        }
        if !options.dry_run {
            std::fs::remove_file(&asset)
                .with_context(|| format!("removing render asset {}", asset.display()))?;
        }
        report.render_assets.push(RemovedPath {
            path: asset,
            deleted: !options.dry_run,
        });
    }

    Ok(())
}

fn prune_worktrees(ctx: &AppCtx, options: &GcOptions, report: &mut GcReport) -> Result<()> {
    let dispatcher = ctx.dispatcher();
    if !options.dry_run {
        git_worktree_prune(&dispatcher.repo)?;
        report.git_worktree_prune_ran = true;
    }

    let receipts = load_receipts(&dispatcher.dispatches_dir)?;
    let cutoff = Duration::from_secs(options.worktree_max_age_days.saturating_mul(24 * 60 * 60));
    let now = SystemTime::now();
    for receipt in receipts
        .iter()
        .filter(|receipt| !is_active_dispatch(receipt))
    {
        let worktree = PathBuf::from(&receipt.worktree);
        if !worktree.starts_with(&dispatcher.worktrees_dir) || !worktree.is_dir() {
            continue;
        }
        if !is_old_enough(&worktree, cutoff, now) {
            continue;
        }
        if !options.dry_run {
            std::fs::remove_dir_all(&worktree)
                .with_context(|| format!("removing worktree {}", worktree.display()))?;
        }
        report.worktrees.push(RemovedPath {
            path: worktree,
            deleted: !options.dry_run,
        });
    }

    if !options.dry_run && !report.worktrees.is_empty() {
        git_worktree_prune(&dispatcher.repo)?;
    }

    Ok(())
}

fn git_worktree_prune(repo: &std::path::Path) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "worktree", "prune"])
        .output()
        .with_context(|| format!("running git worktree prune in {}", repo.display()))?;
    anyhow::ensure!(
        output.status.success(),
        "git worktree prune failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn is_active_dispatch(receipt: &DispatchReceipt) -> bool {
    matches!(
        receipt.status.as_str(),
        "queued" | "agent_running" | "opening_pr"
    )
}

fn is_old_enough(path: &std::path::Path, cutoff: Duration, now: SystemTime) -> bool {
    if cutoff.is_zero() {
        return true;
    }
    let Ok(modified) = path.metadata().and_then(|metadata| metadata.modified()) else {
        return false;
    };
    now.duration_since(modified)
        .map(|age| age >= cutoff)
        .unwrap_or(false)
}

fn rotate_events(ctx: &AppCtx, options: &GcOptions, report: &mut GcReport) -> Result<()> {
    let path = ctx.events_path();
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if metadata.len() <= options.events_max_bytes {
        return Ok(());
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading events ledger {}", path.display()))?;
    let kept = keep_recent_lines(&raw, options.events_keep_bytes as usize);
    let backup_path = path.with_file_name(format!(
        "events.ndjson.{}.bak",
        chrono::Utc::now().format("%Y%m%d%H%M%S%3f")
    ));

    if !options.dry_run {
        std::fs::write(&backup_path, &raw)
            .with_context(|| format!("writing events backup {}", backup_path.display()))?;
        std::fs::write(&path, &kept)
            .with_context(|| format!("rewriting events ledger {}", path.display()))?;
    }

    report.events_rotation = Some(EventsRotation {
        path,
        backup_path,
        original_bytes: metadata.len(),
        kept_bytes: kept.len() as u64,
        rotated: !options.dry_run,
    });
    Ok(())
}

fn keep_recent_lines(raw: &str, keep_bytes: usize) -> String {
    if keep_bytes == 0 {
        return String::new();
    }
    let mut kept = Vec::new();
    let mut total = 0usize;
    for line in raw.lines().rev() {
        let bytes = line.len() + 1;
        if !kept.is_empty() && total + bytes > keep_bytes {
            break;
        }
        kept.push(line);
        total += bytes;
        if total >= keep_bytes {
            break;
        }
    }
    kept.reverse();
    if kept.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept.join("\n"))
    }
}
