//! End-to-end tests through the HTTP routes — the layer the UI actually
//! talks to. The previous incarnation of this project asserted dispatch
//! behavior on an inner function while the route did something else; these
//! tests exist so that cannot happen again.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use doomscrum::config::Config;
use doomscrum::providers::{save_render, VideoRender};
use doomscrum::server::{router, AppCtx};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TestApp {
    addr: SocketAddr,
    root: PathBuf,
    bare: PathBuf,
    _tmp: tempfile::TempDir,
}

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

const SPECS: &[(&str, &str)] = &[
    (
        "001-first.md",
        "# First Spec\n\n## User\nOperators.\n\n## Goal\nShip the first thing.\n\n## Acceptance Criteria\n- It works.\n\n## Risk\nNone.\n",
    ),
    (
        "002-second.md",
        "# Second Spec\n\n## Goal\nShip the second thing.\n",
    ),
    (
        "003-third.md",
        "# Third Spec\n\n## Goal\nShip the third thing.\n",
    ),
];

async fn spawn_app() -> TestApp {
    spawn_app_with(|_| {}).await
}

async fn spawn_app_with(configure: impl FnOnce(&mut Config)) -> TestApp {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    let bare = tmp.path().join("origin.git");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    for (name, body) in SPECS {
        std::fs::write(root.join("backlog.d").join(name), body).unwrap();
    }

    // The synced repo: real git repo with a real (bare) origin remote.
    sh(&root, &["git", "init", "-q", "-b", "main"]);
    sh(
        &root,
        &["git", "config", "user.email", "test@doomscrum.local"],
    );
    sh(&root, &["git", "config", "user.name", "DoomScrum Test"]);
    sh(&root, &["git", "config", "commit.gpgsign", "false"]);
    std::fs::write(root.join(".gitignore"), ".doomscrum/\n").unwrap();
    sh(&root, &["git", "add", "-A"]);
    sh(&root, &["git", "commit", "-qm", "init"]);
    sh(tmp.path(), &["git", "init", "-q", "--bare", "origin.git"]);
    sh(
        &root,
        &["git", "remote", "add", "origin", bare.to_str().unwrap()],
    );

    // Stub agents: prove the pipeline drives whatever command is configured.
    let mut cfg = Config::default();
    cfg.agent.implement_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo implemented > impl-marker.txt".into(),
    ];
    cfg.agent.shape_cmd = vec![
        "sh".into(),
        "-c".into(),
        "printf '\\n## Notes\\nsharpened by agent\\n' >> backlog.d/002-second.md".into(),
    ];
    cfg.agent.pr_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo https://example.test/pr/42".into(),
    ];
    configure(&mut cfg);

    let ctx = AppCtx::new(root.clone(), cfg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(ctx)).await.unwrap();
    });
    TestApp {
        addr,
        root,
        bare,
        _tmp: tmp,
    }
}

impl TestApp {
    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    async fn get(&self, path: &str) -> (u16, Value) {
        let res = reqwest::get(self.url(path)).await.unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    async fn get_range(
        &self,
        path: &str,
        range: &str,
    ) -> (u16, reqwest::header::HeaderMap, Vec<u8>) {
        let res = reqwest::Client::new()
            .get(self.url(path))
            .header(reqwest::header::RANGE, range)
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        let headers = res.headers().clone();
        let bytes = res.bytes().await.unwrap().to_vec();
        (status, headers, bytes)
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let res = reqwest::Client::new()
            .post(self.url(path))
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    /// Poll until the dispatch reaches a terminal status.
    async fn await_dispatch(&self, id: &str) -> Value {
        for _ in 0..100 {
            let (_, body) = self.get("/api/dispatches").await;
            if let Some(d) = body["dispatches"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|d| d["id"] == id)
            {
                let status = d["status"].as_str().unwrap_or_default();
                if ["pr_opened", "completed_local", "failed"].contains(&status) {
                    return d.clone();
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("dispatch {id} never reached a terminal status");
    }

    /// Poll `/api/state{query}` until the predicate holds. Prefetch renders run
    /// detached, so a serve returns before they land — poll to observe them.
    async fn poll_state(&self, query: &str, ready: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..100 {
            let (_, body) = self.get(&format!("/api/state{query}")).await;
            if ready(&body) {
                return body;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("/api/state never satisfied the predicate");
    }

    fn git_stdout(&self, cwd: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }
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

fn seed_dispatch_receipt(
    app: &TestApp,
    prd: &Value,
    id: &str,
    status: &str,
    pr_url: Option<&str>,
    pr_state: Option<&str>,
) {
    let dispatches = app.root.join(".doomscrum/dispatches");
    std::fs::create_dir_all(&dispatches).unwrap();
    let mut receipt = json!({
        "id": id,
        "prd_id": prd["id"].as_str().unwrap(),
        "prd_sha256": prd["sha256"].as_str().unwrap(),
        "prd_title": prd["title"].as_str().unwrap(),
        "prd_rel_path": prd["path"].as_str().unwrap(),
        "kind": "implement",
        "branch": format!("doomscrum/impl-test-{id}"),
        "worktree": app.root.join(".doomscrum/worktrees").join(id).to_string_lossy(),
        "status": status,
        "stages": [],
        "pr_url": pr_url,
        "note": null,
        "agent_log": dispatches.join(format!("{id}.agent.log")).to_string_lossy(),
        "created_at": "2026-07-07T00:00:00.000Z",
        "updated_at": "2026-07-07T00:00:00.000Z"
    });
    if let Some(state) = pr_state {
        receipt["pr_state"] = json!(state);
        receipt["pr_state_at"] = json!("2026-07-07T00:00:00.000Z");
    }
    std::fs::write(
        dispatches.join(format!("{id}.json")),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

fn state_item<'a>(state: &'a Value, prd_id: &str) -> &'a Value {
    state["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["prd"]["id"] == prd_id)
        .unwrap()
}

static GH_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread")]
async fn feed_renders_and_serves_video() {
    let app = spawn_app().await;

    let (status, state) = app.get("/api/state").await;
    assert_eq!(status, 200);
    let items = state["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["prd"]["title"], "First Spec");
    assert_eq!(items[0]["status"], "new");
    assert!(items[0]["render"].is_null());

    let (status, body) = app.post("/api/generate", json!({})).await;
    assert_eq!(status, 200, "generate failed: {body}");
    assert_eq!(body["renders"].as_array().unwrap().len(), 3);

    let (_, state) = app.get("/api/state").await;
    let first = &state["items"][0];
    assert_eq!(first["status"], "rendered");
    let url = first["render"]["asset_url"].as_str().unwrap();
    let res = reqwest::get(app.url(url)).await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "video/mp4");
    let bytes = res.bytes().await.unwrap();
    assert_eq!(&bytes[4..8], b"ftyp", "served asset is a real MP4");
    let total_len = bytes.len();

    let (status, headers, range_bytes) = app.get_range(url, "bytes=4-7").await;
    assert_eq!(status, 206);
    assert_eq!(headers["content-type"], "video/mp4");
    assert_eq!(headers["accept-ranges"], "bytes");
    assert_eq!(headers["content-length"], "4");
    assert_eq!(headers["content-range"], format!("bytes 4-7/{total_len}"));
    assert_eq!(range_bytes, b"ftyp");

    // Regenerate is idempotent unless forced.
    let (_, body) = app.post("/api/generate", json!({})).await;
    assert_eq!(body["renders"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn outcome_history_scores_and_orders_feed_without_touching_specs() {
    let app = spawn_app().await;
    let (_, initial) = app.get("/api/state").await;
    let prds: Vec<Value> = initial["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["prd"].clone())
        .collect();
    assert_eq!(
        prds.iter()
            .map(|p| p["title"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["First Spec", "Second Spec", "Third Spec"]
    );
    let source_before: Vec<(String, Vec<u8>)> = prds
        .iter()
        .map(|prd| {
            let path = prd["path"].as_str().unwrap().to_string();
            (path.clone(), std::fs::read(app.root.join(&path)).unwrap())
        })
        .collect();

    save_render(
        &app.root.join(".doomscrum/renders"),
        &render_fixture(
            prds[1]["id"].as_str().unwrap(),
            prds[1]["sha256"].as_str().unwrap(),
            "second-render",
            "fake-local",
            "2026-07-07T00:00:00.000Z",
        ),
    )
    .unwrap();
    let (status, body) = app
        .post(
            "/api/vibe",
            json!({
                "prd_id": prds[1]["id"],
                "render_id": "second-render",
                "rating": "cursed"
            }),
        )
        .await;
    assert_eq!(status, 200, "vibe rating failed: {body}");
    seed_dispatch_receipt(
        &app,
        &prds[1],
        "second-merged",
        "pr_opened",
        Some("https://github.com/example/repo/pull/2"),
        Some("merged"),
    );
    seed_dispatch_receipt(
        &app,
        &prds[2],
        "third-closed",
        "pr_opened",
        Some("https://github.com/example/repo/pull/3"),
        Some("closed"),
    );

    let (_, ranked) = app.get("/api/state").await;
    let items = ranked["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item["prd"]["title"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Second Spec", "First Spec", "Third Spec"],
        "feed should be readiness score order with filename as tiebreaker: {ranked}"
    );
    assert_eq!(items.len(), 3, "readiness must not hide or gate specs");
    let first_score = state_item(&ranked, prds[0]["id"].as_str().unwrap())["readiness"]["score"]
        .as_f64()
        .unwrap();
    let second = state_item(&ranked, prds[1]["id"].as_str().unwrap());
    let third_score = state_item(&ranked, prds[2]["id"].as_str().unwrap())["readiness"]["score"]
        .as_f64()
        .unwrap();
    assert!(
        second["readiness"]["score"].as_f64().unwrap() > first_score,
        "merged PR plus high vibe should raise readiness: {second}"
    );
    assert!(
        third_score < first_score,
        "closed-unmerged PR should lower readiness: {ranked}"
    );
    assert!(
        second["readiness"]["signals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|signal| signal == "merged_pr"),
        "readiness should explain the PR signal: {second}"
    );

    for (path, before) in &source_before {
        assert_eq!(
            before,
            &std::fs::read(app.root.join(path)).unwrap(),
            "scoring must not mutate source spec {path}"
        );
    }

    std::fs::remove_dir_all(app.root.join(".doomscrum")).unwrap();
    let (_, reset) = app.get("/api/state").await;
    assert_eq!(
        reset["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["prd"]["title"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["First Spec", "Second Spec", "Third Spec"],
        "deleting generated state should reset learning to filename order"
    );
    for item in reset["items"].as_array().unwrap() {
        assert_eq!(item["readiness"]["score"], 0.0);
    }
    for (path, before) in &source_before {
        assert_eq!(before, &std::fs::read(app.root.join(path)).unwrap());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn state_poll_reconciles_pr_state_from_gh_and_surfaces_on_card() {
    let _guard = GH_ENV_LOCK.lock().await;
    let old_gh = std::env::var_os("DOOMSCRUM_GH_BIN");
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd = state["items"][0]["prd"].clone();
    seed_dispatch_receipt(
        &app,
        &prd,
        "needs-reconcile",
        "pr_opened",
        Some("https://github.com/example/repo/pull/42"),
        None,
    );

    let gh = app.root.parent().unwrap().join("fake-gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\nprintf '%s\\n' '{\"state\":\"MERGED\",\"mergedAt\":\"2026-07-07T12:00:00Z\",\"closedAt\":\"2026-07-07T12:00:00Z\"}'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("DOOMSCRUM_GH_BIN", &gh);

    let (_, reconciled) = app.get("/api/state").await;
    let item = state_item(&reconciled, prd["id"].as_str().unwrap());
    assert_eq!(item["dispatch"]["pr_state"], "merged", "{item}");
    assert!(
        item["dispatch"]["pr_state_at"]
            .as_str()
            .unwrap()
            .contains('T'),
        "{item}"
    );
    assert_eq!(item["readiness"]["signals"][0], "merged_pr");

    let raw = std::fs::read_to_string(app.root.join(".doomscrum/dispatches/needs-reconcile.json"))
        .unwrap();
    assert!(raw.contains("\"pr_state\": \"merged\""), "{raw}");

    match old_gh {
        Some(value) => std::env::set_var("DOOMSCRUM_GH_BIN", value),
        None => std::env::remove_var("DOOMSCRUM_GH_BIN"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn state_selects_newest_ready_render_even_when_fixture_is_newer_than_real() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd = &state["items"][0]["prd"];
    let prd_id = prd["id"].as_str().unwrap();
    let prd_sha256 = prd["sha256"].as_str().unwrap();

    save_render(
        &app.root.join(".doomscrum/renders"),
        &render_fixture(
            prd_id,
            prd_sha256,
            "old-real-render",
            "fal",
            "2026-01-01T00:00:00.000Z",
        ),
    )
    .unwrap();
    save_render(
        &app.root.join(".doomscrum/renders"),
        &render_fixture(
            prd_id,
            prd_sha256,
            "new-fixture-render",
            "fake-local",
            "2026-01-01T00:00:01.000Z",
        ),
    )
    .unwrap();

    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][0]["render"]["id"], "new-fixture-render");
    assert_eq!(state["items"][0]["render"]["provider"], "fake-local");
}

#[tokio::test(flavor = "multi_thread")]
async fn forced_regeneration_preserves_old_json_and_updates_gallery_metadata() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd = &state["items"][0]["prd"];
    let prd_id = prd["id"].as_str().unwrap().to_string();
    let prd_sha256 = prd["sha256"].as_str().unwrap().to_string();
    let prd_path = prd["path"].as_str().unwrap();
    let source_path = app.root.join(prd_path);
    let source_before = std::fs::read(&source_path).unwrap();

    let (status, first_body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fake", "prd_id": prd_id }),
        )
        .await;
    assert_eq!(status, 200, "first generate failed: {first_body}");
    let first = &first_body["renders"][0];
    let first_id = first["id"].as_str().unwrap().to_string();
    let first_url = first["asset_url"].as_str().unwrap().to_string();

    let (status, second_body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fake", "prd_id": prd_id, "force": true }),
        )
        .await;
    assert_eq!(status, 200, "forced generate failed: {second_body}");
    let second = &second_body["renders"][0];
    let second_id = second["id"].as_str().unwrap().to_string();
    let second_url = second["asset_url"].as_str().unwrap().to_string();

    assert_ne!(first_id, second_id);
    assert_ne!(first_url, second_url);

    let render_dir = app.root.join(".doomscrum/renders").join(&prd_sha256);
    let mut json_files: Vec<_> = std::fs::read_dir(&render_dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension().is_some_and(|ext| ext == "json")).then_some(path)
        })
        .collect();
    json_files.sort();
    assert_eq!(json_files.len(), 2, "render JSON files: {json_files:?}");
    assert!(render_dir.join(format!("{first_id}.json")).exists());
    assert!(render_dir.join(format!("{second_id}.json")).exists());

    let source_after = std::fs::read(&source_path).unwrap();
    assert_eq!(source_before, source_after);

    let (_, refreshed) = app.get("/api/state").await;
    assert_eq!(refreshed["items"][0]["render"]["id"], second_id);
    assert_eq!(refreshed["items"][0]["render"]["asset_url"], second_url);
}

#[test]
fn gallery_card_signature_tracks_render_media_url() {
    let html = include_str!("../assets/index.html");
    assert!(html.contains("item && item.render && item.render.asset_url"));
    assert!(html.contains("item && item.vibe_rating"));
    assert!(html.contains("/api/vibe"));
    assert!(html.contains("data-vibe"));
}

#[tokio::test(flavor = "multi_thread")]
async fn egress_route_enumerates_both_payloads() {
    let app = spawn_app().await;
    let (status, body) = app.get("/api/egress").await;
    assert_eq!(status, 200, "egress route failed: {body}");
    let ids: Vec<String> = body["payloads"]
        .as_array()
        .expect("payloads is an array")
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&"scriptwriter-openrouter".to_string()),
        "missing OpenRouter payload: {ids:?}"
    );
    assert!(
        ids.contains(&"render-fal".to_string()),
        "missing fal payload: {ids:?}"
    );
    // The disclosure must name prd.raw (OpenRouter) and the spec
    // title/goal/criterion (fal), not just "spec text".
    let joined = body["payloads"].to_string();
    assert!(joined.contains("prd.raw"), "must name prd.raw: {joined}");
    assert!(
        joined.contains("title"),
        "must name the spec title: {joined}"
    );
    assert!(joined.contains("goal"), "must name the spec goal: {joined}");
    assert!(
        joined.contains("criterion"),
        "must name the first criterion: {joined}"
    );
}

#[test]
fn feed_ui_surfaces_egress_disclosure_panel() {
    let html = include_str!("../assets/index.html");
    // The disclosure panel + chip + fetch from the runtime route.
    assert!(
        html.contains(r#"id="egressOverlay""#),
        "missing egress overlay"
    );
    assert!(html.contains(r#"id="egressChip""#), "missing egress chip");
    assert!(html.contains("/api/egress"), "missing /api/egress fetch");
    // AI-generated disclosure on the splash.
    assert!(
        html.contains("AI-generated"),
        "missing AI-generated disclosure"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn right_swipe_dispatches_agent_and_opens_pr() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": prd_id, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200, "swipe failed: {body}");
    let id = body["dispatch"]["id"].as_str().unwrap().to_string();
    let branch = body["dispatch"]["branch"].as_str().unwrap().to_string();
    assert!(
        branch.starts_with("doomscrum/impl-first-spec-"),
        "branch: {branch}"
    );

    let receipt = app.await_dispatch(&id).await;
    assert_eq!(receipt["status"], "pr_opened", "receipt: {receipt}");
    assert_eq!(receipt["pr_url"], "https://example.test/pr/42");

    // The branch with the agent's commit really landed on the remote.
    let refs = app.git_stdout(
        &app.bare,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    );
    assert!(refs.contains(&branch), "remote refs: {refs}");
    let subject = app.git_stdout(&app.bare, &["log", "-1", "--format=%s", &branch]);
    assert!(
        subject.contains("doomscrum: agent output for First Spec"),
        "subject: {subject}"
    );
    let files = app.git_stdout(&app.bare, &["ls-tree", "--name-only", &branch]);
    assert!(files.contains("impl-marker.txt"), "files: {files}");

    // Feed reflects the dispatch status.
    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][0]["status"], "pr_opened");
    assert_eq!(
        state["items"][0]["dispatch"]["pr_url"],
        "https://example.test/pr/42"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn real_render_route_requires_cost_confirmation_and_daily_budget() {
    let app = spawn_app_with(|cfg| {
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 0.01;
    })
    .await;

    let (status, body) = app
        .post("/api/generate", json!({ "provider": "fal" }))
        .await;
    assert_eq!(
        status, 409,
        "unconfirmed fal render should stop early: {body}"
    );
    assert_eq!(body["requires_confirmation"], true);
    assert_eq!(body["render_count"], 3);
    assert!(
        body["planned_usd"].as_f64().unwrap() > 0.01,
        "planned cost should be quoted before provider construction: {body}"
    );

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "confirmed_cost": true }),
        )
        .await;
    assert_eq!(
        status, 429,
        "daily budget should fail before FAL key lookup: {body}"
    );
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("daily render budget"));
    assert_eq!(body["daily_cap_usd"], 0.01);
    assert!(body["reset_at"].as_str().unwrap().contains('T'));
}

#[tokio::test(flavor = "multi_thread")]
async fn real_render_budget_counts_in_flight_reservations() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let fal = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-model"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(json!({
                    "video": { "url": format!("{}/files/out.mp4", fal.uri()) }
                })),
        )
        .expect(1)
        .mount(&fal)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/out.mp4"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(b"\x00\x00\x00\x18ftypmp42-doomscrum-test"),
        )
        .mount(&fal)
        .await;

    let app = spawn_app_with(|cfg| {
        cfg.script.mode = "templates".into();
        cfg.video.fal_model = "fal-ai/test-model".into();
        cfg.video.fal_base_url = fal.uri();
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 1.8;
        cfg.video.price_per_second_usd = 0.15;
    })
    .await;
    let (_, state) = app.get("/api/state").await;
    let first = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let second = state["items"][1]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "prd_id": first, "confirmed_cost": true }),
        )
        .await;
    assert_eq!(status, 200, "first paid render should start: {body}");
    assert_eq!(body["started"], true);

    // The first render dispatches to fal in a detached task; wait for its POST
    // to actually reach the mock before continuing. Without this the mock's
    // expect(1) check (run when `fal` drops at function end) can race a
    // not-yet-scheduled spawn under heavy parallel load and observe zero
    // requests. Waiting also makes "pending first render" provably true: the
    // response is delayed 2s, so the reservation is still in flight here.
    let mut dispatched = false;
    for _ in 0..100 {
        let landed = fal
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .any(|r| r.url.path() == "/fal-ai/test-model");
        if landed {
            dispatched = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(dispatched, "first paid render never dispatched to fal");

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "prd_id": second, "confirmed_cost": true }),
        )
        .await;
    assert_eq!(
        status, 429,
        "pending first render must count against daily cap: {body}"
    );
    assert_eq!(body["daily_pending_usd"], 1.2);
    assert_eq!(body["planned_usd"], 1.2);
    assert!(body["reset_at"].as_str().unwrap().contains('T'));
}

#[tokio::test(flavor = "multi_thread")]
async fn dispatches_are_capped_queued_and_deduped_while_active() {
    let app = spawn_app_with(|cfg| {
        cfg.agent.open_pr = false;
        cfg.agent.max_concurrent_dispatches = 1;
        cfg.agent.implement_cmd = vec![
            "sh".into(),
            "-c".into(),
            "sleep 1; echo implemented > impl-marker.txt".into(),
        ];
    })
    .await;
    let (_, state) = app.get("/api/state").await;
    let first = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let second = state["items"][1]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": first, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200, "first swipe failed: {body}");
    let first_id = body["dispatch"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": first, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200, "duplicate swipe failed: {body}");
    assert_eq!(body["deduped"], true);
    assert_eq!(body["dispatch"]["id"], first_id);

    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": second, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200, "second swipe failed: {body}");
    let second_id = body["dispatch"]["id"].as_str().unwrap().to_string();
    assert_ne!(first_id, second_id);

    tokio::time::sleep(Duration::from_millis(250)).await;
    let (_, body) = app.get("/api/dispatches").await;
    let second_receipt = body["dispatches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == second_id)
        .unwrap();
    assert_eq!(
        second_receipt["status"], "queued",
        "second receipt: {second_receipt}"
    );

    let first_receipt = app.await_dispatch(&first_id).await;
    let second_receipt = app.await_dispatch(&second_id).await;
    assert_eq!(first_receipt["status"], "completed_local");
    assert_eq!(second_receipt["status"], "completed_local");
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_shape_action_dispatches_shape_agent_that_edits_the_spec() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][1]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post("/api/swipe", json!({ "prd_id": prd_id, "action": "shape" }))
        .await;
    assert_eq!(status, 200, "swipe failed: {body}");
    let id = body["dispatch"]["id"].as_str().unwrap().to_string();
    let branch = body["dispatch"]["branch"].as_str().unwrap().to_string();
    assert!(
        branch.starts_with("doomscrum/shape-second-spec-"),
        "branch: {branch}"
    );

    let receipt = app.await_dispatch(&id).await;
    assert_eq!(receipt["status"], "pr_opened", "receipt: {receipt}");

    // The shaped spec landed on the remote branch with the agent's edit.
    let shaped = app.git_stdout(
        &app.bare,
        &["show", &format!("{branch}:backlog.d/002-second.md")],
    );
    assert!(
        shaped.contains("sharpened by agent"),
        "shaped spec: {shaped}"
    );
    // ...and the working backlog on main is untouched.
    let local = std::fs::read_to_string(app.root.join("backlog.d/002-second.md")).unwrap();
    assert!(!local.contains("sharpened by agent"));
}

/// The demo-day contract for arbitrary repos: when `repo.path` points at a
/// foreign repo, the worktree, branch, commit, push, and PR all happen
/// against THAT repo — while runtime state stays under the operator root.
#[tokio::test(flavor = "multi_thread")]
async fn dispatch_against_a_foreign_repo_routes_to_that_repos_remote() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("operator");
    std::fs::create_dir_all(&root).unwrap();

    // The foreign repo: its own git history, backlog, and bare origin.
    let target = tmp.path().join("olympus");
    let target_bare = tmp.path().join("olympus.git");
    std::fs::create_dir_all(target.join("backlog.d")).unwrap();
    std::fs::write(
        target.join("backlog.d/001-foreign.md"),
        "# Foreign Spec\n\n## Goal\nProve dispatch routes here.\n",
    )
    .unwrap();
    sh(&target, &["git", "init", "-q", "-b", "main"]);
    sh(
        &target,
        &["git", "config", "user.email", "t@doomscrum.local"],
    );
    sh(&target, &["git", "config", "user.name", "DoomScrum Test"]);
    sh(&target, &["git", "config", "commit.gpgsign", "false"]);
    sh(&target, &["git", "add", "-A"]);
    sh(&target, &["git", "commit", "-qm", "init"]);
    sh(tmp.path(), &["git", "init", "-q", "--bare", "olympus.git"]);
    sh(
        &target,
        &[
            "git",
            "remote",
            "add",
            "origin",
            target_bare.to_str().unwrap(),
        ],
    );

    let mut cfg = Config::default();
    cfg.repo.path = target.to_string_lossy().to_string();
    cfg.agent.implement_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo done > foreign-marker.txt".into(),
    ];
    cfg.agent.pr_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo https://example.test/pr/7".into(),
    ];

    let ctx = AppCtx::new(root.clone(), cfg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(ctx)).await.unwrap();
    });
    let app = TestApp {
        addr,
        root: root.clone(),
        bare: target_bare.clone(),
        _tmp: tmp,
    };

    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][0]["prd"]["title"], "Foreign Spec");
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": prd_id, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200, "swipe failed: {body}");
    let id = body["dispatch"]["id"].as_str().unwrap().to_string();
    let branch = body["dispatch"]["branch"].as_str().unwrap().to_string();

    let receipt = app.await_dispatch(&id).await;
    assert_eq!(receipt["status"], "pr_opened", "receipt: {receipt}");

    // The branch + agent commit landed on the FOREIGN repo's remote.
    let refs = app.git_stdout(
        &app.bare,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    );
    assert!(refs.contains(&branch), "foreign remote refs: {refs}");
    let files = app.git_stdout(&app.bare, &["ls-tree", "--name-only", &branch]);
    assert!(files.contains("foreign-marker.txt"), "files: {files}");

    // Operator root never became a git repo; state stayed on its side.
    assert!(!root.join(".git").exists());
    assert!(root.join(".doomscrum/dispatches").exists());
}

/// 009: a flopped agent is visible from the feed — failing stage + log
/// tail via the log route — and retrying creates a fresh dispatch.
#[tokio::test(flavor = "multi_thread")]
async fn failed_dispatch_exposes_log_and_retry_creates_a_fresh_dispatch() {
    let mut app = spawn_app().await;
    // Keep a handle on the temp dir; rebuild the app with a flopping agent.
    let root = app.root.clone();
    let mut cfg = Config::default();
    cfg.agent.implement_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo the build exploded spectacularly; exit 1".into(),
    ];
    let ctx = AppCtx::new(root, cfg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    app.addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(ctx)).await.unwrap();
    });

    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let (_, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": prd_id, "action": "implement" }),
        )
        .await;
    let id = body["dispatch"]["id"].as_str().unwrap().to_string();
    let receipt = app.await_dispatch(&id).await;
    assert_eq!(receipt["status"], "failed");

    // The log route surfaces the failing stage and the agent's last words.
    let (status, log) = app.get(&format!("/api/dispatch/{id}/log")).await;
    assert_eq!(status, 200);
    assert_eq!(log["status"], "failed");
    assert_eq!(log["failing_stage"], "agent");
    let tail = log["tail"].as_array().unwrap();
    assert!(
        tail.iter()
            .any(|l| l.as_str().unwrap_or("").contains("exploded spectacularly")),
        "tail: {tail:?}"
    );

    // Retry = swipe again: a fresh dispatch with a fresh id.
    let (status, body) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": prd_id, "action": "implement" }),
        )
        .await;
    assert_eq!(status, 200);
    let id2 = body["dispatch"]["id"].as_str().unwrap();
    assert_ne!(id, id2, "retry must create a fresh dispatch");

    let (status, _) = app.get("/api/dispatch/nope/log").await;
    assert_eq!(status, 404);
}

/// Demo-day flow: switch the synced repo from the UI without a restart;
/// feed follows, state stays namespaced per repo.
#[tokio::test(flavor = "multi_thread")]
async fn repo_switch_at_runtime_swaps_the_feed_and_isolates_state() {
    let app = spawn_app().await;

    // A second repo with its own backlog appears on disk.
    let other = app.root.parent().unwrap().join("otherrepo");
    std::fs::create_dir_all(other.join("backlog.d")).unwrap();
    std::fs::write(
        other.join("backlog.d/001-other.md"),
        "# Other Repo Spec\n\n## Goal\nBe someone else's backlog.\n",
    )
    .unwrap();

    // Render the default repo first so we can prove isolation.
    let (_, body) = app.post("/api/generate", json!({})).await;
    assert_eq!(body["renders"].as_array().unwrap().len(), 3);

    let (status, body) = app.get("/api/repo").await;
    assert_eq!(status, 200);
    assert_eq!(body["name"], "project");

    // Switch; the feed now serves the other repo, unrendered.
    let (status, body) = app
        .post("/api/repo", json!({ "path": other.to_string_lossy() }))
        .await;
    assert_eq!(status, 200, "switch failed: {body}");
    assert_eq!(body["name"], "otherrepo");
    assert_eq!(body["recents"].as_array().unwrap().len(), 1);

    let (_, state) = app.get("/api/state").await;
    let items = state["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["prd"]["title"], "Other Repo Spec");
    assert!(
        items[0]["render"].is_null(),
        "default repo's renders bled into the other repo"
    );

    // Switch back: original feed and its renders are still there.
    let (status, _) = app
        .post("/api/repo", json!({ "path": app.root.to_string_lossy() }))
        .await;
    assert_eq!(status, 200);
    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"].as_array().unwrap().len(), 3);
    assert!(!state["items"][0]["render"].is_null());

    // Junk paths are rejected.
    let (status, _) = app
        .post("/api/repo", json!({ "path": "/nope/zilch" }))
        .await;
    assert_eq!(status, 400);
    let no_backlog = app.root.parent().unwrap().join("plain");
    std::fs::create_dir_all(&no_backlog).unwrap();
    let (status, _) = app
        .post("/api/repo", json!({ "path": no_backlog.to_string_lossy() }))
        .await;
    assert_eq!(status, 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn skip_swipe_is_durable_and_nondestructive() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][2]["prd"]["id"].as_str().unwrap().to_string();

    let (status, _) = app
        .post("/api/swipe", json!({ "prd_id": prd_id, "action": "skip" }))
        .await;
    assert_eq!(status, 200);

    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][2]["status"], "skipped");
    // Skipping never creates a dispatch.
    let (_, body) = app.get("/api/dispatches").await;
    assert_eq!(body["dispatches"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn vibe_rating_is_render_scoped_and_nondestructive() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd = &state["items"][0]["prd"];
    let prd_id = prd["id"].as_str().unwrap().to_string();
    let prd_sha256 = prd["sha256"].as_str().unwrap().to_string();
    let prd_path = prd["path"].as_str().unwrap();
    let source_path = app.root.join(prd_path);
    let source_before = std::fs::read(&source_path).unwrap();

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fake", "prd_id": prd_id }),
        )
        .await;
    assert_eq!(status, 200, "generate failed: {body}");
    let render_id = body["renders"][0]["id"].as_str().unwrap().to_string();
    let render_json = app
        .root
        .join(".doomscrum/renders")
        .join(&prd_sha256)
        .join(format!("{render_id}.json"));
    let render_json_before = std::fs::read(&render_json).unwrap();

    let (status, body) = app
        .post(
            "/api/vibe",
            json!({ "prd_id": prd_id, "render_id": render_id, "rating": "cursed" }),
        )
        .await;
    assert_eq!(status, 200, "vibe rating failed: {body}");
    assert_eq!(body["event"]["kind"], "vibe_rating");
    assert_eq!(body["event"]["render_id"], render_id);
    assert_eq!(body["event"]["rating"], "cursed");

    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][0]["vibe_rating"], "cursed");

    let events = std::fs::read_to_string(app.root.join(".doomscrum/events.ndjson")).unwrap();
    assert!(events.contains("\"kind\":\"vibe_rating\""), "{events}");
    assert!(events.contains("\"rating\":\"cursed\""), "{events}");
    assert_eq!(source_before, std::fs::read(&source_path).unwrap());
    assert_eq!(render_json_before, std::fs::read(&render_json).unwrap());

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fake", "prd_id": prd_id, "force": true }),
        )
        .await;
    assert_eq!(status, 200, "forced generate failed: {body}");
    let new_render_id = body["renders"][0]["id"].as_str().unwrap();
    assert_ne!(new_render_id, render_id);

    let (_, state) = app.get("/api/state").await;
    assert_eq!(state["items"][0]["render"]["id"], new_render_id);
    assert!(state["items"][0]["vibe_rating"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn vibe_rating_rejects_unknown_rating_and_wrong_render() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let (status, body) = app
        .post(
            "/api/vibe",
            json!({ "prd_id": prd_id, "render_id": "nope", "rating": "cursed" }),
        )
        .await;
    assert_eq!(status, 404, "missing render should be rejected: {body}");

    let (status, body) = app
        .post("/api/generate", json!({ "provider": "fake" }))
        .await;
    assert_eq!(status, 200, "generate failed: {body}");
    let render_id = body["renders"][0]["id"].as_str().unwrap().to_string();

    let (_, state) = app.get("/api/state").await;
    let other_prd_id = state["items"][1]["prd"]["id"].as_str().unwrap().to_string();
    let (status, body) = app
        .post(
            "/api/vibe",
            json!({ "prd_id": other_prd_id, "render_id": render_id, "rating": "cursed" }),
        )
        .await;
    assert_eq!(
        status, 404,
        "render from a different spec should be rejected: {body}"
    );

    let (status, body) = app
        .post(
            "/api/vibe",
            json!({ "prd_id": prd_id, "render_id": render_id, "rating": "shareholder" }),
        )
        .await;
    assert_eq!(status, 400, "unknown rating should be rejected: {body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tap_returns_exact_spec_source() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd = &state["items"][0]["prd"];
    let (status, spec) = app
        .get(&format!("/api/spec/{}", prd["id"].as_str().unwrap()))
        .await;
    assert_eq!(status, 200);
    assert_eq!(spec["path"], "backlog.d/001-first.md");
    assert_eq!(spec["sha256"], *prd.get("sha256").unwrap());
    assert!(spec["raw"]
        .as_str()
        .unwrap()
        .contains("## Acceptance Criteria"));
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_requests_are_rejected() {
    let app = spawn_app().await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();

    let (status, _) = app
        .post("/api/swipe", json!({ "prd_id": prd_id, "action": "yolo" }))
        .await;
    assert_eq!(status, 400);

    let (status, _) = app
        .post(
            "/api/swipe",
            json!({ "prd_id": "nope", "action": "implement" }),
        )
        .await;
    assert_eq!(status, 404);

    let (status, _) = app.get("/api/spec/nope").await;
    assert_eq!(status, 404);

    let (status, _) = app.get("/media/abc/evil.txt").await;
    assert_eq!(status, 403);

    let (status, _) = app
        .post("/api/generate", json!({ "provider": "nonsense" }))
        .await;
    assert_eq!(status, 400);
}

// Debug-only in the router (`#[cfg(debug_assertions)]`) — tests always build
// in debug, so the route is present here, and it is compiled out of the
// release/Fly image. This is the live-panic parity proof: a request to the
// deliberately-panicking route must come back 500 (CatchPanicLayer caught it)
// and the server must keep serving afterward (the worker task survived, not
// died with the panic). The panic-hook side (`doomscrum.panic` at the hub) is
// covered by the unit test in `canary.rs`; here we prove the wired route.
#[tokio::test]
async fn debug_panic_route_returns_500_and_keeps_serving() {
    let app = spawn_app().await;

    let res = reqwest::get(app.url("/debug/panic")).await.unwrap();
    assert_eq!(res.status().as_u16(), 500);

    // The process is still up and answering — a panicking handler did not
    // take the server down with it.
    let (status, _) = app.get("/api/state").await;
    assert_eq!(status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn serving_the_feed_prefetches_only_the_viewport_window() {
    let app = spawn_app_with(|cfg| {
        cfg.video.provider = "fake".into();
        cfg.feed.prefetch_depth = 2;
    })
    .await;
    // Serving the feed at the top triggers JIT renders for the window [0, 2).
    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("?cursor=0", |b| {
            b["items"][0]["render"].is_object() && b["items"][1]["render"].is_object()
        })
        .await;
    // The two windowed specs render; the deeper spec stays $0 / unrendered.
    assert!(body["items"][0]["render"].is_object(), "spec 0 rendered");
    assert!(body["items"][1]["render"].is_object(), "spec 1 rendered");
    assert!(
        body["items"][2]["render"].is_null(),
        "a spec deeper than the window must not render: {}",
        body["items"][2]["render"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn revisiting_a_rendered_spec_replays_the_cache_without_respending() {
    let app = spawn_app_with(|cfg| {
        cfg.video.provider = "fake".into();
        cfg.feed.prefetch_depth = 1;
    })
    .await;
    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("?cursor=0", |b| b["items"][0]["render"].is_object())
        .await;
    let first = body["items"][0]["render"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Re-serve the same position: the cached render replays, no new render id.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let again = app.get("/api/state?cursor=0").await.1;
    assert_eq!(
        again["items"][0]["render"]["id"].as_str().unwrap(),
        first,
        "revisit must replay the cached render, not spend on a new one"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn over_budget_jit_renders_degrade_to_a_badged_fixture() {
    let app = spawn_app_with(|cfg| {
        cfg.video.provider = "fal".into();
        cfg.video.max_total_spend_usd = 0.0; // any real render is over the cap
        cfg.feed.prefetch_depth = 1;
    })
    .await;
    // The feed must serve even with the wallet exhausted (no 402/500).
    let (status, _) = app.get("/api/state?cursor=0").await;
    assert_eq!(status, 200);
    let body = app
        .poll_state("?cursor=0", |b| b["items"][0]["render"].is_object())
        .await;
    let render = &body["items"][0]["render"];
    assert_eq!(
        render["provider"], "fake-local",
        "over-budget render degrades to the free provider"
    );
    assert_eq!(
        render["degraded_reason"], "render budget exhausted",
        "a degraded render must badge the reason"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bare_state_query_does_not_prefetch_or_spend() {
    let app = spawn_app_with(|cfg| {
        cfg.video.provider = "fake".into();
        cfg.feed.prefetch_depth = 3;
    })
    .await;
    // A bare /api/state (no cursor) is a pure read — it must never start renders.
    for _ in 0..3 {
        app.get("/api/state").await;
    }
    // Wait well past the fake-render latency a prefetch would have incurred.
    tokio::time::sleep(Duration::from_millis(400)).await;
    let (_, body) = app.get("/api/state").await;
    assert_eq!(
        body["cooking"],
        json!({}),
        "a bare query must not start any render"
    );
    for (i, item) in body["items"].as_array().unwrap().iter().enumerate() {
        assert!(
            item["render"].is_null(),
            "a bare query must not render spec {i}: {}",
            item["render"]
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_prefetch_window_follows_the_cursor_as_it_advances() {
    let app = spawn_app_with(|cfg| {
        cfg.video.provider = "fake".into();
        cfg.feed.prefetch_depth = 1;
    })
    .await;
    // At the top, only spec 0 warms; spec 1 stays $0 until the cursor reaches it.
    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("?cursor=0", |b| b["items"][0]["render"].is_object())
        .await;
    assert!(
        body["items"][1]["render"].is_null(),
        "spec 1 must stay $0 while the cursor sits at 0: {}",
        body["items"][1]["render"]
    );
    // Advance the cursor — the window slides and spec 1 renders on arrival.
    app.get("/api/state?cursor=1").await;
    let body = app
        .poll_state("?cursor=1", |b| b["items"][1]["render"].is_object())
        .await;
    assert!(
        body["items"][1]["render"].is_object(),
        "spec 1 renders once the cursor reaches it"
    );
}

// --- first-run on-ramps (doomscrum-942) ------------------------------------
// In-app FAL key entry: the UI's key sheet posts here so a stranger never has
// to leave the app to enable real renders. The key is stored under the state
// dir (0600), never echoed back, and the response quotes the per-render price
// and the starter budget the wallet already enforces.
#[tokio::test]
async fn in_app_key_entry_stores_fal_key_and_quotes_price_and_budget() {
    let app = spawn_app().await;

    // Blank or whitespace keys are rejected with a recovery-grade message.
    let (code, body) = app
        .post("/api/keys", json!({"provider": "fal", "key": "   "}))
        .await;
    assert_eq!(code, 400, "{body}");

    // Unknown providers are rejected — only fal has an in-app key surface.
    let (code, body) = app
        .post(
            "/api/keys",
            json!({"provider": "openai", "key": "abcdef123456"}),
        )
        .await;
    assert_eq!(code, 400, "{body}");

    // A valid key is accepted, stored, and priced — but never echoed back.
    let fal_key = "2b8c4d9e1f0a:f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6";
    let (code, body) = app
        .post("/api/keys", json!({"provider": "fal", "key": fal_key}))
        .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["fal_configured"], true);
    assert!(
        body["price_per_render_usd"].as_f64().unwrap() > 0.0,
        "key response must quote the per-render price: {body}"
    );
    assert!(
        body["daily_cap_usd"].as_f64().unwrap() > 0.0 && body["cap_usd"].as_f64().unwrap() > 0.0,
        "key response must state the starter budget: {body}"
    );
    assert!(
        !body.to_string().contains(fal_key),
        "the key value must never be echoed back: {body}"
    );

    // Persisted under the state dir with owner-only permissions.
    let keys_path = app.root.join(".doomscrum").join("keys.json");
    let raw = std::fs::read_to_string(&keys_path).unwrap();
    assert!(raw.contains(fal_key), "key file must hold the stored key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&keys_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "keys.json must be owner-only");
    }
}

// The empty-backlog on-ramp names the exact backlog path; the UI reads the
// configured backlog dir from /api/repo instead of hardcoding "backlog.d".
#[tokio::test]
async fn repo_route_names_the_backlog_dir_for_onramp_copy() {
    let app = spawn_app().await;
    let (code, body) = app.get("/api/repo").await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["backlog_dir"], "backlog.d", "{body}");
}

// --- runtime reliability epic (doomscrum-931) --------------------------------
// Crash recovery, self-healing render lifecycle, and the durable cost ledger.

/// Mount a fal mock that answers the submit with an inline (synchronous)
/// video URL and serves the MP4 bytes.
async fn mount_fal_success(fal: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-model"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "video": { "url": format!("{}/files/out.mp4", fal.uri()) }
        })))
        .mount(fal)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/out.mp4"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(b"\x00\x00\x00\x18ftypmp42-doomscrum-test"),
        )
        .mount(fal)
        .await;
}

fn fal_test_config(cfg: &mut Config, fal_uri: String) {
    cfg.script.mode = "templates".into();
    cfg.video.provider = "fal".into();
    cfg.video.fal_model = "fal-ai/test-model".into();
    cfg.video.fal_base_url = fal_uri;
    cfg.video.price_per_second_usd = 0.15; // 8s clip → $1.20/render
}

/// Acceptance (doomscrum-931): a budget-degraded fixture upgrades to a real
/// render once it re-enters the viewport window and budget allows.
#[tokio::test(flavor = "multi_thread")]
async fn budget_degraded_fixture_upgrades_to_real_render_once_budget_allows() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let fal = MockServer::start().await;
    mount_fal_success(&fal).await;
    let app = spawn_app_with(|cfg| {
        fal_test_config(cfg, fal.uri());
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 100.0;
        cfg.feed.prefetch_depth = 1;
    })
    .await;

    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let prd_sha = state["items"][0]["prd"]["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    // Seed the pre-crash world: the wallet was exhausted, so this spec holds a
    // degraded fixture instead of a real render.
    let mut degraded = render_fixture(
        &prd_id,
        &prd_sha,
        "degraded-fixture",
        "fake-local",
        "2026-01-01T00:00:00.000Z",
    );
    degraded.degraded_reason = Some("render budget exhausted".into());
    save_render(&app.root.join(".doomscrum/renders"), &degraded).unwrap();

    // Budget is now available: the viewport poll upgrades the fixture.
    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("?cursor=0", |b| {
            b["items"][0]["render"]["provider"] == "fal"
        })
        .await;
    assert!(
        body["items"][0]["render"]["degraded_reason"].is_null(),
        "the upgraded render is real, not badged: {}",
        body["items"][0]["render"]
    );
    // Exactly one paid render: once real, the spec never re-enters the window.
    tokio::time::sleep(Duration::from_millis(200)).await;
    app.get("/api/state?cursor=0").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let submits = fal
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/fal-ai/test-model")
        .count();
    assert_eq!(submits, 1, "an upgraded spec must not re-render");
}

/// Acceptance (doomscrum-931): while the wallet is still exhausted, a
/// degraded fixture is NOT re-rendered on every poll.
#[tokio::test(flavor = "multi_thread")]
async fn degraded_fixture_is_not_rerendered_while_still_over_budget() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let app = spawn_app_with(|cfg| {
        cfg.script.mode = "templates".into();
        cfg.video.provider = "fal".into();
        cfg.video.max_total_spend_usd = 0.0; // still over budget
        cfg.feed.prefetch_depth = 1;
    })
    .await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let prd_sha = state["items"][0]["prd"]["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let mut degraded = render_fixture(
        &prd_id,
        &prd_sha,
        "degraded-fixture",
        "fake-local",
        "2026-01-01T00:00:00.000Z",
    );
    degraded.degraded_reason = Some("render budget exhausted".into());
    save_render(&app.root.join(".doomscrum/renders"), &degraded).unwrap();

    for _ in 0..3 {
        app.get("/api/state?cursor=0").await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (_, body) = app.get("/api/state?cursor=0").await;
    assert_eq!(
        body["items"][0]["render"]["id"], "degraded-fixture",
        "the fixture must not be re-rendered while over budget: {}",
        body["items"][0]["render"]
    );
    assert_eq!(
        body["cooking"],
        json!({}),
        "no render job may start while the wallet refuses an upgrade"
    );
}

/// Acceptance (doomscrum-931): a failed JIT render is retried on a later poll
/// via the bounded retry — the cooking "failed: …" key is cleared and the spec
/// re-attempted at the route level — and the failure survives in the durable
/// events ledger.
#[tokio::test(flavor = "multi_thread")]
async fn failed_jit_render_is_retried_on_a_later_poll_and_recorded_durably() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let fal = MockServer::start().await;
    // First submit blows up; every later one succeeds.
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-model"))
        .respond_with(ResponseTemplate::new(500).set_body_string("transient provider error"))
        .up_to_n_times(1)
        .mount(&fal)
        .await;
    mount_fal_success(&fal).await;

    let app = spawn_app_with(|cfg| {
        fal_test_config(cfg, fal.uri());
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 100.0;
        cfg.feed.prefetch_depth = 1;
        cfg.feed.render_retry_backoff_sec = 0; // retry on the next poll
        cfg.feed.render_max_attempts = 3;
    })
    .await;
    let (_, state) = app.get("/api/state").await;
    let prd_id = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();

    // Cursor poll starts attempt 1 (which fails). A bare query observes the
    // failed key without triggering the retry.
    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("", |b| {
            b["cooking"][&prd_id]
                .as_str()
                .is_some_and(|s| s.starts_with("failed:"))
        })
        .await;
    assert!(
        body["cooking"][&prd_id].as_str().unwrap().contains("fal"),
        "failure label carries the provider error: {}",
        body["cooking"][&prd_id]
    );
    // The failure is durable — it survives the in-memory map.
    let events = std::fs::read_to_string(app.root.join(".doomscrum/events.ndjson")).unwrap();
    assert!(
        events.contains("render_failed"),
        "render failure must append a durable event: {events}"
    );

    // The next cursor poll clears the failed key and re-attempts; the retry
    // succeeds against the now-healthy provider.
    let body = app
        .poll_state("?cursor=0", |b| {
            b["items"][0]["render"]["provider"] == "fal"
        })
        .await;
    assert!(
        body["cooking"][&prd_id].is_null(),
        "the failed cooking key must be cleared after the successful retry: {}",
        body["cooking"]
    );
}

/// Acceptance (doomscrum-931): the durable cost ledger survives the renders
/// dir being wiped — the spend meter and the wallet gate keep the truth.
#[tokio::test(flavor = "multi_thread")]
async fn cost_ledger_survives_renders_wipe_and_wallet_gate_reads_it() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let fal = MockServer::start().await;
    mount_fal_success(&fal).await;
    let app = spawn_app_with(|cfg| {
        fal_test_config(cfg, fal.uri());
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 1.8; // one $1.20 render fits, two don't
    })
    .await;
    let (_, state) = app.get("/api/state").await;
    let first = state["items"][0]["prd"]["id"].as_str().unwrap().to_string();
    let second = state["items"][1]["prd"]["id"].as_str().unwrap().to_string();

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "prd_id": first, "confirmed_cost": true }),
        )
        .await;
    assert_eq!(status, 200, "first paid render should start: {body}");
    // Wait for the detached render to land as provenance + ledger entry.
    app.poll_state("", |b| {
        b["spend"]["total_usd"].as_f64().unwrap_or(0.0) > 1.0
    })
    .await;
    let ledger_path = app.root.join(".doomscrum/costs.ndjson");
    let ledger = std::fs::read_to_string(&ledger_path).unwrap();
    assert!(
        ledger.contains("\"cost_usd\":1.2"),
        "ledger records the paid render: {ledger}"
    );

    // Wipe the renders dir — the classic meter-reset footgun.
    std::fs::remove_dir_all(app.root.join(".doomscrum/renders")).unwrap();

    let (_, body) = app.get("/api/state").await;
    assert!(
        (body["spend"]["total_usd"].as_f64().unwrap() - 1.2).abs() < 1e-9,
        "spend must survive a renders wipe: {}",
        body["spend"]
    );
    // And the wallet gate refuses as if nothing was wiped: $1.20 spent today
    // + $1.20 planned > $1.80 daily cap.
    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "prd_id": second, "confirmed_cost": true }),
        )
        .await;
    assert_eq!(
        status, 429,
        "the wallet gate must read the ledger, not surviving render JSONs: {body}"
    );
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("daily render budget"),
        "{body}"
    );
}

/// 032's mid-window accounting oracle: with a cap that affords some-but-not-
/// all of the window, the first spec renders real and the next degrades —
/// the per-iteration spend accumulation, proven at the route level.
#[tokio::test(flavor = "multi_thread")]
async fn budget_accumulates_mid_window_first_real_then_degraded() {
    std::env::set_var("FAL_API_KEY", "test-key");
    let fal = MockServer::start().await;
    mount_fal_success(&fal).await;
    let app = spawn_app_with(|cfg| {
        fal_test_config(cfg, fal.uri());
        cfg.video.max_total_spend_usd = 100.0;
        cfg.video.max_daily_spend_usd = 1.3; // affords one $1.20 render, not two
        cfg.feed.prefetch_depth = 2;
    })
    .await;

    app.get("/api/state?cursor=0").await;
    let body = app
        .poll_state("?cursor=0", |b| {
            b["items"][0]["render"].is_object() && b["items"][1]["render"].is_object()
        })
        .await;
    assert_eq!(
        body["items"][0]["render"]["provider"], "fal",
        "first windowed spec renders real: {}",
        body["items"][0]["render"]
    );
    assert_eq!(
        body["items"][1]["render"]["provider"], "fake-local",
        "second spec exceeds the remaining budget and degrades: {}",
        body["items"][1]["render"]
    );
    assert_eq!(
        body["items"][1]["render"]["degraded_reason"], "render budget exhausted",
        "{}",
        body["items"][1]["render"]
    );
}

/// Acceptance (doomscrum-931): boot reconciles dispatch status from disk — a
/// crash mid-dispatch must not leave a permanently frozen `agent_running`
/// receipt (which would also keep GC protecting its orphaned worktree).
#[tokio::test]
async fn boot_reconcile_fails_stranded_dispatches_and_appends_durable_events() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    std::fs::write(root.join("backlog.d/001-first.md"), SPECS[0].1).unwrap();

    let ctx = AppCtx::new(root.clone(), Config::default());
    let dispatcher = ctx.dispatcher();
    let prd = doomscrum::backlog::PrdSource {
        id: "prd-stranded".into(),
        sha256: "sha-stranded".into(),
        rel_path: "backlog.d/001-first.md".into(),
        abs_path: root.join("backlog.d/001-first.md"),
        title: "First Spec".into(),
        priority: 0,
        raw: SPECS[0].1.into(),
        issue_number: None,
    };
    // The pre-crash world: a receipt persisted `queued` whose driving task
    // died with the old process.
    let receipt = dispatcher
        .create(&prd, doomscrum::dispatch::DispatchKind::Implement)
        .unwrap();
    assert_eq!(receipt.status, "queued");

    // Boot reconcile: the stranded receipt flips to failed…
    let reconciled = ctx.reconcile_on_boot().unwrap();
    assert_eq!(reconciled.len(), 1);
    assert_eq!(reconciled[0].status, "failed");
    let receipts = doomscrum::dispatch::load_receipts(&ctx.dispatcher().dispatches_dir).unwrap();
    assert_eq!(receipts[0].status, "failed");
    assert!(
        receipts[0]
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("stranded"),
        "{:?}",
        receipts[0].note
    );
    // …with a durable event, and a second boot reconciles nothing.
    let events = std::fs::read_to_string(root.join(".doomscrum/events.ndjson")).unwrap();
    assert!(events.contains("dispatch_failed"), "{events}");
    assert!(ctx.reconcile_on_boot().unwrap().is_empty());
}
