use std::path::{Path, PathBuf};
use std::process::Command;

use doomscrum::config::Config;
use doomscrum::dispatch::{DispatchKind, DispatchReceipt};
use doomscrum::gc::{collect, GcOptions};
use doomscrum::providers::{save_render, VideoRender};
use doomscrum::server::AppCtx;
use serde_json::json;

fn sh(cwd: &Path, cmd: &[&str]) {
    let out = Command::new(cmd[0])
        .args(&cmd[1..])
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("spawn {cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "command {cmd:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn cmd(cwd: &Path, program: &str, args: &[&str]) -> std::process::Output {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("spawn {program} {args:?}: {e}"))
}

fn ctx(root: PathBuf) -> AppCtx {
    let cfg = Config::default();
    AppCtx::new(root, cfg)
}

fn render_fixture(
    prd_id: &str,
    prd_sha256: &str,
    id: &str,
    provider: &str,
    created_at: &str,
) -> VideoRender {
    let asset_file = format!("{id}.mp4");
    VideoRender {
        id: id.into(),
        prd_id: prd_id.into(),
        prd_sha256: prd_sha256.into(),
        storyboard_id: format!("{id}-storyboard"),
        provider: provider.into(),
        model: "test-model".into(),
        native_audio: true,
        status: "ready".into(),
        asset_url: format!("/media/{prd_sha256}/{asset_file}"),
        asset_file,
        caption_artifact_file: None,
        degraded_reason: None,
        provider_job_id: Some(format!("{id}-job")),
        cost_estimate_usd: 0.0,
        latency_ms: 1,
        created_at: created_at.into(),
    }
}

fn write_render_asset(ctx: &AppCtx, render: &VideoRender) {
    save_render(&ctx.renders_dir(), render).unwrap();
    let path = ctx
        .renders_dir()
        .join(&render.prd_sha256)
        .join(&render.asset_file);
    std::fs::write(path, format!("mp4:{}", render.id)).unwrap();
}

fn write_receipt(ctx: &AppCtx, id: &str, status: &str, worktree: &Path) {
    std::fs::create_dir_all(ctx.dispatcher().dispatches_dir.clone()).unwrap();
    let receipt = DispatchReceipt {
        id: id.into(),
        prd_id: "prd".into(),
        prd_sha256: "sha".into(),
        prd_title: "Ticket".into(),
        kind: DispatchKind::Implement,
        branch: format!("doomscrum/{id}"),
        worktree: worktree.to_string_lossy().to_string(),
        status: status.into(),
        stages: Vec::new(),
        pr_url: None,
        note: None,
        agent_log: ctx
            .dispatcher()
            .dispatches_dir
            .join(format!("{id}.agent.log"))
            .to_string_lossy()
            .to_string(),
        created_at: "2026-06-12T00:00:00.000Z".into(),
        updated_at: "2026-06-12T00:00:00.000Z".into(),
    };
    std::fs::write(
        ctx.dispatcher().dispatches_dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[test]
fn dry_run_reports_superseded_render_without_deleting_assets_or_specs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    let spec_path = root.join("backlog.d/001-demo.md");
    std::fs::write(&spec_path, "# Demo\n\n## Goal\nShip.\n").unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);

    let ctx = ctx(root.clone());
    let old = render_fixture("prd-1", "sha-1", "old", "fal", "2026-06-12T00:00:00.000Z");
    let new = render_fixture("prd-1", "sha-1", "new", "fal", "2026-06-12T00:01:00.000Z");
    write_render_asset(&ctx, &old);
    write_render_asset(&ctx, &new);

    let before_spec = std::fs::read(&spec_path).unwrap();
    let report = collect(&ctx, GcOptions::dry_run()).unwrap();

    assert_eq!(report.render_assets.len(), 1);
    assert!(report.render_assets[0].path.ends_with("old.mp4"));
    assert!(!report.render_assets[0].deleted);
    assert!(ctx.renders_dir().join("sha-1/old.mp4").exists());
    assert!(ctx.renders_dir().join("sha-1/old.json").exists());
    assert_eq!(std::fs::read(&spec_path).unwrap(), before_spec);
}

#[test]
fn gc_removes_only_superseded_render_assets_and_preserves_provenance_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root);

    let old_fal = render_fixture(
        "prd-1",
        "sha-1",
        "old-fal",
        "fal",
        "2026-06-12T00:00:00.000Z",
    );
    let new_fal = render_fixture(
        "prd-1",
        "sha-1",
        "new-fal",
        "fal",
        "2026-06-12T00:01:00.000Z",
    );
    let fake = render_fixture(
        "prd-1",
        "sha-1",
        "only-fake",
        "fake-local",
        "2026-06-12T00:00:30.000Z",
    );
    write_render_asset(&ctx, &old_fal);
    write_render_asset(&ctx, &new_fal);
    write_render_asset(&ctx, &fake);

    let report = collect(&ctx, GcOptions::default()).unwrap();

    assert_eq!(report.render_assets.len(), 1);
    assert!(report.render_assets[0].deleted);
    assert!(!ctx.renders_dir().join("sha-1/old-fal.mp4").exists());
    assert!(ctx.renders_dir().join("sha-1/old-fal.json").exists());
    assert!(ctx.renders_dir().join("sha-1/new-fal.mp4").exists());
    assert!(ctx.renders_dir().join("sha-1/only-fake.mp4").exists());
}

#[test]
fn gc_prunes_terminal_worktrees_but_keeps_open_dispatch_state() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root);
    let dispatcher = ctx.dispatcher();
    let completed = dispatcher.worktrees_dir.join("completed");
    std::fs::create_dir_all(&completed).unwrap();
    write_receipt(&ctx, "completed", "completed_local", &completed);
    let active_statuses = ["queued", "agent_running", "opening_pr"];
    for status in active_statuses {
        let worktree = dispatcher.worktrees_dir.join(status);
        std::fs::create_dir_all(&worktree).unwrap();
        write_receipt(&ctx, status, status, &worktree);
    }

    let report = collect(
        &ctx,
        GcOptions {
            worktree_max_age_days: 0,
            ..GcOptions::default()
        },
    )
    .unwrap();

    assert_eq!(report.worktrees.len(), 1);
    assert_eq!(report.worktrees[0].path, completed);
    assert!(report.git_worktree_prune_ran);
    assert!(!completed.exists());
    for status in active_statuses {
        assert!(dispatcher.worktrees_dir.join(status).exists());
        assert!(dispatcher
            .dispatches_dir
            .join(format!("{status}.json"))
            .exists());
    }
}

#[test]
fn gc_keeps_terminal_worktrees_younger_than_the_age_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root);
    let dispatcher = ctx.dispatcher();
    let completed = dispatcher.worktrees_dir.join("completed");
    std::fs::create_dir_all(&completed).unwrap();
    write_receipt(&ctx, "completed", "completed_local", &completed);

    let report = collect(
        &ctx,
        GcOptions {
            worktree_max_age_days: 7,
            ..GcOptions::default()
        },
    )
    .unwrap();

    assert!(report.worktrees.is_empty());
    assert!(completed.exists());
}

#[test]
fn gc_prunes_git_metadata_after_deleting_registered_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    sh(
        &root,
        &["git", "config", "user.email", "test@doomscrum.local"],
    );
    sh(&root, &["git", "config", "user.name", "DoomScrum Test"]);
    std::fs::write(root.join("backlog.d/001-demo.md"), "# Demo\n").unwrap();
    sh(&root, &["git", "add", "-A"]);
    sh(&root, &["git", "commit", "-qm", "init"]);
    let ctx = ctx(root.clone());
    let worktree = ctx.dispatcher().worktrees_dir.join("registered");
    std::fs::create_dir_all(ctx.dispatcher().worktrees_dir.clone()).unwrap();
    sh(
        &root,
        &[
            "git",
            "worktree",
            "add",
            worktree.to_str().unwrap(),
            "-b",
            "doomscrum/registered",
        ],
    );
    write_receipt(&ctx, "registered", "completed_local", &worktree);

    collect(
        &ctx,
        GcOptions {
            worktree_max_age_days: 0,
            ..GcOptions::default()
        },
    )
    .unwrap();

    assert!(!worktree.exists());
    let list = cmd(&root, "git", &["worktree", "list", "--porcelain"]);
    assert!(list.status.success());
    assert!(
        !String::from_utf8_lossy(&list.stdout).contains(worktree.to_str().unwrap()),
        "{}",
        String::from_utf8_lossy(&list.stdout)
    );
}

#[test]
fn gc_rotates_large_event_ledger_and_keeps_recent_complete_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root);
    std::fs::create_dir_all(ctx.state_dir()).unwrap();
    let events = [
        json!({"id":"1","prd_id":"p","prd_sha256":"s","kind":"skip","created_at":"2026-06-12T00:00:00.000Z"}).to_string(),
        json!({"id":"2","prd_id":"p","prd_sha256":"s","kind":"rendered","created_at":"2026-06-12T00:01:00.000Z"}).to_string(),
        json!({"id":"3","prd_id":"p","prd_sha256":"s","kind":"vibe_rating","render_id":"r","rating":"solid","created_at":"2026-06-12T00:02:00.000Z"}).to_string(),
    ];
    std::fs::write(ctx.events_path(), format!("{}\n", events.join("\n"))).unwrap();

    let report = collect(
        &ctx,
        GcOptions {
            events_max_bytes: 40,
            events_keep_bytes: events[2].len() as u64 + 1,
            ..GcOptions::default()
        },
    )
    .unwrap();

    let rotation = report.events_rotation.expect("events rotation");
    assert!(rotation.rotated);
    assert!(rotation.backup_path.exists());
    let current = std::fs::read_to_string(ctx.events_path()).unwrap();
    assert_eq!(current, format!("{}\n", events[2]));
    let backup = std::fs::read_to_string(rotation.backup_path).unwrap();
    assert!(backup.contains("\"id\":\"1\""));
    assert!(backup.contains("\"id\":\"3\""));
}

#[test]
fn dry_run_reports_worktrees_and_events_without_mutating_them() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root);
    let dispatcher = ctx.dispatcher();
    let completed = dispatcher.worktrees_dir.join("completed");
    std::fs::create_dir_all(&completed).unwrap();
    write_receipt(&ctx, "completed", "completed_local", &completed);
    std::fs::create_dir_all(ctx.state_dir()).unwrap();
    let events = "one\nsecond\nthird\n";
    std::fs::write(ctx.events_path(), events).unwrap();

    let report = collect(
        &ctx,
        GcOptions {
            dry_run: true,
            worktree_max_age_days: 0,
            events_max_bytes: 4,
            events_keep_bytes: 6,
        },
    )
    .unwrap();

    assert_eq!(report.worktrees.len(), 1);
    assert!(!report.worktrees[0].deleted);
    assert!(!report.git_worktree_prune_ran);
    assert!(completed.exists());
    let rotation = report.events_rotation.expect("events dry-run rotation");
    assert!(!rotation.rotated);
    assert!(!rotation.backup_path.exists());
    assert_eq!(std::fs::read_to_string(ctx.events_path()).unwrap(), events);
}

#[test]
fn cli_dry_run_prints_planned_deletions_and_rotations() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    let ctx = ctx(root.clone());
    let old = render_fixture("prd-1", "sha-1", "old", "fal", "2026-06-12T00:00:00.000Z");
    let new = render_fixture("prd-1", "sha-1", "new", "fal", "2026-06-12T00:01:00.000Z");
    write_render_asset(&ctx, &old);
    write_render_asset(&ctx, &new);
    let completed = ctx.dispatcher().worktrees_dir.join("completed");
    std::fs::create_dir_all(&completed).unwrap();
    write_receipt(&ctx, "completed", "completed_local", &completed);
    std::fs::write(ctx.events_path(), "one\nsecond\nthird\n").unwrap();

    let out = cmd(
        &root,
        env!("CARGO_BIN_EXE_doomscrum"),
        &[
            "--root",
            root.to_str().unwrap(),
            "gc",
            "--dry-run",
            "--worktree-max-age-days",
            "0",
            "--events-max-bytes",
            "4",
            "--events-keep-bytes",
            "6",
        ],
    );

    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would_delete"));
    assert!(stdout.contains("old.mp4"));
    assert!(stdout.contains("git_worktree_prune=would_run"));
    assert!(stdout.contains("events would_rotate"));
    assert!(stdout.contains("original_bytes="));
    assert!(ctx.renders_dir().join("sha-1/old.mp4").exists());
    assert!(completed.exists());
    assert_eq!(
        std::fs::read_to_string(ctx.events_path()).unwrap(),
        "one\nsecond\nthird\n"
    );
}
