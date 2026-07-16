pub mod fake;
pub mod fal;
pub mod samples;
pub mod stills;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::distill::Storyboard;
use crate::util::sha256_hex;

static RENDER_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Provenance for one generated MP4. The source spec stays authoritative;
/// every render points back at the spec hash and storyboard that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRender {
    pub id: String,
    pub prd_id: String,
    pub prd_sha256: String,
    pub storyboard_id: String,
    pub provider: String,
    pub model: String,
    pub native_audio: bool,
    pub status: String,
    /// Filename inside `renders/{prd_sha256}/`.
    pub asset_file: String,
    /// URL path the server exposes the MP4 at.
    pub asset_url: String,
    /// Provider-neutral word timings sidecar inside `renders/{prd_sha256}/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_artifact_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_job_id: Option<String>,
    pub cost_estimate_usd: f64,
    pub latency_ms: u64,
    pub created_at: String,
    /// Set when this render is a degraded substitute — e.g. a free fixture
    /// stood in for a real render the wallet gate refused. The feed badges
    /// this reason verbatim (e.g. "render budget exhausted").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

/// Provider-neutral source for persisted word-level caption timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptionSource {
    ForcedAlignment,
    FalWhisper,
    Deepgram,
    /// Timings are estimated from the known script length and measured
    /// audio duration. Used by the local deterministic TTS/caption path
    /// where no word-level forced alignment is available yet.
    Estimated,
}

/// Product-owned caption data. Provider payloads normalize into this shape
/// before feed UI, archived composition, or QA consume timings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionArtifact {
    pub source: CaptionSource,
    pub render_sha256: String,
    pub normalized_expected: String,
    pub normalized_observed: String,
    pub words: Vec<CaptionWord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: Option<f64>,
}

impl CaptionArtifact {
    pub fn new(
        source: CaptionSource,
        render_sha256: impl Into<String>,
        expected_text: impl AsRef<str>,
        observed_text: impl AsRef<str>,
        words: Vec<CaptionWord>,
    ) -> Self {
        Self {
            source,
            render_sha256: render_sha256.into(),
            normalized_expected: normalize_caption_text(expected_text.as_ref()),
            normalized_observed: normalize_caption_text(observed_text.as_ref()),
            words,
        }
    }

    pub fn to_srt(&self) -> String {
        self.words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                format!(
                    "{}\n{} --> {}\n{}\n",
                    index + 1,
                    format_srt_timestamp(word.start_ms),
                    format_srt_timestamp(word.end_ms),
                    word.text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl CaptionWord {
    pub fn new(
        text: impl Into<String>,
        start_ms: u64,
        end_ms: u64,
        confidence: Option<f64>,
    ) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms,
            confidence,
        }
    }
}

pub enum Provider {
    Fake(fake::FakeProvider),
    Fal(fal::FalProvider),
    Stills(stills::StillsProvider),
}

impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Fake(_) => "fake-local",
            Provider::Fal(_) => "fal",
            Provider::Stills(_) => "stills",
        }
    }

    pub async fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        match self {
            Provider::Fake(p) => p.render(storyboard, renders_dir),
            Provider::Fal(p) => p.render(storyboard, renders_dir).await,
            Provider::Stills(p) => p.render(storyboard, renders_dir).await,
        }
    }

    /// The clip length this provider will actually produce for a requested
    /// duration. Storyboards must be compiled with this value so the script's
    /// word budget and "finish by second N" pacing match the real clip.
    pub fn clip_duration(&self, target_sec: u32) -> u32 {
        match self {
            Provider::Fake(_) => target_sec,
            Provider::Fal(p) => fal::clip_duration(&p.model, target_sec),
            // Stills supports any duration — it simply renders the requested clip.
            Provider::Stills(_) => target_sec,
        }
    }
}

pub fn save_render(renders_dir: &Path, render: &VideoRender) -> Result<()> {
    let dir = renders_dir.join(&render.prd_sha256);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", render.id));
    std::fs::write(&path, serde_json::to_string_pretty(render)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn caption_artifact_file(render: &VideoRender) -> String {
    render
        .caption_artifact_file
        .clone()
        .unwrap_or_else(|| format!("{}.captions.json", render.id))
}

pub fn save_caption_artifact(
    renders_dir: &Path,
    render: &VideoRender,
    artifact: &CaptionArtifact,
) -> Result<()> {
    let dir = renders_dir.join(&render.prd_sha256);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(caption_artifact_file(render));
    std::fs::write(&path, serde_json::to_string_pretty(artifact)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn load_caption_artifact(
    renders_dir: &Path,
    render: &VideoRender,
) -> Result<Option<CaptionArtifact>> {
    let path = renders_dir
        .join(&render.prd_sha256)
        .join(caption_artifact_file(render));
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))
            .map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// URL path the feed can fetch this render's caption artifact at, if the
/// sidecar actually exists on disk. Served by the same `/media` route as the
/// MP4 so LAN/phone clients reach it wherever they reach the video.
pub fn caption_artifact_url(renders_dir: &Path, render: &VideoRender) -> Option<String> {
    let file = caption_artifact_file(render);
    renders_dir
        .join(&render.prd_sha256)
        .join(&file)
        .is_file()
        .then(|| format!("/media/{}/{}", render.prd_sha256, file))
}

pub(crate) fn cache_distinct_render_id(seed: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = RENDER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let digest = sha256_hex(format!("{seed}:{nanos}:{counter}").as_bytes());
    format!("{nanos:020}-{counter:06x}-{}", &digest[..12])
}

/// Freshness ordering for render provenance. `created_at` is the primary key;
/// render id is a deterministic tie-breaker for same-millisecond writes.
pub fn compare_render_freshness(a: &VideoRender, b: &VideoRender) -> std::cmp::Ordering {
    a.created_at
        .cmp(&b.created_at)
        .then_with(|| a.id.cmp(&b.id))
}

pub fn normalize_caption_text(text: &str) -> String {
    let mut out = String::new();
    let mut last_was_space = true;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn format_srt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// All render provenance files, newest first.
pub fn load_renders(renders_dir: &Path) -> Result<Vec<VideoRender>> {
    let mut renders = Vec::new();
    let entries = match std::fs::read_dir(renders_dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(renders),
        Err(err) => return Err(err.into()),
    };
    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.path().is_dir() {
            continue;
        }
        for file in std::fs::read_dir(entry.path())?.filter_map(|e| e.ok()) {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(render) = serde_json::from_str::<VideoRender>(&raw) {
                        renders.push(render);
                    }
                }
            }
        }
    }
    renders.sort_by(|a, b| compare_render_freshness(b, a));
    Ok(renders)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_artifact_serializes_provider_neutral_word_timings() {
        let artifact = CaptionArtifact::new(
            CaptionSource::FalWhisper,
            "render-sha",
            "Hello, cache chaos!",
            "hello cache chaos",
            vec![
                CaptionWord::new("Hello", 120, 420, Some(0.96)),
                CaptionWord::new("cache", 430, 760, None),
            ],
        );

        let json = serde_json::to_string_pretty(&artifact).unwrap();

        assert!(json.contains("\"source\": \"fal_whisper\""));
        assert!(json.contains("\"render_sha256\": \"render-sha\""));
        assert!(json.contains("\"normalized_expected\": \"hello cache chaos\""));
        assert!(json.contains("\"normalized_observed\": \"hello cache chaos\""));
        assert!(json.contains("\"confidence\": 0.96"));
        assert!(json.contains("\"confidence\": null"));

        let roundtrip: CaptionArtifact = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.source, CaptionSource::FalWhisper);
        assert_eq!(roundtrip.words[1].text, "cache");
        assert_eq!(roundtrip.words[1].confidence, None);
    }

    #[test]
    fn caption_artifact_represents_supported_sources_without_provider_payloads() {
        for source in [
            CaptionSource::ForcedAlignment,
            CaptionSource::FalWhisper,
            CaptionSource::Deepgram,
        ] {
            let artifact =
                CaptionArtifact::new(source, "render-sha", "", "Aligned words", Vec::new());
            let value = serde_json::to_value(&artifact).unwrap();
            assert!(value.get("provider_job_id").is_none());
            assert!(value.get("chunks").is_none());
            assert!(value.get("results").is_none());
        }
    }

    #[test]
    fn caption_artifact_round_trips_into_srt() {
        let artifact = CaptionArtifact::new(
            CaptionSource::Deepgram,
            "render-sha",
            "Ship the demo",
            "ship the demo",
            vec![
                CaptionWord::new("Ship", 0, 500, Some(0.91)),
                CaptionWord::new("the", 500, 700, Some(0.89)),
                CaptionWord::new("demo", 700, 1300, Some(0.94)),
            ],
        );

        assert_eq!(
            artifact.to_srt(),
            "1\n00:00:00,000 --> 00:00:00,500\nShip\n\n2\n00:00:00,500 --> 00:00:00,700\nthe\n\n3\n00:00:00,700 --> 00:00:01,300\ndemo\n"
        );
    }

    #[test]
    fn caption_artifact_url_present_only_when_sidecar_exists() {
        let dir = tempfile::tempdir().unwrap();
        let render = VideoRender {
            id: "render-1".into(),
            prd_id: "prd-1".into(),
            prd_sha256: "sha-1".into(),
            storyboard_id: "storyboard-1".into(),
            provider: "fal".into(),
            model: "fal-ai/veo3.1/lite".into(),
            native_audio: true,
            status: "ready".into(),
            asset_file: "render-1.mp4".into(),
            asset_url: "/media/sha-1/render-1.mp4".into(),
            caption_artifact_file: None,
            degraded_reason: None,
            provider_job_id: None,
            cost_estimate_usd: 0.1,
            latency_ms: 10,
            created_at: "2026-06-16T00:00:00Z".into(),
        };
        assert_eq!(caption_artifact_url(dir.path(), &render), None);

        let artifact = CaptionArtifact::new(
            CaptionSource::FalWhisper,
            "render-sha",
            "Expected",
            "Observed",
            vec![CaptionWord::new("Observed", 100, 900, None)],
        );
        save_caption_artifact(dir.path(), &render, &artifact).unwrap();
        assert_eq!(
            caption_artifact_url(dir.path(), &render).as_deref(),
            Some("/media/sha-1/render-1.captions.json")
        );
    }

    #[test]
    fn caption_artifact_saves_next_to_render_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let render = VideoRender {
            id: "render-1".into(),
            prd_id: "prd-1".into(),
            prd_sha256: "sha-1".into(),
            storyboard_id: "storyboard-1".into(),
            provider: "fal".into(),
            model: "fal-ai/test".into(),
            native_audio: true,
            status: "ready".into(),
            asset_file: "render-1.mp4".into(),
            asset_url: "/media/sha-1/render-1.mp4".into(),
            caption_artifact_file: Some("captions.json".into()),
            degraded_reason: None,
            provider_job_id: Some("job-1".into()),
            cost_estimate_usd: 0.1,
            latency_ms: 10,
            created_at: "2026-06-16T00:00:00Z".into(),
        };
        let artifact = CaptionArtifact::new(
            CaptionSource::ForcedAlignment,
            "render-sha",
            "Expected",
            "Observed",
            vec![CaptionWord::new("Observed", 100, 900, None)],
        );

        save_caption_artifact(dir.path(), &render, &artifact).unwrap();
        save_render(dir.path(), &render).unwrap();

        let saved =
            std::fs::read_to_string(dir.path().join("sha-1").join("captions.json")).unwrap();
        assert!(saved.contains("\"source\": \"forced_alignment\""));
        assert!(saved.contains("\"render_sha256\": \"render-sha\""));
        assert_eq!(
            load_caption_artifact(dir.path(), &render).unwrap(),
            Some(artifact)
        );
        let renders = load_renders(dir.path()).unwrap();
        assert_eq!(renders.len(), 1);
        assert_eq!(
            renders[0].caption_artifact_file.as_deref(),
            Some("captions.json")
        );
    }
}
