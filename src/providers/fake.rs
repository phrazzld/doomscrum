use std::path::Path;
use std::time::Instant;

use anyhow::Result;

use crate::distill::Storyboard;
use crate::providers::{save_render, VideoRender};
use crate::util::{now_rfc3339, sha256_hex};

/// Embedded 2s 9:16 h264+aac fixture, generated once with ffmpeg.
/// Keeps tests and offline dev deterministic with zero runtime dependencies.
const FIXTURE_MP4: &[u8] = include_bytes!("../../assets/fixture.mp4");

pub struct FakeProvider;

impl FakeProvider {
    pub fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        let started = Instant::now();
        let id = sha256_hex(format!("{}:fake-local", storyboard.id).as_bytes());
        let dir = renders_dir.join(&storyboard.prd_sha256);
        std::fs::create_dir_all(&dir)?;
        let asset_file = format!("{id}.mp4");
        std::fs::write(dir.join(&asset_file), FIXTURE_MP4)?;
        let render = VideoRender {
            id: id.clone(),
            prd_id: storyboard.prd_id.clone(),
            prd_sha256: storyboard.prd_sha256.clone(),
            storyboard_id: storyboard.id.clone(),
            provider: "fake-local".into(),
            model: "embedded-fixture".into(),
            native_audio: true,
            status: "ready".into(),
            asset_url: format!("/media/{}/{}", storyboard.prd_sha256, asset_file),
            asset_file,
            provider_job_id: Some(format!("fake-{}", crate::util::short(&id))),
            cost_estimate_usd: 0.0,
            latency_ms: started.elapsed().as_millis() as u64,
            created_at: now_rfc3339(),
        };
        save_render(renders_dir, &render)?;
        Ok(render)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::PrdSource;
    use crate::distill::{compile_storyboard, distill};
    use std::path::PathBuf;

    #[test]
    fn writes_playable_mp4_with_provenance() {
        let raw = "# Spec\n\n## Goal\nDo a thing.\n";
        let prd = PrdSource {
            id: sha256_hex(raw.as_bytes()),
            sha256: sha256_hex(raw.as_bytes()),
            rel_path: "backlog.d/spec.md".into(),
            abs_path: PathBuf::from("backlog.d/spec.md"),
            title: "Spec".into(),
            priority: 0,
            raw: raw.into(),
        };
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let dir = tempfile::tempdir().unwrap();
        let render = FakeProvider.render(&storyboard, dir.path()).unwrap();

        assert_eq!(render.status, "ready");
        assert!(render.native_audio);
        let mp4 =
            std::fs::read(dir.path().join(&render.prd_sha256).join(&render.asset_file)).unwrap();
        assert!(mp4.len() > 10_000, "fixture should be a real MP4");
        assert_eq!(&mp4[4..8], b"ftyp", "MP4 container signature");
        let provenance = std::fs::read_to_string(
            dir.path()
                .join(&render.prd_sha256)
                .join(format!("{}.json", render.id)),
        )
        .unwrap();
        assert!(provenance.contains(&prd.sha256));
        assert!(provenance.contains("fake-local"));
    }
}
