//! Fire-and-forget Canary self-reporter. No creds => silent no-op.
//! A Canary outage never blocks, slows, or panics the host app.
//!
//! DoomScrum is a short-lived CLI (or a `serve` invocation that runs until
//! killed) — not a standing service — so there is no background check-in
//! loop: one `check_in()` per invocation, plus `report_error` on the
//! top-level failure path. Sends run off the critical path on a detached
//! thread; [`flush`] blocks (briefly, bounded by [`SEND_TIMEOUT`]) until any
//! in-flight sends finish, so a one-shot invocation's proof event reaches
//! the network before the process exits instead of racing it.

use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

const SERVICE: &str = "doomscrum"; // overridable via CANARY_SERVICE
const MONITOR: &str = "doomscrum"; // must already exist in Canary (MON-i01a9d8alhga)
const TTL_MS: u64 = 120_000;
const SEND_TIMEOUT: Duration = Duration::from_secs(3);

/// Detached send threads not yet joined by [`flush`].
static PENDING: OnceLock<Mutex<Vec<JoinHandle<()>>>> = OnceLock::new();

fn config() -> Option<(String, String)> {
    let endpoint = std::env::var("CANARY_ENDPOINT").ok()?;
    let key = std::env::var("CANARY_API_KEY")
        .or_else(|_| std::env::var("CANARY_INGEST_KEY"))
        .ok()?;
    (!endpoint.trim().is_empty() && !key.trim().is_empty())
        .then(|| (endpoint.trim_end_matches('/').to_owned(), key))
}

fn service() -> String {
    std::env::var("CANARY_SERVICE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| SERVICE.to_owned())
}

/// Report a handled or unhandled error. Safe to call anywhere; no-ops
/// without `CANARY_ENDPOINT`/`CANARY_API_KEY`.
pub fn report_error(error_class: &str, message: &str) {
    let Some((endpoint, key)) = config() else {
        return;
    };
    let environment =
        std::env::var("CANARY_ENVIRONMENT").unwrap_or_else(|_| "production".to_owned());
    let body = serde_json::json!({
        "service": service(),
        "error_class": error_class,
        "message": message.chars().take(4096).collect::<String>(),
        "severity": "error",
        "environment": environment,
    });
    spawn_send(endpoint, key, "/api/v1/errors", body);
}

/// One heartbeat for this invocation. No-ops without creds.
pub fn check_in() {
    let Some((endpoint, key)) = config() else {
        return;
    };
    let body = serde_json::json!({
        "monitor": MONITOR,
        "status": "alive",
        "summary": concat!(env!("CARGO_PKG_NAME"), " run"),
        "ttl_ms": TTL_MS,
    });
    spawn_send(endpoint, key, "/api/v1/check-ins", body);
}

/// Block until any in-flight sends finish. Each send thread is internally
/// bounded by [`SEND_TIMEOUT`] (times at most two attempts), so this call is
/// brief even on a hung network — never unbounded. Call once, right before
/// process exit, so a one-shot CLI invocation's check-in/error actually
/// reaches the network instead of losing a race with the process dying.
pub fn flush() {
    let Some(pending) = PENDING.get() else {
        return;
    };
    let handles = match pending.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => return,
    };
    for handle in handles {
        let _ = handle.join();
    }
}

fn spawn_send(endpoint: String, key: String, path: &'static str, body: serde_json::Value) {
    let Ok(handle) = std::thread::Builder::new()
        .name("canary-report".into())
        .spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(SEND_TIMEOUT))
                .build()
                .into();
            let url = format!("{endpoint}{path}");
            let auth = format!("Bearer {key}");
            for _ in 0..2 {
                // one retry, then give up silently
                let ok = agent
                    .post(&url)
                    .header("Authorization", &auth)
                    .send_json(&body)
                    .is_ok();
                if ok {
                    break;
                }
            }
        })
    else {
        return; // spawn failure is not the app's problem either
    };
    let pending = PENDING.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = pending.lock() {
        guard.push(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    // `CANARY_*` env vars are process-global; serialize every test that
    // mutates them so parallel `cargo test` threads don't race each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Bind a one-shot mock server, return its address and a receiver for
    /// the first request's (method, path, body-as-json, auth header).
    fn mock_server() -> (
        String,
        std::sync::mpsc::Receiver<(String, String, serde_json::Value, String)>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                return;
            }
            let mut method = String::new();
            let mut path = String::new();
            let mut parts = request_line.split_whitespace();
            if let Some(m) = parts.next() {
                method = m.to_string();
            }
            if let Some(p) = parts.next() {
                path = p.to_string();
            }
            let mut content_length = 0usize;
            let mut auth = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    content_length = v.trim().parse().unwrap_or(0);
                }
                if lower.starts_with("authorization:") {
                    auth = line
                        .split_once(':')
                        .map(|(_, v)| v.trim().to_string())
                        .unwrap_or_default();
                }
            }
            let mut body_bytes = vec![0u8; content_length];
            let _ = reader.read_exact(&mut body_bytes);
            let body: serde_json::Value =
                serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null);
            let mut stream = stream;
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}");
            let _ = tx.send((method, path, body, auth));
        });
        (format!("http://{addr}"), rx)
    }

    #[test]
    fn no_creds_is_a_silent_noop() {
        let _guard = lock_env();
        std::env::remove_var("CANARY_ENDPOINT");
        std::env::remove_var("CANARY_API_KEY");
        std::env::remove_var("CANARY_INGEST_KEY");
        // No thread should even spawn; flush() must return immediately.
        report_error("doomscrum.test.noop", "should not send");
        check_in();
        flush();
        assert!(config().is_none());
    }

    #[test]
    fn check_in_reaches_mock_server_with_expected_body() {
        let _guard = lock_env();
        let (endpoint, rx) = mock_server();
        std::env::set_var("CANARY_ENDPOINT", &endpoint);
        std::env::set_var("CANARY_API_KEY", "test-key");
        std::env::remove_var("CANARY_INGEST_KEY");

        check_in();
        flush();

        let (method, path, body, auth) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server should receive the check-in");
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/v1/check-ins");
        assert_eq!(auth, "Bearer test-key");
        assert_eq!(body["monitor"], "doomscrum");
        assert_eq!(body["status"], "alive");
        assert_eq!(body["ttl_ms"], 120_000);

        std::env::remove_var("CANARY_ENDPOINT");
        std::env::remove_var("CANARY_API_KEY");
    }

    #[test]
    fn report_error_reaches_mock_server_with_expected_body() {
        let _guard = lock_env();
        let (endpoint, rx) = mock_server();
        std::env::set_var("CANARY_ENDPOINT", &endpoint);
        std::env::set_var("CANARY_API_KEY", "test-key");
        std::env::remove_var("CANARY_INGEST_KEY");

        report_error("doomscrum.run.failed", "boom");
        flush();

        let (method, path, body, auth) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server should receive the error");
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/v1/errors");
        assert_eq!(auth, "Bearer test-key");
        assert_eq!(body["service"], "doomscrum");
        assert_eq!(body["error_class"], "doomscrum.run.failed");
        assert_eq!(body["message"], "boom");
        assert_eq!(body["severity"], "error");

        std::env::remove_var("CANARY_ENDPOINT");
        std::env::remove_var("CANARY_API_KEY");
    }

    #[test]
    fn dead_port_does_not_hang_or_panic() {
        let _guard = lock_env();
        // Bind then immediately drop, freeing the port with nothing
        // listening on it — connection should fail fast, not hang.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        std::env::set_var("CANARY_ENDPOINT", format!("http://{addr}"));
        std::env::set_var("CANARY_API_KEY", "test-key");
        std::env::remove_var("CANARY_INGEST_KEY");

        let started = std::time::Instant::now();
        report_error("doomscrum.test.dead_port", "should not hang");
        flush();
        // Bounded by SEND_TIMEOUT * 2 attempts + scheduling slack.
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "flush() should not hang on a dead port"
        );

        std::env::remove_var("CANARY_ENDPOINT");
        std::env::remove_var("CANARY_API_KEY");
    }

    #[test]
    fn service_override_and_environment_default_are_respected() {
        let _guard = lock_env();
        let (endpoint, rx) = mock_server();
        std::env::set_var("CANARY_ENDPOINT", &endpoint);
        std::env::set_var("CANARY_API_KEY", "test-key");
        std::env::set_var("CANARY_SERVICE", "doomscrum-custom");
        std::env::remove_var("CANARY_ENVIRONMENT");
        std::env::remove_var("CANARY_INGEST_KEY");

        report_error("doomscrum.test.class", "msg");
        flush();

        let (_, _, body, _) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(body["service"], "doomscrum-custom");
        assert_eq!(body["environment"], "production");

        std::env::remove_var("CANARY_ENDPOINT");
        std::env::remove_var("CANARY_API_KEY");
        std::env::remove_var("CANARY_SERVICE");
    }
}
