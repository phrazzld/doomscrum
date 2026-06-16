//! Browser-level smoke for the DoomScrum swipe surface.
//!
//! This intentionally launches the real HTML in Chrome and drives pointer
//! gestures. The stub agent commands come from a temp doomscrum.toml so this
//! cannot accidentally inherit a real operator agent from the test process.

use std::ffi::OsStr;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use doomscrum::config::Config;
use doomscrum::server::{router, AppCtx};
use headless_chrome::{Browser, LaunchOptions, Tab};
use serde_json::{json, Value};

struct TestApp {
    addr: SocketAddr,
    root: PathBuf,
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

async fn spawn_browser_app() -> TestApp {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    let bare = tmp.path().join("origin.git");
    std::fs::create_dir_all(root.join("backlog.d")).unwrap();
    std::fs::write(
        root.join("backlog.d/001-first.md"),
        "# First Spec\n\n## Goal\nShip the first thing.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("backlog.d/002-second.md"),
        "# Second Spec\n\n## Goal\nShip the second thing.\n",
    )
    .unwrap();

    std::fs::write(
        root.join("doomscrum.toml"),
        r#"
[repo]
path = "."
backlog_dir = "backlog.d"
state_dir = ".doomscrum"

[video]
provider = "fake"

[script]
mode = "templates"

[agent]
implement_cmd = ["sh", "-c", "echo implemented > impl-marker.txt"]
shape_cmd = ["sh", "-c", "printf '\n## Notes\nsharpened by agent\n' >> {spec_path}"]
pr_cmd = ["sh", "-c", "echo https://example.test/pr/browser-e2e"]
open_pr = true
max_concurrent_dispatches = 1
"#,
    )
    .unwrap();

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

    let cfg = Config::load(&root).unwrap();
    assert_eq!(
        cfg.agent.implement_cmd,
        ["sh", "-c", "echo implemented > impl-marker.txt"],
        "browser e2e must use the config-file stub agent command"
    );

    let ctx = AppCtx::new(root.clone(), cfg);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router(ctx)).await.unwrap();
    });

    TestApp {
        addr,
        root,
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

    async fn await_implemented(&self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let (_, body) = self.get("/api/dispatches").await;
            if let Some(receipt) = body["dispatches"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|d| d["kind"] == "implement")
            {
                let status = receipt["status"].as_str().unwrap_or_default();
                if ["pr_opened", "completed_local", "failed"].contains(&status) {
                    return receipt.clone();
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("implement dispatch did not reach a terminal status");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_gestures_cover_skip_overlay_and_dispatch() {
    let app = spawn_browser_app().await;
    let (status, body) = app
        .post("/api/generate", json!({ "provider": "fake" }))
        .await;
    assert_eq!(status, 200, "fixture generation failed: {body}");

    let browser = launch_browser();
    let tab = browser.new_tab().unwrap();
    tab.set_default_timeout(Duration::from_secs(10));
    tab.navigate_to(&app.url("/"))
        .unwrap()
        .wait_until_navigated()
        .unwrap();

    tab.wait_for_element("#splash").unwrap().click().unwrap();
    wait_for_js(&tab, "Boolean(document.querySelector('#card video'))");

    pointer_swipe(&tab, "#card", 0, -160);
    wait_until_status(&app, 0, "skipped").await;
    wait_for_js(
        &tab,
        "document.querySelector('#card .meme')?.textContent.includes('Second Spec')",
    );

    pointer_tap(&tab, "#card");
    wait_for_js(
        &tab,
        "document.querySelector('#overlay.show #specRaw')?.textContent.includes('# Second Spec')",
    );
    let raw = js_string(&tab, "document.querySelector('#specRaw').textContent");
    assert!(
        raw.contains("# Second Spec") && raw.contains("Ship the second thing."),
        "overlay raw spec: {raw}"
    );

    tab.wait_for_element("#overlayClose")
        .unwrap()
        .click()
        .unwrap();
    wait_for_js(&tab, "!document.querySelector('#overlay.show')");
    pointer_swipe(&tab, "#card", 160, 0);

    let receipt = app.await_implemented().await;
    assert!(
        ["pr_opened", "completed_local"].contains(&receipt["status"].as_str().unwrap_or_default()),
        "receipt: {receipt}"
    );
    assert_eq!(receipt["status"], "pr_opened", "receipt: {receipt}");
    assert_eq!(
        receipt["pr_url"], "https://example.test/pr/browser-e2e",
        "receipt: {receipt}"
    );
    assert!(std::fs::read_dir(app.root.join(".doomscrum/dispatches"))
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".json")));
}

async fn wait_until_status(app: &TestApp, index: usize, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let (_, state) = app.get("/api/state").await;
        if state["items"][index]["status"] == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let (_, state) = app.get("/api/state").await;
    panic!("item {index} did not reach status {expected}: {state}");
}

fn launch_browser() -> Browser {
    let no_sandbox = OsStr::new("--no-sandbox");
    let disable_setuid = OsStr::new("--disable-setuid-sandbox");
    let mut builder = LaunchOptions::default_builder();
    builder
        .headless(true)
        .sandbox(false)
        .window_size(Some((390, 844)))
        .args(vec![no_sandbox, disable_setuid]);
    if let Some(path) = chrome_path() {
        builder.path(Some(path));
    }
    Browser::new(builder.build().expect("building Chrome launch options"))
        .expect("launching headless Chrome for browser e2e")
}

fn chrome_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CHROME").map(PathBuf::from) {
        return Some(path);
    }
    let mac_chrome = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if mac_chrome.exists() {
        return Some(mac_chrome);
    }
    [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ]
    .iter()
    .find_map(|name| {
        Command::new("sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
            .filter(|path| !path.as_os_str().is_empty())
    })
}

fn pointer_tap(tab: &Tab, selector: &str) {
    dispatch_pointer(tab, selector, 0, 0);
}

fn pointer_swipe(tab: &Tab, selector: &str, dx: i32, dy: i32) {
    dispatch_pointer(tab, selector, dx, dy);
}

fn dispatch_pointer(tab: &Tab, selector: &str, dx: i32, dy: i32) {
    let script = format!(
        r#"
(() => {{
  const el = document.querySelector({selector:?});
  if (!el) throw new Error("missing " + {selector:?});
  const r = el.getBoundingClientRect();
  const x = r.left + r.width / 2;
  const y = r.top + r.height / 2;
  const init = (type, clientX, clientY) => new PointerEvent(type, {{
    bubbles: true,
    cancelable: true,
    pointerId: 1,
    pointerType: "touch",
    isPrimary: true,
    clientX,
    clientY,
  }});
  el.dispatchEvent(init("pointerdown", x, y));
  if ({dx} !== 0 || {dy} !== 0) el.dispatchEvent(init("pointermove", x + {dx}, y + {dy}));
  el.dispatchEvent(init("pointerup", x + {dx}, y + {dy}));
  return true;
}})()
"#
    );
    tab.evaluate(&script, false).unwrap();
}

fn wait_for_js(tab: &Tab, expression: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let value = tab.evaluate(expression, false).unwrap();
        if value.value == Some(Value::Bool(true)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("condition did not become true: {expression}");
}

fn js_string(tab: &Tab, expression: &str) -> String {
    tab.evaluate(expression, false)
        .unwrap()
        .value
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}
