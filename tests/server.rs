//! End-to-end tests through the HTTP routes — the layer the UI actually
//! talks to. The previous incarnation of this project asserted dispatch
//! behavior on an inner function while the route did something else; these
//! tests exist so that cannot happen again.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
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
