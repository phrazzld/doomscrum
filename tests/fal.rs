//! FAL provider contract tests against a mock queue API.

use std::path::PathBuf;
use std::time::Duration;

use specifi::backlog::PrdSource;
use specifi::distill::{compile_storyboard, distill};
use specifi::providers::fal::FalProvider;
use specifi::util::sha256_hex;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(base_url: String, api_key: &str) -> FalProvider {
    FalProvider {
        model: "fal-ai/test-model".into(),
        base_url,
        api_key: api_key.into(),
        max_duration_sec: 8,
        estimate_usd: 1.5,
        poll_interval: Duration::from_millis(10),
        max_polls: 10,
    }
}

fn sample_prd() -> PrdSource {
    let raw = "# Wired Spec\n\n## Goal\nGenerate real video.\n\n## Acceptance Criteria\n- MP4 with audio.\n";
    PrdSource {
        id: sha256_hex(raw.as_bytes()),
        sha256: sha256_hex(raw.as_bytes()),
        rel_path: "backlog.d/wired.md".into(),
        abs_path: PathBuf::from("backlog.d/wired.md"),
        title: "Wired Spec".into(),
        priority: 0,
        raw: raw.into(),
    }
}

const FAKE_MP4: &[u8] = b"\x00\x00\x00\x18ftypmp42-not-a-real-movie-but-close-enough";

#[tokio::test]
async fn renders_through_queue_poll_and_download() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/fal-ai/test-model"))
        .and(header("authorization", "Key test-key"))
        .and(body_partial_json(serde_json::json!({
            "aspect_ratio": "9:16",
            "generate_audio": true,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "request_id": "req-1",
            "status_url": format!("{}/status/req-1", server.uri()),
            "response_url": format!("{}/result/req-1", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // First poll in progress, then completed.
    Mock::given(method("GET"))
        .and(path("/status/req-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "IN_PROGRESS"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/status/req-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "COMPLETED"
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/result/req-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "video": { "url": format!("{}/files/out.mp4", server.uri()) }
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/files/out.mp4"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(FAKE_MP4))
        .expect(1)
        .mount(&server)
        .await;

    let prd = sample_prd();
    let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
    let dir = tempfile::tempdir().unwrap();
    let render = provider(server.uri(), "test-key")
        .render(&storyboard, dir.path())
        .await
        .unwrap();

    assert_eq!(render.provider, "fal");
    assert_eq!(render.model, "fal-ai/test-model");
    assert_eq!(render.status, "ready");
    assert!(render.native_audio);
    assert_eq!(render.provider_job_id.as_deref(), Some("req-1"));
    assert_eq!(render.prd_sha256, prd.sha256);

    let asset =
        std::fs::read(dir.path().join(&render.prd_sha256).join(&render.asset_file)).unwrap();
    assert_eq!(asset, FAKE_MP4);
    let provenance = std::fs::read_to_string(
        dir.path()
            .join(&render.prd_sha256)
            .join(format!("{}.json", render.id)),
    )
    .unwrap();
    assert!(provenance.contains("fal-ai/test-model"));
}

#[tokio::test]
async fn missing_api_key_is_a_clear_error() {
    let prd = sample_prd();
    let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
    let dir = tempfile::tempdir().unwrap();
    let err = provider("http://127.0.0.1:1".into(), "  ")
        .render(&storyboard, dir.path())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("FAL_API_KEY"), "err: {err}");
}

#[tokio::test]
async fn failed_job_surfaces_provider_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-model"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status_url": format!("{}/status/x", server.uri()),
            "response_url": format!("{}/result/x", server.uri()),
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/status/x"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "FAILED"
        })))
        .mount(&server)
        .await;

    let prd = sample_prd();
    let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
    let dir = tempfile::tempdir().unwrap();
    let err = provider(server.uri(), "k")
        .render(&storyboard, dir.path())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("fal job failed"), "err: {err}");
}
