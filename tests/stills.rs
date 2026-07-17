//! Stills pipeline contract tests against a mocked fal queue and a local
//! ffmpeg composition. No real money is spent.

use std::path::Path;

use doomscrum::backlog::PrdSource;
use doomscrum::config::{Config, MixEntry, StillsConfig, VideoConfig};
use doomscrum::distill::{compile_storyboard, distill};
use doomscrum::providers::fal::{avg_unit_cost, unit_cost};
use doomscrum::providers::stills::StillsProvider;
use doomscrum::render::ledger;
use doomscrum::render::pipeline::render_spec;
use doomscrum::server::AppCtx;
use doomscrum::util::sha256_hex;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_prd() -> PrdSource {
    let raw = "# Stills Spec\n\n## Goal\nRender cheap 9:16 clips from a single still.\n\n## Acceptance Criteria\n- 1080x1920 MP4.\n- Caption sidecar.\n";
    PrdSource {
        id: sha256_hex(raw.as_bytes()),
        sha256: sha256_hex(raw.as_bytes()),
        rel_path: "backlog.d/stills.md".into(),
        abs_path: PathBuf::from("backlog.d/stills.md"),
        title: "Stills Spec".into(),
        priority: 0,
        raw: raw.into(),
        issue_number: None,
    }
}

fn provider(base_url: String, api_key: &str) -> StillsProvider {
    StillsProvider {
        model: "stills/ken-burns".into(),
        base_url,
        api_key: api_key.into(),
        max_duration_sec: 8,
        image_model: "fal-ai/test-image".into(),
        image_price_usd: 0.03,
        tts_cmd: Vec::new(),
        poll_interval: Duration::from_millis(10),
        max_polls: 10,
        request_timeout: Duration::from_secs(5),
    }
}

const FAKE_IMAGE: &[u8] = include_bytes!("../assets/icon-192.png");

use std::path::PathBuf;
use std::time::Duration;

fn write_wav(path: &Path, seconds: f64) {
    let sample_rate = 16000u32;
    let total_samples = (sample_rate as f64 * seconds).round() as u32;
    let data_size = total_samples * 2;
    let file_size = 36 + data_size;

    let mut bytes: Vec<u8> = Vec::with_capacity((file_size + 8) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&file_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // subchunk size
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for _ in 0..total_samples {
        bytes.extend_from_slice(&0i16.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[tokio::test]
async fn image_keyframe_stage_polls_and_downloads_from_queue() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/fal-ai/test-image"))
        .and(header("authorization", "Key test-key"))
        .and(body_partial_json(serde_json::json!({
            "image_size": "portrait_16_9",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "request_id": "req-stills-1",
            "status_url": format!("{}/status/req-stills-1", server.uri()),
            "response_url": format!("{}/result/req-stills-1", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/status/req-stills-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "IN_PROGRESS"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/status/req-stills-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "COMPLETED"
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/result/req-stills-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "images": [{ "url": format!("{}/files/keyframe.png", server.uri()) }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/files/keyframe.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(FAKE_IMAGE))
        .expect(1)
        .mount(&server)
        .await;

    let prd = sample_prd();
    let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
    let dir = tempfile::tempdir().unwrap();
    let work_dir = dir.path().join("work");
    std::fs::create_dir_all(&work_dir).unwrap();

    let p = provider(server.uri(), "test-key");
    let keyframe = p.generate_keyframe(&storyboard, &work_dir).await.unwrap();

    assert!(keyframe.exists());
    assert!(keyframe
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("keyframe."));
    assert_eq!(p.model, "stills/ken-burns");
    assert!((p.image_price_usd - 0.03).abs() < 1e-9);
}

#[tokio::test]
async fn failed_render_cleans_work_dir() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-image"))
        .respond_with(ResponseTemplate::new(500).set_body_string("synthetic failure"))
        .expect(1)
        .mount(&server)
        .await;

    let prd = sample_prd();
    let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
    let dir = tempfile::tempdir().unwrap();
    let result = provider(server.uri(), "test-key")
        .render(&storyboard, dir.path())
        .await;

    assert!(
        result.is_err(),
        "the queue-stage failure must reach the caller"
    );
    let prd_dir = dir.path().join(&storyboard.prd_sha256);
    let leaked_work_dirs: Vec<_> = std::fs::read_dir(prd_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("stills-work-")
        })
        .collect();
    assert!(
        leaked_work_dirs.is_empty(),
        "failed stills render leaked scratch dirs: {leaked_work_dirs:?}"
    );
}

#[tokio::test]
async fn end_to_end_render_produces_9_16_mp4_and_captions() {
    if !ffmpeg_available() {
        eprintln!("skipping ffmpeg composition test: ffmpeg/ffprobe not on PATH");
        return;
    }

    let server = MockServer::start().await;
    mock_image_queue(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let wav_source = dir.path().join("source.wav");
    write_wav(&wav_source, 2.0);

    let old_fal_key = std::env::var("FAL_API_KEY").ok();
    std::env::set_var("FAL_API_KEY", "test-key");
    let _restore_key = RestoreFalKey(old_fal_key);

    let mut cfg = Config::default();
    cfg.video.provider = "fal".into();
    cfg.video.fal_model = "stills/ken-burns".into();
    cfg.video.fal_base_url = server.uri();
    cfg.video.stills.image_model = "fal-ai/test-image".into();
    cfg.video.stills.image_price_usd = 0.03;
    cfg.video.stills.tts_cmd = vec![
        "cp".into(),
        wav_source.to_string_lossy().into_owned(),
        "{out}".into(),
    ];
    cfg.script.mode = "templates".into();

    let root = tempfile::tempdir().unwrap();
    let ctx = AppCtx::new(root.path().into(), cfg);
    ctx.store_key("FAL_API_KEY", "test-key").unwrap();

    let prd = sample_prd();
    let render = render_spec(&ctx, "fal", &prd).await.unwrap();

    assert_eq!(render.provider, "stills");
    assert_eq!(render.model, "stills/ken-burns");
    assert!(render.native_audio);
    assert!((render.cost_estimate_usd - 0.03).abs() < 1e-9);
    assert!(render.caption_artifact_file.is_some());

    let renders_dir = ctx.renders_dir();
    let asset_path = renders_dir
        .join(&render.prd_sha256)
        .join(&render.asset_file);
    assert!(asset_path.is_file());
    let caption_path = renders_dir
        .join(&render.prd_sha256)
        .join(render.caption_artifact_file.as_ref().unwrap());
    assert!(caption_path.is_file());

    let probe = ffprobe_video(&asset_path);
    eprintln!(
        "SMOKE: {}x{} duration={:.3}s",
        probe.width, probe.height, probe.duration
    );
    assert_eq!(probe.width, 1080);
    assert_eq!(probe.height, 1920);
    // Target duration is 8s; the 2s audio + 0.5s padding is shorter.
    assert!(
        (probe.duration - 8.0).abs() < 0.5,
        "duration was {}",
        probe.duration
    );

    // The ledger records the real stills spend.
    let entries = ledger::read_all(&ctx.ledger_path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].provider, "stills");
    assert!((entries[0].cost_usd - 0.03).abs() < 1e-9);

    let artifact: doomscrum::providers::CaptionArtifact =
        serde_json::from_str(&std::fs::read_to_string(&caption_path).unwrap()).unwrap();
    assert_eq!(
        artifact.source,
        doomscrum::providers::CaptionSource::Estimated
    );
    assert!(!artifact.words.is_empty());
    // First word starts after the 200ms lead-in.
    assert!(artifact.words[0].start_ms >= 200);
}

struct RestoreFalKey(Option<String>);
impl Drop for RestoreFalKey {
    fn drop(&mut self) {
        match &self.0 {
            Some(v) => std::env::set_var("FAL_API_KEY", v),
            None => std::env::remove_var("FAL_API_KEY"),
        }
    }
}

#[tokio::test]
async fn zero_cost_fixture_render_writes_no_ledger_entry() {
    let mut cfg = Config::default();
    cfg.script.mode = "templates".into();
    let root = tempfile::tempdir().unwrap();
    let ctx = AppCtx::new(root.path().into(), cfg);
    let prd = sample_prd();

    let render = render_spec(&ctx, "fake", &prd).await.unwrap();

    assert_eq!(render.provider, "fake-local");
    assert_eq!(render.cost_estimate_usd, 0.0);
    assert!(ledger::read_all(&ctx.ledger_path()).unwrap().is_empty());
}

#[test]
fn unit_cost_quotes_image_price_for_stills_models() {
    let mut cfg = VideoConfig {
        fal_model: "stills/ken-burns".into(),
        max_duration_sec: 12,
        stills: StillsConfig {
            image_price_usd: 0.03,
            ..StillsConfig::default()
        },
        ..VideoConfig::default()
    };
    assert!((unit_cost(&cfg) - 0.03).abs() < 1e-9);

    // Mix with one stills entry and one native-video entry.
    cfg.mix = vec![
        MixEntry {
            model: "stills/ken-burns".into(),
            duration_sec: 10,
            weight: 1,
        },
        MixEntry {
            model: "fal-ai/veo3.1/lite".into(),
            duration_sec: 8,
            weight: 1,
        },
    ];
    let avg = avg_unit_cost(&cfg);
    // stills = $0.03, veo3.1/lite 8s @ $0.05/s = $0.40.
    assert!((avg - (0.03 + 0.40) / 2.0).abs() < 1e-9, "avg={avg}");
}

#[test]
fn clip_duration_for_stills_returns_target() {
    assert_eq!(
        doomscrum::providers::fal::clip_duration("stills/ken-burns", 8),
        8
    );
    assert_eq!(
        doomscrum::providers::fal::clip_duration("stills/ken-burns", 12),
        12
    );
}

async fn mock_image_queue(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/fal-ai/test-image"))
        .and(header("authorization", "Key test-key"))
        .and(body_partial_json(
            serde_json::json!({ "image_size": "portrait_16_9" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "request_id": "req-stills-e2e",
            "status_url": format!("{}/status/req-stills-e2e", server.uri()),
            "response_url": format!("{}/result/req-stills-e2e", server.uri()),
        })))
        .expect(1)
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/status/req-stills-e2e"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "COMPLETED"
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/result/req-stills-e2e"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "images": [{ "url": format!("{}/files/keyframe.png", server.uri()) }]
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/files/keyframe.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(FAKE_IMAGE))
        .mount(server)
        .await;
}

struct VideoProbe {
    width: u32,
    height: u32,
    duration: f64,
}

fn ffprobe_video(path: &Path) -> VideoProbe {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let stream = &json["streams"][0];
    VideoProbe {
        width: stream["width"].as_u64().unwrap() as u32,
        height: stream["height"].as_u64().unwrap() as u32,
        duration: stream["duration"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| stream["duration"].as_f64())
            .unwrap_or(0.0),
    }
}

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        && std::process::Command::new("ffprobe")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}
