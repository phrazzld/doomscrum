use std::path::Path;

use anyhow::Result;

use crate::backlog;
use crate::providers::{cache_distinct_render_id, save_render, VideoRender};

/// Bootstrap sample renders from `assets/samples/` into the renders directory
/// when it is empty. Copies MP4s and creates provenance for the first N
/// backlog specs (up to the number of sample videos available).
///
/// Returns the number of specs bootstrapped (0 if samples don't exist or
/// renders already exist).
pub fn bootstrap(
    root: &Path,
    repo_path: &Path,
    backlog_dir: &str,
    max_items: usize,
    renders_dir: &Path,
) -> Result<usize> {
    let samples_dir = root.join("assets").join("samples");
    if !samples_dir.is_dir() {
        return Ok(0);
    }

    let mut sample_files: Vec<_> = std::fs::read_dir(&samples_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mp4"))
        .collect();

    if sample_files.is_empty() {
        return Ok(0);
    }

    sample_files.sort_by_key(|e| e.file_name());

    if is_renders_dir_populated(renders_dir)? {
        return Ok(0);
    }

    let prds = backlog::scan(repo_path, backlog_dir, max_items).unwrap_or_default();
    if prds.is_empty() {
        return Ok(0);
    }

    let sample_count = sample_files.len().min(prds.len());
    for i in 0..sample_count {
        let prd = &prds[i];
        let sample = &sample_files[i % sample_files.len()];
        let sample_path = sample.path();

        let id = cache_distinct_render_id(&format!("sample-{i}"));
        let dir = renders_dir.join(&prd.sha256);
        std::fs::create_dir_all(&dir)?;

        let asset_file = format!("{id}.mp4");
        let asset_path = dir.join(&asset_file);
        std::fs::copy(&sample_path, &asset_path)?;

        let model_tag = sample
            .file_name()
            .to_string_lossy()
            .strip_prefix("sample_")
            .and_then(|s| s.strip_suffix(".mp4"))
            .unwrap_or("sample")
            .to_string();

        let render = VideoRender {
            id,
            prd_id: prd.id.clone(),
            prd_sha256: prd.sha256.clone(),
            storyboard_id: crate::util::sha256_hex(format!("sample-{i}-{}", prd.id).as_bytes()),
            provider: "fake-local".into(),
            model: format!("sample-{model_tag}"),
            native_audio: true,
            status: "ready".into(),
            asset_url: format!("/media/{}/{}", prd.sha256, asset_file),
            asset_file,
            caption_artifact_file: None,
            degraded_reason: Some("sample video".into()),
            provider_job_id: None,
            cost_estimate_usd: 0.0,
            latency_ms: 0,
            created_at: "2026-07-02T00:00:00Z".into(),
        };

        save_render(renders_dir, &render)?;
    }

    Ok(sample_count)
}

fn is_renders_dir_populated(renders_dir: &Path) -> Result<bool> {
    if !renders_dir.exists() {
        return Ok(false);
    }
    let populated = std::fs::read_dir(renders_dir)?
        .filter_map(|e| e.ok())
        .any(|e| e.path().is_dir());
    Ok(populated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn no_samples_dir_returns_zero() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join("backlog.d")).unwrap();
        let mut f = std::fs::File::create(repo.join("backlog.d").join("spec.md")).unwrap();
        writeln!(f, "# Test\n\n## Goal\nDo it.\n").unwrap();
        let renders = root.path().join("renders");

        let n = bootstrap(root.path(), &repo, "backlog.d", 10, &renders).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn populated_renders_dir_returns_zero() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join("backlog.d")).unwrap();
        let mut f = std::fs::File::create(repo.join("backlog.d").join("spec.md")).unwrap();
        writeln!(f, "# Test\n\n## Goal\nDo it.\n").unwrap();

        let samples = root.path().join("assets").join("samples");
        std::fs::create_dir_all(&samples).unwrap();
        std::fs::write(samples.join("sample_test.mp4"), b"fake mp4").unwrap();

        let renders = root.path().join("renders");
        std::fs::create_dir_all(renders.join("some-sha")).unwrap();

        let n = bootstrap(root.path(), &repo, "backlog.d", 10, &renders).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn bootstraps_samples_into_empty_renders() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join("backlog.d")).unwrap();
        let mut f = std::fs::File::create(repo.join("backlog.d").join("spec_a.md")).unwrap();
        writeln!(f, "# Spec A\n\n## Goal\nFirst goal.\n").unwrap();
        let mut f = std::fs::File::create(repo.join("backlog.d").join("spec_b.md")).unwrap();
        writeln!(f, "# Spec B\n\n## Goal\nSecond goal.\n").unwrap();

        let samples = root.path().join("assets").join("samples");
        std::fs::create_dir_all(&samples).unwrap();
        std::fs::write(samples.join("sample_one.mp4"), b"fake mp4 one").unwrap();

        let renders = root.path().join("renders");

        let n = bootstrap(root.path(), &repo, "backlog.d", 10, &renders).unwrap();
        assert!(n >= 1, "should bootstrap at least one spec");

        let all = crate::providers::load_renders(&renders).unwrap();
        assert!(!all.is_empty(), "should have render provenance");
        for render in &all {
            assert_eq!(render.provider, "fake-local");
            assert!(render.model.starts_with("sample-"));
            assert_eq!(render.degraded_reason.as_deref(), Some("sample video"));
            assert_eq!(render.status, "ready");
        }
    }

    #[test]
    fn second_bootstrap_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join("backlog.d")).unwrap();
        let mut f = std::fs::File::create(repo.join("backlog.d").join("spec.md")).unwrap();
        writeln!(f, "# Test\n\n## Goal\nDo it.\n").unwrap();

        let samples = root.path().join("assets").join("samples");
        std::fs::create_dir_all(&samples).unwrap();
        std::fs::write(samples.join("sample_x.mp4"), b"mp4 content").unwrap();

        let renders = root.path().join("renders");

        let first = bootstrap(root.path(), &repo, "backlog.d", 10, &renders).unwrap();
        let second = bootstrap(root.path(), &repo, "backlog.d", 10, &renders).unwrap();

        assert!(first > 0);
        assert_eq!(second, 0, "second bootstrap should be a no-op");
    }
}
