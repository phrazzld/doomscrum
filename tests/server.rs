//! End-to-end tests through the HTTP routes — the layer the UI actually
//! talks to. The previous incarnation of this project asserted dispatch
//! behavior on an inner function while the route did something else; these
//! tests exist so that cannot happen again.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use doomscrum::config::Config;
use doomscrum::server::{router, AppCtx};

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

    fn git_stdout(&self, cwd: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }
}

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

    let (status, body) = app.post("/api/generate", json!({ "provider": "fal" })).await;
    assert_eq!(status, 409, "unconfirmed fal render should stop early: {body}");
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
    assert_eq!(status, 429, "daily budget should fail before FAL key lookup: {body}");
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
        .respond_with(ResponseTemplate::new(200).set_body_bytes(
            b"\x00\x00\x00\x18ftypmp42-doomscrum-test",
        ))
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

    let (status, body) = app
        .post(
            "/api/generate",
            json!({ "provider": "fal", "prd_id": second, "confirmed_cost": true }),
        )
        .await;
    assert_eq!(status, 429, "pending first render must count against daily cap: {body}");
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
    assert_eq!(second_receipt["status"], "queued", "second receipt: {second_receipt}");

    let first_receipt = app.await_dispatch(&first_id).await;
    let second_receipt = app.await_dispatch(&second_id).await;
    assert_eq!(first_receipt["status"], "completed_local");
    assert_eq!(second_receipt["status"], "completed_local");
}

#[tokio::test(flavor = "multi_thread")]
async fn left_swipe_dispatches_shape_agent_that_edits_the_spec() {
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
    sh(&target, &["git", "config", "user.email", "t@doomscrum.local"]);
    sh(&target, &["git", "config", "user.name", "DoomScrum Test"]);
    sh(&target, &["git", "config", "commit.gpgsign", "false"]);
    sh(&target, &["git", "add", "-A"]);
    sh(&target, &["git", "commit", "-qm", "init"]);
    sh(tmp.path(), &["git", "init", "-q", "--bare", "olympus.git"]);
    sh(
        &target,
        &["git", "remote", "add", "origin", target_bare.to_str().unwrap()],
    );

    let mut cfg = Config::default();
    cfg.repo.path = target.to_string_lossy().to_string();
    cfg.agent.implement_cmd = vec![
        "sh".into(),
        "-c".into(),
        "echo done > foreign-marker.txt".into(),
    ];
    cfg.agent.pr_cmd = vec!["sh".into(), "-c".into(), "echo https://example.test/pr/7".into()];

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
    let (status, _) = app.post("/api/repo", json!({ "path": "/nope/zilch" })).await;
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
