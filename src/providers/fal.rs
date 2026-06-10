use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::distill::Storyboard;
use crate::providers::{save_render, VideoRender};
use crate::util::{now_rfc3339, sha256_hex};

/// Client for fal.ai's queue API. Generating a video sends spec-derived
/// prompt text to a remote provider — an explicit disclosure event.
pub struct FalProvider {
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub max_duration_sec: u32,
    pub price_per_second_usd: f64,
    pub poll_interval: Duration,
    pub max_polls: u32,
}

#[derive(Debug, Deserialize)]
struct QueueResponse {
    video: Option<VideoUrl>,
    videos: Option<Vec<VideoUrl>>,
    #[serde(alias = "request_id")]
    request_id: Option<String>,
    status_url: Option<String>,
    response_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VideoUrl {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: Option<String>,
}

/// Per-second prices (720p, audio on) for models we have verified on fal,
/// checked 2026-06-10. Unknown models fall back to the configured
/// `price_per_second_usd` so spend tracking never silently reads $0.
pub fn model_price_per_second(model: &str) -> Option<f64> {
    if model.contains("ltx-2.3") && model.contains("fast") {
        Some(0.04)
    } else if model.contains("ltx-2.3") {
        Some(0.06)
    } else if model.contains("veo3.1/lite") {
        Some(0.05)
    } else if model.contains("veo3.1/fast") {
        Some(0.15)
    } else if model.contains("sora-2") {
        Some(0.10)
    } else if model.contains("kling-video/v2.6/pro") {
        Some(0.14)
    } else if model.contains("seedance-2.0/fast") {
        Some(0.2419)
    } else if model.contains("seedance-2.0") {
        Some(0.3034)
    } else {
        None
    }
}

/// Snap a requested duration to the model's supported values, preferring
/// the next longer clip so the script never loses breathing room.
/// (sora-2 takes 4/8/12, kling only 5/10, seedance any 4–15, veo 4/6/8.)
pub fn clip_duration(model: &str, target: u32) -> u32 {
    let snap_up = |allowed: &[u32]| {
        allowed
            .iter()
            .copied()
            .find(|&d| d >= target)
            .unwrap_or_else(|| *allowed.last().unwrap())
    };
    if model.contains("sora-2") {
        snap_up(&[4, 8, 12])
    } else if model.contains("kling-video") {
        snap_up(&[5, 10])
    } else if model.contains("seedance") {
        target.clamp(4, 15)
    } else if model.contains("ltx-2.3") {
        snap_up(&[6, 8, 10, 12, 14, 16, 18, 20])
    } else {
        snap_up(&[4, 6, 8])
    }
}

/// Estimated cost of one render under this config: snapped clip duration ×
/// the model's per-second price. The wallet gates and /api/state both quote
/// this, so the number shown is the number billed.
pub fn unit_cost(cfg: &crate::config::VideoConfig) -> f64 {
    f64::from(clip_duration(&cfg.fal_model, cfg.max_duration_sec))
        * model_price_per_second(&cfg.fal_model).unwrap_or(cfg.price_per_second_usd)
}

/// Expected per-render cost under the configured mix: the weight-averaged
/// unit cost across the portfolio (plain unit cost when no mix is set).
pub fn avg_unit_cost(cfg: &crate::config::VideoConfig) -> f64 {
    if cfg.mix.is_empty() {
        return unit_cost(cfg);
    }
    let total: f64 = cfg.mix.iter().map(|m| f64::from(m.weight.max(1))).sum();
    cfg.mix
        .iter()
        .map(|m| {
            let mut c = cfg.clone();
            c.fal_model = m.model.clone();
            c.max_duration_sec = m.duration_sec;
            unit_cost(&c) * f64::from(m.weight.max(1))
        })
        .sum::<f64>()
        / total
}

impl FalProvider {
    pub fn from_config(cfg: &crate::config::VideoConfig, api_key: String) -> Self {
        Self {
            model: cfg.fal_model.clone(),
            base_url: cfg.fal_base_url.clone(),
            api_key,
            max_duration_sec: cfg.max_duration_sec,
            price_per_second_usd: model_price_per_second(&cfg.fal_model)
                .unwrap_or(cfg.price_per_second_usd),
            // Premium models (kling pro, seedance) can queue+render for
            // many minutes; fal charges on success, so giving up early
            // strands a billed render. 2s × 600 = 20 min ceiling.
            poll_interval: Duration::from_secs(2),
            max_polls: 600,
        }
    }

    /// Billed seconds × price: duration is snapped the same way the submit
    /// request snaps it, so the estimate matches what fal actually charges.
    pub fn render_cost(&self, storyboard: &Storyboard) -> f64 {
        f64::from(self.effective_duration(storyboard)) * self.price_per_second_usd
    }

    fn effective_duration(&self, storyboard: &Storyboard) -> u32 {
        clip_duration(
            &self.model,
            storyboard.target_duration_sec.min(self.max_duration_sec.max(
                // snap-up may legitimately exceed the configured max (kling
                // only does 5s/10s); never snap *down* below the storyboard.
                storyboard.target_duration_sec,
            )),
        )
    }

    /// Each model family has its own request schema (verified against fal's
    /// OpenAPI 2026-06-10): sora-2 wants an integer duration + resolution,
    /// kling a "5"/"10" string, seedance a "4".."15" string + resolution,
    /// and veo a "{n}s" suffixed string.
    fn request_body(&self, storyboard: &Storyboard) -> serde_json::Value {
        let duration = self.effective_duration(storyboard);
        let prompt = &storyboard.provider_prompt;
        if self.model.contains("sora-2") {
            json!({
                "prompt": prompt,
                "aspect_ratio": "9:16",
                "resolution": "720p",
                "duration": duration,
            })
        } else if self.model.contains("kling-video") {
            json!({
                "prompt": prompt,
                "aspect_ratio": "9:16",
                "duration": duration.to_string(),
                "generate_audio": true,
            })
        } else if self.model.contains("seedance") {
            json!({
                "prompt": prompt,
                "aspect_ratio": "9:16",
                "resolution": "720p",
                "duration": duration.to_string(),
                "generate_audio": true,
            })
        } else if self.model.contains("ltx-2.3") {
            json!({
                "prompt": prompt,
                "aspect_ratio": "9:16",
                "resolution": "1080p",
                "duration": duration,
                "generate_audio": true,
            })
        } else {
            json!({
                "prompt": prompt,
                "aspect_ratio": "9:16",
                "duration": format!("{duration}s"),
                "generate_audio": true,
            })
        }
    }

    pub async fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        if self.api_key.trim().is_empty() {
            bail!("FAL_API_KEY or FAL_KEY is required for real video generation");
        }
        let started = Instant::now();
        let client = reqwest::Client::new();

        let result = self.submit_and_poll(&client, storyboard).await?;
        let url = result
            .video
            .as_ref()
            .and_then(|v| v.url.clone())
            .or_else(|| {
                result
                    .videos
                    .as_ref()
                    .and_then(|vs| vs.first())
                    .and_then(|v| v.url.clone())
            })
            .ok_or_else(|| anyhow!("fal response did not contain a video URL"))?;

        let bytes = client
            .get(&url)
            .send()
            .await?
            .error_for_status()
            .context("fal video download failed")?
            .bytes()
            .await?;

        let id = sha256_hex(format!("{}:fal:{}:{}", storyboard.id, self.model, url).as_bytes());
        let dir = renders_dir.join(&storyboard.prd_sha256);
        std::fs::create_dir_all(&dir)?;
        let asset_file = format!("{id}.mp4");
        std::fs::write(dir.join(&asset_file), &bytes)?;

        let render = VideoRender {
            id: id.clone(),
            prd_id: storyboard.prd_id.clone(),
            prd_sha256: storyboard.prd_sha256.clone(),
            storyboard_id: storyboard.id.clone(),
            provider: "fal".into(),
            model: self.model.clone(),
            native_audio: true,
            status: "ready".into(),
            asset_url: format!("/media/{}/{}", storyboard.prd_sha256, asset_file),
            asset_file,
            provider_job_id: result.request_id.or(Some(url)),
            cost_estimate_usd: self.render_cost(storyboard),
            latency_ms: started.elapsed().as_millis() as u64,
            created_at: now_rfc3339(),
        };
        save_render(renders_dir, &render)?;
        Ok(render)
    }

    async fn submit_and_poll(
        &self,
        client: &reqwest::Client,
        storyboard: &Storyboard,
    ) -> Result<QueueResponse> {
        let submit_url = format!("{}/{}", self.base_url.trim_end_matches('/'), self.model);
        let response = client
            .post(&submit_url)
            .header("authorization", format!("Key {}", self.api_key))
            .json(&self.request_body(storyboard))
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(
                "fal submit failed: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        let queued: QueueResponse = response
            .json()
            .await
            .context("parsing fal submit response")?;
        let (Some(status_url), Some(response_url)) = (&queued.status_url, &queued.response_url)
        else {
            return Ok(queued); // synchronous response with the video inline
        };

        for _ in 0..self.max_polls {
            let status_response = client
                .get(status_url)
                .header("authorization", format!("Key {}", self.api_key))
                .send()
                .await?
                .error_for_status()
                .context("fal status poll failed")?;
            let status: StatusResponse = status_response.json().await?;
            match status.status.as_deref() {
                Some("COMPLETED") => {
                    let result = client
                        .get(response_url)
                        .header("authorization", format!("Key {}", self.api_key))
                        .send()
                        .await?
                        .error_for_status()
                        .context("fal result fetch failed")?;
                    let mut result: QueueResponse =
                        result.json().await.context("parsing fal result")?;
                    // The job id only appears on the submit response.
                    result.request_id = result.request_id.or_else(|| queued.request_id.clone());
                    return Ok(result);
                }
                Some("FAILED") => bail!("fal job failed"),
                _ => tokio::time::sleep(self.poll_interval).await,
            }
        }
        bail!("fal job timed out after {} polls", self.max_polls)
    }
}
