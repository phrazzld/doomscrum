//! FAL provider contract tests against a mock queue API.

use std::path::PathBuf;
use std::time::Duration;

use doomscrum::backlog::PrdSource;
use doomscrum::distill::{compile_storyboard, distill};
use doomscrum::providers::fal::FalProvider;
use doomscrum::util::sha256_hex;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(base_url: String, api_key: &str) -> FalProvider {
    FalProvider {
        model: "fal-ai/test-model".into(),
        base_url,
        api_key: api_key.into(),
        max_duration_sec: 8,
        price_per_second_usd: 0.15,
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
    // 8 billed seconds × $0.15/s
    assert!((render.cost_estimate_usd - 1.2).abs() < 1e-9);

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

/// Each model family gets the request schema fal actually accepts
/// (verified against fal's OpenAPI 2026-06-10): sora-2 integer durations,
/// kling "5"/"10" strings, seedance "4".."15" strings + resolution,
/// veo-style "{n}s" for everything else.
#[tokio::test]
async fn each_model_family_gets_its_own_request_schema() {
    let cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "fal-ai/sora-2/text-to-video",
            serde_json::json!({"duration": 8, "resolution": "720p", "aspect_ratio": "9:16"}),
        ),
        (
            "fal-ai/kling-video/v2.6/pro/text-to-video",
            // kling only does 5s/10s: an 8s target snaps UP to 10.
            serde_json::json!({"duration": "10", "generate_audio": true, "aspect_ratio": "9:16"}),
        ),
        (
            "bytedance/seedance-2.0/fast/text-to-video",
            serde_json::json!({"duration": "8", "resolution": "720p", "generate_audio": true}),
        ),
        (
            "fal-ai/veo3.1/fast",
            serde_json::json!({"duration": "8s", "generate_audio": true}),
        ),
        (
            "fal-ai/veo3.1/lite",
            serde_json::json!({"duration": "8s", "generate_audio": true, "aspect_ratio": "9:16"}),
        ),
    ];
    for (model, expected_body) in cases {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/{model}")))
            .and(body_partial_json(expected_body.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "video": { "url": format!("{}/files/out.mp4", server.uri()) }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/out.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(FAKE_MP4))
            .mount(&server)
            .await;

        let prd = sample_prd();
        let mut p = provider(server.uri(), "k");
        p.model = model.into();
        // Compile the storyboard at the duration the model will really
        // produce, exactly as the CLI/server do.
        let duration = doomscrum::providers::fal::clip_duration(model, 8);
        let storyboard = compile_storyboard(&prd, &distill(&prd), duration);
        let dir = tempfile::tempdir().unwrap();
        let render = p.render(&storyboard, dir.path()).await.unwrap_or_else(|e| {
            panic!("render failed for {model}: {e:#} (expected body {expected_body})")
        });
        assert_eq!(render.model, model);
    }
}

#[test]
fn known_models_have_verified_prices_and_durations() {
    use doomscrum::providers::fal::{clip_duration, model_price_per_second};
    assert_eq!(model_price_per_second("fal-ai/veo3.1/fast"), Some(0.15));
    // veo3.1/lite: $0.05/s at 720p with audio, verified on fal 2026-06-10.
    assert_eq!(model_price_per_second("fal-ai/veo3.1/lite"), Some(0.05));
    assert_eq!(
        model_price_per_second("fal-ai/sora-2/text-to-video"),
        Some(0.10)
    );
    assert_eq!(
        model_price_per_second("fal-ai/kling-video/v2.6/pro/text-to-video"),
        Some(0.14)
    );
    assert_eq!(
        model_price_per_second("bytedance/seedance-2.0/fast/text-to-video"),
        Some(0.2419)
    );
    assert_eq!(model_price_per_second("fal-ai/mystery-model"), None);

    assert_eq!(clip_duration("fal-ai/sora-2/text-to-video", 8), 8);
    assert_eq!(clip_duration("fal-ai/sora-2/text-to-video", 9), 12);
    assert_eq!(clip_duration("fal-ai/kling-video/v2.6/pro/text-to-video", 8), 10);
    assert_eq!(clip_duration("fal-ai/kling-video/v2.6/pro/text-to-video", 4), 5);
    assert_eq!(clip_duration("bytedance/seedance-2.0/fast/text-to-video", 8), 8);
    assert_eq!(clip_duration("bytedance/seedance-2.0/fast/text-to-video", 20), 15);
    assert_eq!(clip_duration("fal-ai/veo3.1/fast", 8), 8);
    assert_eq!(clip_duration("fal-ai/veo3.1/lite", 12), 8);
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
