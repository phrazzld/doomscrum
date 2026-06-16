use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};

use crate::distill::Storyboard;
use crate::providers::{cache_distinct_render_id, save_render, VideoRender};
use crate::util::now_rfc3339;

/// Embedded 2s 9:16 h264+aac fixture, generated once with ffmpeg.
/// Keeps tests and offline dev deterministic with zero runtime dependencies.
const FIXTURE_MP4: &[u8] = include_bytes!("../../assets/fixture.mp4");

pub struct FakeProvider;

impl FakeProvider {
    pub fn render(&self, storyboard: &Storyboard, renders_dir: &Path) -> Result<VideoRender> {
        self.render_with_ffmpeg(storyboard, renders_dir, "ffmpeg")
    }

    fn render_with_ffmpeg(
        &self,
        storyboard: &Storyboard,
        renders_dir: &Path,
        ffmpeg: &str,
    ) -> Result<VideoRender> {
        let started = Instant::now();
        let created_at = now_rfc3339();
        let id = cache_distinct_render_id(&format!("{}:fake-local", storyboard.id));
        let dir = renders_dir.join(&storyboard.prd_sha256);
        std::fs::create_dir_all(&dir)?;
        let asset_file = format!("{id}.mp4");
        let asset_path = dir.join(&asset_file);
        let model = match write_spec_fixture(ffmpeg, storyboard, &asset_path) {
            Ok(()) => "ffmpeg-fixture",
            Err(_) => {
                std::fs::write(&asset_path, FIXTURE_MP4).with_context(|| {
                    format!("writing fallback fixture {}", asset_path.display())
                })?;
                "embedded-fixture"
            }
        };
        let render = VideoRender {
            id: id.clone(),
            prd_id: storyboard.prd_id.clone(),
            prd_sha256: storyboard.prd_sha256.clone(),
            storyboard_id: storyboard.id.clone(),
            provider: "fake-local".into(),
            model: model.into(),
            native_audio: true,
            status: "ready".into(),
            asset_url: format!("/media/{}/{}", storyboard.prd_sha256, asset_file),
            asset_file,
            caption_artifact_file: None,
            degraded_reason: None,
            provider_job_id: Some(format!("fake-{}", crate::util::short(&id))),
            cost_estimate_usd: 0.0,
            latency_ms: started.elapsed().as_millis() as u64,
            created_at,
        };
        save_render(renders_dir, &render)?;
        Ok(render)
    }
}

fn write_spec_fixture(ffmpeg: &str, storyboard: &Storyboard, output_path: &Path) -> Result<()> {
    anyhow::ensure!(
        ffmpeg_has_drawtext(ffmpeg),
        "{ffmpeg} is missing the drawtext filter"
    );
    let args = ffmpeg_args(storyboard, output_path);
    let output = Command::new(ffmpeg)
        .args(&args)
        .output()
        .with_context(|| format!("spawning {ffmpeg}"))?;
    anyhow::ensure!(
        output.status.success(),
        "ffmpeg fixture failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::metadata(output_path)
        .with_context(|| format!("checking ffmpeg fixture {}", output_path.display()))?
        .len();
    anyhow::ensure!(bytes > 0, "ffmpeg fixture was empty");
    Ok(())
}

fn ffmpeg_has_drawtext(ffmpeg: &str) -> bool {
    Command::new(ffmpeg)
        .args(["-hide_banner", "-filters"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.contains(" drawtext "))
        })
        .unwrap_or(false)
}

fn ffmpeg_args(storyboard: &Storyboard, output: &Path) -> Vec<String> {
    let title = fixture_title(storyboard);
    let format = storyboard.tone.replace('_', " ");
    let color = fixture_color(&storyboard.tone);
    let text = drawtext_text(&format!("{title}\n{format}"));
    let drawtext = format!(
        "drawtext=text='{text}':fontcolor=white:fontsize=28:x=(w-text_w)/2:y=(h-text_h)/2:box=1:boxcolor=black@0.55:boxborderw=18"
    );
    vec![
        "-y".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!("color=c={color}:s=360x640:d=2:r=30"),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "sine=frequency=440:duration=2".into(),
        "-vf".into(),
        drawtext,
        "-shortest".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:v".into(),
        "libx264".into(),
        "-c:a".into(),
        "aac".into(),
        "-movflags".into(),
        "+faststart".into(),
        output.to_string_lossy().to_string(),
    ]
}

fn fixture_title(storyboard: &Storyboard) -> String {
    storyboard
        .beats
        .first()
        .map(|beat| {
            beat.caption
                .trim_end_matches(" just entered the chat")
                .to_string()
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| crate::util::short(&storyboard.prd_sha256).to_string())
}

fn fixture_color(tone: &str) -> &'static str {
    match tone {
        "fruit_drama_v3" => "0xB91C1C",
        "genz_explainer_v3" => "0x7C3AED",
        "cryptid_vlog_v3" => "0x047857",
        "italian_brainrot_v3" => "0xC2410C",
        "street_interview_v4" => "0x0E7490",
        "infomercial_v1" => "0xCA8A04",
        _ => "0x111827",
    }
}

fn drawtext_text(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\\' | '\'' | ':' => ' ',
            '\n' => '|',
            ch if ch.is_control() => ' ',
            ch => ch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::PrdSource;
    use crate::distill::{compile_storyboard, compile_with_format, distill, BrainrotFormat};
    use crate::util::sha256_hex;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn prd_with(raw: &str, priority: usize) -> PrdSource {
        PrdSource {
            id: sha256_hex(raw.as_bytes()),
            sha256: sha256_hex(raw.as_bytes()),
            rel_path: "backlog.d/spec.md".into(),
            abs_path: PathBuf::from("backlog.d/spec.md"),
            title: raw
                .lines()
                .next()
                .unwrap_or("# Spec")
                .trim_start_matches("# ")
                .into(),
            priority,
            raw: raw.into(),
        }
    }

    #[test]
    fn writes_playable_mp4_with_provenance() {
        let raw = "# Spec\n\n## Goal\nDo a thing.\n";
        let prd = prd_with(raw, 0);
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
        assert!(["embedded-fixture", "ffmpeg-fixture"].contains(&render.model.as_str()));
    }

    #[test]
    fn repeated_fixture_renders_preserve_distinct_provenance() {
        let raw = "# Spec\n\n## Goal\nDo a thing.\n";
        let prd = prd_with(raw, 0);
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let dir = tempfile::tempdir().unwrap();

        let first = FakeProvider.render(&storyboard, dir.path()).unwrap();
        let second = FakeProvider.render(&storyboard, dir.path()).unwrap();

        assert_ne!(first.id, second.id);
        assert_ne!(first.asset_url, second.asset_url);
        let render_dir = dir.path().join(&prd.sha256);
        assert!(render_dir.join(format!("{}.json", first.id)).exists());
        assert!(render_dir.join(format!("{}.json", second.id)).exists());
        let json_count = std::fs::read_dir(render_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .count();
        assert_eq!(json_count, 2);
    }

    #[test]
    fn missing_ffmpeg_falls_back_to_embedded_fixture() {
        let raw = "# Offline Fallback\n\n## Goal\nStill render.\n";
        let prd = prd_with(raw, 0);
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let dir = tempfile::tempdir().unwrap();

        let render = FakeProvider
            .render_with_ffmpeg(&storyboard, dir.path(), "definitely-no-ffmpeg-here")
            .unwrap();

        assert_eq!(render.model, "embedded-fixture");
        let mp4 =
            std::fs::read(dir.path().join(&render.prd_sha256).join(&render.asset_file)).unwrap();
        assert_eq!(mp4, FIXTURE_MP4);
    }

    #[test]
    fn ffmpeg_args_overlay_spec_title_format_and_distinct_color() {
        let raw = "# Cache Chaos\n\n## Goal\nShow the newest render.\n";
        let prd = prd_with(raw, 0);
        let brief = distill(&prd);
        let fruit = compile_with_format(&prd, &brief, 8, BrainrotFormat::FruitDrama);
        let cryptid = compile_with_format(&prd, &brief, 8, BrainrotFormat::CryptidVlog);
        let fruit_args = ffmpeg_args(&fruit, Path::new("fruit.mp4"));
        let cryptid_args = ffmpeg_args(&cryptid, Path::new("cryptid.mp4"));
        let fruit_joined = fruit_args.join(" ");
        let cryptid_joined = cryptid_args.join(" ");

        assert!(fruit_joined.contains("Cache Chaos"));
        assert!(fruit_joined.contains("fruit drama v3"));
        assert!(fruit_joined.contains("0xB91C1C"));
        assert!(cryptid_joined.contains("cryptid vlog v3"));
        assert!(cryptid_joined.contains("0x047857"));
        assert_ne!(fruit_args, cryptid_args);
    }

    #[test]
    fn invokes_drawtext_ffmpeg_when_the_filter_is_available() {
        let dir = tempfile::tempdir().unwrap();
        let fake_ffmpeg = dir.path().join("fake-ffmpeg");
        let log = dir.path().join("ffmpeg.args");
        let mut script = std::fs::File::create(&fake_ffmpeg).unwrap();
        writeln!(
            script,
            "#!/bin/sh\nif [ \"$1\" = \"-hide_banner\" ]; then printf ' T.C drawtext V->V Draw text\\n'; exit 0; fi\nprintf '%s\\n' \"$@\" > '{}'\nlast=''\nfor arg in \"$@\"; do last=\"$arg\"; done\nprintf 'fake mp4' > \"$last\"\n",
            log.display()
        )
        .unwrap();
        drop(script);
        let mut perms = std::fs::metadata(&fake_ffmpeg).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_ffmpeg, perms).unwrap();
        let prd = prd_with("# Scripted Fixture\n\n## Goal\nCall ffmpeg.\n", 0);
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);

        let render = FakeProvider
            .render_with_ffmpeg(&storyboard, dir.path(), fake_ffmpeg.to_str().unwrap())
            .unwrap();

        assert_eq!(render.model, "ffmpeg-fixture");
        let args = std::fs::read_to_string(log).unwrap();
        assert!(args.contains("drawtext=text='Scripted Fixture|fruit drama v3'"));
        assert!(args.contains("0xB91C1C"));
    }

    #[test]
    fn ffmpeg_fixture_is_spec_specific_when_drawtext_is_available() {
        if !ffmpeg_has_drawtext("ffmpeg") {
            return;
        }
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|out| !out.status.success())
            .unwrap_or(true)
        {
            return;
        }

        let prd = prd_with("# Same Demo\n\n## Goal\nShow one thing.\n", 0);
        let brief = distill(&prd);
        let first_board = compile_with_format(&prd, &brief, 8, BrainrotFormat::FruitDrama);
        let second_board = compile_with_format(&prd, &brief, 8, BrainrotFormat::CryptidVlog);
        let dir = tempfile::tempdir().unwrap();

        let first_render = FakeProvider.render(&first_board, dir.path()).unwrap();
        let second_render = FakeProvider.render(&second_board, dir.path()).unwrap();

        assert_eq!(first_render.model, "ffmpeg-fixture");
        assert_eq!(second_render.model, "ffmpeg-fixture");
        let first_mp4 = std::fs::read(
            dir.path()
                .join(&first_render.prd_sha256)
                .join(&first_render.asset_file),
        )
        .unwrap();
        let second_mp4 = std::fs::read(
            dir.path()
                .join(&second_render.prd_sha256)
                .join(&second_render.asset_file),
        )
        .unwrap();
        assert_ne!(first_mp4, second_mp4);
    }
}
