pub mod fake;
pub mod fal;

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::distill::Storyboard;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_job_id: Option<String>,
    pub cost_estimate_usd: f64,
    pub latency_ms: u64,
    pub created_at: String,
}

pub enum Provider {
    Fake(fake::FakeProvider),
    Fal(fal::FalProvider),
}

impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Fake(_) => "fake-local",
            Provider::Fal(_) => "fal",
        }
    }

    pub async fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        match self {
            Provider::Fake(p) => p.render(storyboard, renders_dir),
            Provider::Fal(p) => p.render(storyboard, renders_dir).await,
        }
    }

    /// The clip length this provider will actually produce for a requested
    /// duration. Storyboards must be compiled with this value so the script's
    /// word budget and "finish by second N" pacing match the real clip.
    pub fn clip_duration(&self, target_sec: u32) -> u32 {
        match self {
            Provider::Fake(_) => target_sec,
            Provider::Fal(p) => fal::clip_duration(&p.model, target_sec),
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
    renders.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(renders)
}
