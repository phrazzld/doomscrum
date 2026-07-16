use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::config::substitute;
use crate::distill::Storyboard;
use crate::providers::{
    cache_distinct_render_id, save_caption_artifact, save_render, CaptionArtifact, CaptionSource,
    CaptionWord, VideoRender,
};
use crate::util::{now_rfc3339, spec_seed};

/// $0.05/clip pipeline: one bespoke AI keyframe, local Ken Burns motion,
/// deterministic TTS narration, and an honest estimated caption artifact.
/// The only paid call is the single fal.ai image generation.
pub struct StillsProvider {
    /// Pipeline id drawn by the render mix, e.g. "stills/ken-burns".
    pub model: String,
    /// fal queue API base URL.
    pub base_url: String,
    /// FAL_API_KEY value.
    pub api_key: String,
    /// Requested clip length (the stills pipeline can produce any duration).
    pub max_duration_sec: u32,
    /// fal image model used for the keyframe.
    pub image_model: String,
    /// Price of one keyframe; quoted as the render cost.
    pub image_price_usd: f64,
    /// Local TTS command template with `{text}` and `{out}` placeholders.
    /// Empty = silent clip.
    pub tts_cmd: Vec<String>,
    pub poll_interval: Duration,
    pub max_polls: u32,
    pub request_timeout: Duration,
}

#[derive(Debug, Deserialize)]
struct QueueResponse {
    image: Option<Url>,
    images: Option<Vec<Url>>,
    #[serde(alias = "request_id")]
    request_id: Option<String>,
    status_url: Option<String>,
    response_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Url {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: Option<String>,
}

impl StillsProvider {
    pub fn from_config(cfg: &crate::config::VideoConfig, api_key: String) -> Self {
        Self {
            model: cfg.fal_model.clone(),
            base_url: cfg.fal_base_url.clone(),
            api_key,
            max_duration_sec: cfg.max_duration_sec,
            image_model: cfg.stills.image_model.clone(),
            image_price_usd: cfg.stills.image_price_usd,
            tts_cmd: cfg.stills.tts_cmd.clone(),
            poll_interval: Duration::from_secs(2),
            max_polls: 600,
            request_timeout: Duration::from_secs(120),
        }
    }

    pub async fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        if self.api_key.trim().is_empty() {
            bail!("FAL_API_KEY or FAL_KEY is required for stills keyframe generation");
        }

        let started = Instant::now();
        let prd_dir = renders_dir.join(&storyboard.prd_sha256);
        std::fs::create_dir_all(&prd_dir)?;
        let work_dir = prd_dir.join(format!(
            "stills-work-{}",
            cache_distinct_render_id(&storyboard.id)
        ));
        std::fs::create_dir_all(&work_dir)?;

        // 1. Bespoke keyframe image via the fal queue API.
        let keyframe = self.generate_keyframe(storyboard, &work_dir).await?;

        // 2. Deterministic TTS from the exact expected script.
        let audio = self.render_tts(storyboard, &work_dir).await?;
        let audio_duration = match &audio {
            Some(path) => ffprobe_duration(path).await.unwrap_or(0.0),
            None => 0.0,
        };
        let requested = self.max_duration_sec as f64;
        let final_duration = (audio_duration + 0.5).max(requested);

        // 3. Compose 9:16 MP4 with ffmpeg Ken Burns over the keyframe.
        let created_at = now_rfc3339();
        let id = cache_distinct_render_id(&format!("{}:stills:{}", storyboard.id, self.model));
        let asset_file = format!("{id}.mp4");
        let asset_path = prd_dir.join(&asset_file);

        self.compose(
            &keyframe,
            audio.as_deref(),
            final_duration,
            storyboard,
            &asset_path,
        )
        .await?;

        // 4. Estimated word-synced caption artifact.
        let caption_artifact_file = Some(format!("{id}.captions.json"));
        let caption_audio_ms = if audio_duration > 0.0 {
            (audio_duration * 1000.0).round() as u64
        } else {
            (final_duration * 1000.0).round() as u64
        };
        let caption_words = distribute_caption_words(&storyboard.expected_script, caption_audio_ms);
        let render = VideoRender {
            id: id.clone(),
            prd_id: storyboard.prd_id.clone(),
            prd_sha256: storyboard.prd_sha256.clone(),
            storyboard_id: storyboard.id.clone(),
            provider: "stills".into(),
            model: self.model.clone(),
            native_audio: audio.is_some(),
            status: "ready".into(),
            asset_url: format!("/media/{}/{asset_file}", storyboard.prd_sha256),
            asset_file: asset_file.clone(),
            caption_artifact_file: caption_artifact_file.clone(),
            degraded_reason: None,
            provider_job_id: None,
            cost_estimate_usd: self.image_price_usd,
            latency_ms: started.elapsed().as_millis() as u64,
            created_at,
        };
        let artifact = CaptionArtifact::new(
            CaptionSource::Estimated,
            &id,
            &storyboard.expected_script,
            &storyboard.expected_script,
            caption_words,
        );
        save_caption_artifact(renders_dir, &render, &artifact)?;
        save_render(renders_dir, &render)?;

        // Best-effort cleanup of the temporary work dir; failures are not fatal.
        let _ = std::fs::remove_dir_all(&work_dir);
        Ok(render)
    }

    /// Fetch one bespoke keyframe from the fal image queue. Exposed so tests
    /// can assert on the mocked queue submit/poll/fetch flow and prompt content.
    pub async fn generate_keyframe(
        &self,
        storyboard: &Storyboard,
        work_dir: &Path,
    ) -> Result<PathBuf> {
        let client = reqwest::Client::builder()
            .timeout(self.request_timeout)
            .build()
            .context("building fal http client")?;

        let result = self.submit_and_poll(&client, storyboard).await?;
        let url = result
            .image
            .as_ref()
            .and_then(|u| u.url.clone())
            .or_else(|| {
                result
                    .images
                    .as_ref()
                    .and_then(|imgs| imgs.first())
                    .and_then(|u| u.url.clone())
            })
            .ok_or_else(|| anyhow!("fal image response did not contain an image URL"))?;

        let bytes = client
            .get(&url)
            .header("authorization", format!("Key {}", self.api_key))
            .send()
            .await?
            .error_for_status()
            .context("fal image download failed")?
            .bytes()
            .await?;

        let ext = image_extension(&url);
        let path = work_dir.join(format!("keyframe.{ext}"));
        std::fs::write(&path, &bytes)?;
        Ok(path)
    }

    async fn submit_and_poll(
        &self,
        client: &reqwest::Client,
        storyboard: &Storyboard,
    ) -> Result<QueueResponse> {
        let submit_url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            self.image_model
        );
        let response = client
            .post(&submit_url)
            .header("authorization", format!("Key {}", self.api_key))
            .json(&self.request_body(storyboard))
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(
                "fal stills submit failed: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        let queued: QueueResponse = response
            .json()
            .await
            .context("parsing fal stills submit response")?;
        let (Some(status_url), Some(response_url)) = (&queued.status_url, &queued.response_url)
        else {
            return Ok(queued); // synchronous response with the image inline
        };

        for _ in 0..self.max_polls {
            let status_response = client
                .get(status_url)
                .header("authorization", format!("Key {}", self.api_key))
                .send()
                .await?
                .error_for_status()
                .context("fal stills status poll failed")?;
            let status: StatusResponse = status_response.json().await?;
            match status.status.as_deref() {
                Some("COMPLETED") => {
                    let result = client
                        .get(response_url)
                        .header("authorization", format!("Key {}", self.api_key))
                        .send()
                        .await?
                        .error_for_status()
                        .context("fal stills result fetch failed")?;
                    let mut result: QueueResponse =
                        result.json().await.context("parsing fal stills result")?;
                    result.request_id = result.request_id.or_else(|| queued.request_id.clone());
                    return Ok(result);
                }
                Some("FAILED") => bail!("fal stills job failed"),
                _ => tokio::time::sleep(self.poll_interval).await,
            }
        }
        bail!("fal stills job timed out after {} polls", self.max_polls)
    }

    fn request_body(&self, storyboard: &Storyboard) -> serde_json::Value {
        json!({
            "prompt": keyframe_prompt(storyboard),
            // fal image models take a preset or {width,height}; "portrait_16_9"
            // is the 9:16 vertical preset. A bare "9:16" string is rejected 422
            // (verified live 2026-07-15).
            "image_size": "portrait_16_9",
        })
    }

    async fn render_tts(
        &self,
        storyboard: &Storyboard,
        work_dir: &Path,
    ) -> Result<Option<PathBuf>> {
        if self.tts_cmd.is_empty() {
            return Ok(None);
        }
        let audio_path = work_dir.join("narration.aiff");
        let audio_path_str = audio_path
            .to_str()
            .ok_or_else(|| anyhow!("invalid audio path"))?;
        let cmd = substitute(
            &self.tts_cmd,
            &[
                ("text", &storyboard.expected_script),
                ("out", audio_path_str),
            ],
        );
        let (bin, args) = cmd.split_first().context("empty tts_cmd")?;
        let status = tokio::process::Command::new(bin)
            .args(args)
            .status()
            .await
            .with_context(|| format!("running TTS command {bin}"))?;
        if !status.success() {
            bail!("TTS command failed: {bin} exited with {status}");
        }
        Ok(Some(audio_path))
    }

    async fn compose(
        &self,
        keyframe: &Path,
        audio: Option<&Path>,
        duration_sec: f64,
        storyboard: &Storyboard,
        out: &Path,
    ) -> Result<()> {
        if !command_ok("ffmpeg") || !command_ok("ffprobe") {
            bail!("ffmpeg and ffprobe are required for the stills pipeline (brew install ffmpeg)");
        }

        let fps = 30;
        let frames = ((duration_sec * fps as f64).round() as u32).max(1);
        let seed = spec_seed(&storyboard.prd_sha256);
        let zoom_in = seed & 1 == 0;
        let pan_right = (seed >> 1) & 1 == 1;
        let pan_down = (seed >> 2) & 1 == 1;
        let zoom_min = 1.0;
        let zoom_max = 1.15;
        let zoom_delta = zoom_max - zoom_min;

        let z_expr = if zoom_in {
            format!("{zoom_min}+{zoom_delta}*in/{frames}")
        } else {
            format!("{zoom_max}-{zoom_delta}*in/{frames}")
        };
        let x_expr = if pan_right {
            format!("(in/{frames})*(iw-iw/zoom)")
        } else {
            format!("(1-in/{frames})*(iw-iw/zoom)")
        };
        let y_expr = if pan_down {
            format!("(in/{frames})*(ih-ih/zoom)")
        } else {
            format!("(1-in/{frames})*(ih-ih/zoom)")
        };
        let filter = format!(
            "scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920:(in_w-1080)/2:(in_h-1920)/2,zoompan=z='{z_expr}':x='{x_expr}':y='{y_expr}':d={frames}:s=1080x1920,format=yuv420p"
        );

        let mut ffmpeg = tokio::process::Command::new("ffmpeg");
        ffmpeg
            .arg("-y")
            .arg("-loop")
            .arg("1")
            .arg("-framerate")
            .arg("30")
            .arg("-i")
            .arg(keyframe);

        if let Some(audio) = audio {
            ffmpeg.arg("-i").arg(audio);
        }

        ffmpeg
            .arg("-vf")
            .arg(filter)
            .arg("-t")
            .arg(format!("{duration_sec}"))
            .arg("-r")
            .arg("30")
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("fast")
            .arg("-movflags")
            .arg("+faststart");

        if audio.is_some() {
            ffmpeg
                .arg("-af")
                .arg(format!("apad=whole_dur={duration_sec}"))
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("128k");
        } else {
            ffmpeg.arg("-an");
        }

        let status = ffmpeg.arg(out).status().await.context("running ffmpeg")?;
        if !status.success() {
            bail!("ffmpeg failed to compose stills clip (exit {status})");
        }
        Ok(())
    }
}

/// Derive a single keyframe prompt from the storyboard. We use the visual scene
/// description already composed in `provider_prompt` (which is seeded by the spec
/// and already speaks the spec content), strip the motion/caption header and
/// pacing/guardrail lines that are specific to native video models, and add a
/// "still image" framing so the image model renders one bespoke frame instead of
/// trying to synthesize a full video.
fn keyframe_prompt(storyboard: &Storyboard) -> String {
    let scene: String = storyboard
        .provider_prompt
        .lines()
        .skip(1) // drop the native-video header
        .take_while(|line| {
            !line.starts_with("Dialogue starts")
                && !line.starts_with("Full script")
                && !line.starts_with("Never cut off")
                && !line.starts_with("All spoken lines")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    format!(
        "Vertical 9:16 still image, hypermaximalist single frame:\n{}\nNo readable text, no captions, cinematic lighting.",
        scene
    )
}

/// Best-effort image extension from the URL; defaults to png.
fn image_extension(url: &str) -> String {
    let lower = url.to_lowercase();
    for ext in [".png", ".jpg", ".jpeg", ".webp"] {
        if lower.ends_with(ext) {
            return ext.trim_start_matches('.').to_string();
        }
    }
    "png".into()
}

/// Distribute the script's words evenly across `[200ms, audio_end_ms]`.
fn distribute_caption_words(text: &str, audio_ms: u64) -> Vec<CaptionWord> {
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let start_offset = 200_u64;
    let usable = audio_ms.saturating_sub(start_offset);
    let n = words.len() as u64;
    let word_ms = usable / n.max(1);
    words
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let i = i as u64;
            let start = start_offset + i * word_ms;
            let end = if i + 1 == n {
                audio_ms
            } else {
                start + word_ms
            };
            CaptionWord::new(w, start, end, None)
        })
        .collect()
}

async fn ffprobe_duration(path: &Path) -> Result<f64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .await
        .context("running ffprobe")?;
    if !output.status.success() {
        bail!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parsing ffprobe json")?;
    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| json["format"]["duration"].as_f64())
        .unwrap_or(0.0);
    Ok(duration)
}

fn command_ok(bin: &str) -> bool {
    std::process::Command::new(bin)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::PrdSource;
    use crate::distill::{compile_storyboard, distill};

    #[test]
    fn caption_words_span_the_available_audio_window() {
        let words = distribute_caption_words("Ship the demo now", 1200);
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].start_ms, 200);
        assert!(words[0].end_ms > words[0].start_ms);
        assert_eq!(words.last().unwrap().end_ms, 1200);
        // No placeholder confidence for estimated timings.
        assert!(words.iter().all(|w| w.confidence.is_none()));
    }

    #[test]
    fn image_extension_parses_common_urls() {
        assert_eq!(image_extension("https://x.co/a.png"), "png");
        assert_eq!(image_extension("https://x.co/a.jpg?w=1"), "png");
        assert_eq!(image_extension("https://x.co/no-ext"), "png");
    }

    #[test]
    fn keyframe_prompt_derives_from_provider_prompt_scene() {
        let prd = PrdSource {
            id: "id".into(),
            sha256: "a".repeat(64),
            rel_path: "backlog.d/t.md".into(),
            abs_path: std::path::PathBuf::new(),
            title: "Test Spec".into(),
            priority: 0,
            raw: "# Test Spec\n\n## Goal\nShip the thing.\n".into(),
            issue_number: None,
        };
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let prompt = keyframe_prompt(&storyboard);
        // The prompt is framed as a still image and strips the video header/pacing.
        assert!(prompt.starts_with("Vertical 9:16 still image"));
        assert!(prompt.contains("No readable text"));
        // It preserves the spec-derived scene content from provider_prompt.
        assert!(prompt.contains("Ship the thing"));
        assert!(!prompt.contains("native audio"));
        assert!(!prompt.contains("Dialogue starts"));
    }
}
