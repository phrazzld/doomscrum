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
    pub estimate_usd: f64,
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

impl FalProvider {
    pub fn from_config(cfg: &crate::config::VideoConfig, api_key: String) -> Self {
        Self {
            model: cfg.fal_model.clone(),
            base_url: cfg.fal_base_url.clone(),
            api_key,
            max_duration_sec: cfg.max_duration_sec,
            estimate_usd: cfg.estimate_usd,
            poll_interval: Duration::from_secs(1),
            max_polls: 180,
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
            cost_estimate_usd: self.estimate_usd,
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
            .json(&json!({
                "prompt": storyboard.provider_prompt,
                "aspect_ratio": "9:16",
                "duration": format!("{}s", storyboard.target_duration_sec.min(self.max_duration_sec)),
                "generate_audio": true,
            }))
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
