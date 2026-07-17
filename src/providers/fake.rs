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
        self.render_with_ffmpeg_reason(storyboard, renders_dir, "ffmpeg", None)
    }

    pub fn render_degraded(
        &self,
        storyboard: &Storyboard,
        renders_dir: &Path,
        reason: &str,
    ) -> Result<VideoRender> {
        self.render_with_ffmpeg_reason(storyboard, renders_dir, "ffmpeg", Some(reason))
    }

    #[cfg(test)]
    fn render_with_ffmpeg(
        &self,
        storyboard: &Storyboard,
        renders_dir: &Path,
        ffmpeg: &str,
    ) -> Result<VideoRender> {
        self.render_with_ffmpeg_reason(storyboard, renders_dir, ffmpeg, None)
    }

    fn render_with_ffmpeg_reason(
        &self,
        storyboard: &Storyboard,
        renders_dir: &Path,
        ffmpeg: &str,
        final_reason: Option<&str>,
    ) -> Result<VideoRender> {
        let started = Instant::now();
        let created_at = now_rfc3339();
        let id = cache_distinct_render_id(&format!("{}:fake-local", storyboard.id));
        let dir = renders_dir.join(&storyboard.prd_sha256);
        std::fs::create_dir_all(&dir)?;
        let asset_file = format!("{id}.mp4");
        let asset_path = dir.join(&asset_file);
        let (model, degraded_reason) = match write_spec_fixture(ffmpeg, storyboard, &asset_path) {
            Ok(()) => ("ffmpeg-fixture", None),
            Err(error) => {
                std::fs::write(&asset_path, FIXTURE_MP4).with_context(|| {
                    format!("writing fallback fixture {}", asset_path.display())
                })?;
                (
                    "embedded-fixture",
                    Some(format!("spec-branded free preview unavailable: {error:#}")),
                )
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
            degraded_reason: final_reason.map(str::to_owned).or(degraded_reason),
            provider_job_id: Some(format!("fake-{}", crate::util::short(&id))),
            cost_estimate_usd: 0.0,
            latency_ms: started.elapsed().as_millis() as u64,
            created_at,
        };
        save_render(renders_dir, &render)?;
        Ok(render)
    }
}

const FRAME_WIDTH: u32 = 360;
const FRAME_HEIGHT: u32 = 640;
const FRAME_RATE: u32 = 30;
const FRAME_DURATION_SEC: u32 = 2;
const FONT_SCALE: u32 = 2;
const MAX_DISPLAY_CHARS: usize = 160;
const MAX_LINES_PER_SECTION: usize = 5;

fn write_spec_fixture(ffmpeg: &str, storyboard: &Storyboard, output_path: &Path) -> Result<()> {
    let frame_path = output_path.with_extension("ppm");
    let result = (|| {
        std::fs::write(&frame_path, spec_frame_ppm(storyboard))
            .with_context(|| format!("writing spec preview frame {}", frame_path.display()))?;
        let args = ffmpeg_args(&frame_path, output_path);
        let output = Command::new(ffmpeg)
            .args(&args)
            .output()
            .with_context(|| format!("spawning {ffmpeg}"))?;
        anyhow::ensure!(
            output.status.success(),
            "ffmpeg spec preview failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        validate_encoded_asset(output_path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&frame_path);
    result
}

fn validate_encoded_asset(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading encoded preview {}", path.display()))?;
    anyhow::ensure!(bytes.len() > 8, "encoded preview was empty");
    anyhow::ensure!(&bytes[4..8] == b"ftyp", "encoded preview was not an MP4");

    match Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
        ])
        .arg(path)
        .output()
    {
        Ok(probe) => {
            anyhow::ensure!(
                probe.status.success(),
                "ffprobe rejected encoded preview: {}",
                String::from_utf8_lossy(&probe.stderr)
            );
            let dimensions = String::from_utf8_lossy(&probe.stdout);
            anyhow::ensure!(
                dimensions
                    .lines()
                    .any(|line| line.trim() == format!("{FRAME_WIDTH}x{FRAME_HEIGHT}")),
                "encoded preview dimensions were not {FRAME_WIDTH}x{FRAME_HEIGHT}: {dimensions}"
            );

            let duration = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "format=duration",
                    "-of",
                    "default=noprint_wrappers=1:nokey=1",
                ])
                .arg(path)
                .output()
                .context("reading encoded preview duration")?;
            anyhow::ensure!(
                duration.status.success(),
                "ffprobe could not read encoded preview duration: {}",
                String::from_utf8_lossy(&duration.stderr)
            );
            let seconds = String::from_utf8_lossy(&duration.stdout)
                .trim()
                .parse::<f64>()
                .context("encoded preview duration was not numeric")?;
            anyhow::ensure!(
                (FRAME_DURATION_SEC as f64 - 0.25..=FRAME_DURATION_SEC as f64 + 0.25)
                    .contains(&seconds),
                "encoded preview duration was {seconds:.3}s, expected about {FRAME_DURATION_SEC}s"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("spawning ffprobe for encoded preview"),
    }
    Ok(())
}

fn ffmpeg_args(frame: &Path, output: &Path) -> Vec<String> {
    vec![
        "-y".into(),
        "-loop".into(),
        "1".into(),
        "-framerate".into(),
        FRAME_RATE.to_string(),
        "-i".into(),
        frame.to_string_lossy().to_string(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "sine=frequency=440:duration=2".into(),
        "-t".into(),
        FRAME_DURATION_SEC.to_string(),
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

fn spec_frame_ppm(storyboard: &Storyboard) -> Vec<u8> {
    let rgb = spec_frame(storyboard);
    let header = format!("P6\n{} {}\n255\n", FRAME_WIDTH, FRAME_HEIGHT);
    let mut ppm = Vec::with_capacity(header.len() + rgb.len());
    ppm.extend_from_slice(header.as_bytes());
    ppm.extend_from_slice(&rgb);
    ppm
}

fn spec_frame(storyboard: &Storyboard) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_WIDTH as usize * FRAME_HEIGHT as usize * 3];
    let (base_r, base_g, base_b) = spec_color(&storyboard.prd_sha256);
    for y in 0..FRAME_HEIGHT {
        let shade = (y * 18 / FRAME_HEIGHT) as u8;
        for x in 0..FRAME_WIDTH {
            let i = ((y * FRAME_WIDTH + x) * 3) as usize;
            frame[i] = base_r.saturating_add(shade);
            frame[i + 1] = base_g.saturating_add(shade / 2);
            frame[i + 2] = base_b.saturating_add(shade);
        }
    }
    fill_rect(&mut frame, 0, 0, FRAME_WIDTH, 14, (255, 255, 255));
    draw_text_line(
        &mut frame,
        "DOOMSCRUM FREE PREVIEW",
        18,
        24,
        (255, 255, 255),
    );
    draw_section(&mut frame, "TITLE", &fixture_title(storyboard), 72);
    draw_section(&mut frame, "GOAL", &fixture_goal(storyboard), 206);
    draw_section(
        &mut frame,
        "FIRST CRITERION",
        &fixture_criterion(storyboard),
        356,
    );
    frame
}

fn spec_color(hash: &str) -> (u8, u8, u8) {
    let r = hex_byte(hash, 0).saturating_add(24);
    let g = hex_byte(hash, 2).saturating_add(18);
    let b = hex_byte(hash, 4).saturating_add(36);
    (r.min(150), g.min(120), b.min(170))
}

fn hex_byte(value: &str, offset: usize) -> u8 {
    let bytes = value.as_bytes();
    bytes.get(offset).copied().map(hex_nibble).unwrap_or(17) * 16
        + bytes.get(offset + 1).copied().map(hex_nibble).unwrap_or(3)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => 0,
    }
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

fn fixture_goal(storyboard: &Storyboard) -> String {
    storyboard
        .beats
        .first()
        .map(|beat| beat.spec_payload.clone())
        .filter(|goal| !goal.trim().is_empty())
        .unwrap_or_else(|| "No goal recorded.".into())
}

fn fixture_criterion(storyboard: &Storyboard) -> String {
    storyboard
        .beats
        .iter()
        .find(|beat| beat.label == "Payload")
        .or_else(|| storyboard.beats.get(2))
        .map(|beat| beat.spec_payload.clone())
        .filter(|criterion| !criterion.trim().is_empty())
        .unwrap_or_else(|| "No first criterion recorded.".into())
}

fn draw_section(frame: &mut [u8], label: &str, value: &str, y: u32) {
    fill_rect(
        frame,
        14,
        y.saturating_sub(8),
        FRAME_WIDTH - 28,
        2,
        (255, 255, 255),
    );
    draw_text_line(frame, label, 18, y, (255, 228, 120));
    let lines = wrapped_lines(value, 25);
    for (line, text) in lines.iter().take(MAX_LINES_PER_SECTION).enumerate() {
        draw_text_line(frame, text, 18, y + 18 + line as u32 * 16, (255, 255, 255));
    }
}

fn wrapped_lines(value: &str, width: usize) -> Vec<String> {
    let normalized: String = value
        .chars()
        .take(MAX_DISPLAY_CHARS)
        .map(|ch| {
            if ch.is_ascii() {
                ch.to_ascii_uppercase()
            } else {
                '?'
            }
        })
        .collect();
    let mut lines = Vec::new();
    let mut line = String::new();
    for ch in normalized.chars() {
        if ch.is_whitespace() && line.is_empty() {
            continue;
        }
        line.push(if ch.is_whitespace() { ' ' } else { ch });
        if line.chars().count() >= width {
            lines.push(std::mem::take(&mut line));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push("(EMPTY)".into());
    }
    lines
}

fn draw_text_line(frame: &mut [u8], text: &str, x: u32, y: u32, color: (u8, u8, u8)) {
    let mut cursor = x;
    for ch in text.chars().take(MAX_DISPLAY_CHARS) {
        if cursor + 6 * FONT_SCALE >= FRAME_WIDTH {
            break;
        }
        draw_char(frame, ch, cursor, y, color);
        cursor += 6 * FONT_SCALE;
    }
}

fn draw_char(frame: &mut [u8], ch: char, x: u32, y: u32, color: (u8, u8, u8)) {
    let glyph = glyph(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                fill_rect(
                    frame,
                    x + col * FONT_SCALE,
                    y + row as u32 * FONT_SCALE,
                    FONT_SCALE,
                    FONT_SCALE,
                    color,
                );
            }
        }
    }
}

fn fill_rect(frame: &mut [u8], x: u32, y: u32, width: u32, height: u32, color: (u8, u8, u8)) {
    let x_end = x.saturating_add(width).min(FRAME_WIDTH);
    let y_end = y.saturating_add(height).min(FRAME_HEIGHT);
    for py in y..y_end {
        for px in x..x_end {
            let i = ((py * FRAME_WIDTH + px) * 3) as usize;
            frame[i..i + 3].copy_from_slice(&[color.0, color.1, color.2]);
        }
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ':' => [0, 0b00100, 0, 0, 0b00100, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b00110, 0b00110],
        ',' => [0, 0, 0, 0, 0, 0b00110, 0b00100],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0, 0],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0, 0, 0b00100],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        _ => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backlog::PrdSource;
    use crate::distill::{compile_storyboard, distill};
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
            issue_number: None,
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
    fn no_drawtext_ffmpeg_still_produces_spec_branded_render() {
        let real_ffmpeg = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .expect("host ffmpeg is required for this regression");
        assert!(real_ffmpeg.status.success());
        let real_path = std::env::current_exe()
            .ok()
            .and_then(|_| std::env::var_os("PATH"))
            .and_then(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join("ffmpeg"))
                    .find(|candidate| candidate.exists())
            })
            .expect("ffmpeg on PATH");
        let dir = tempfile::tempdir().unwrap();
        let wrapper = dir.path().join("ffmpeg-no-drawtext");
        let mut script = std::fs::File::create(&wrapper).unwrap();
        writeln!(
            script,
            "#!/bin/sh\nif [ \"$1\" = \"-hide_banner\" ] && [ \"$2\" = \"-filters\" ]; then printf ' T.C hflip V->V\n'; exit 0; fi\nexec '{}' \"$@\"",
            real_path.display()
        )
        .unwrap();
        drop(script);
        let mut perms = std::fs::metadata(&wrapper).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&wrapper, perms).unwrap();

        let prd = prd_with(
            "# No Drawtext Truth\n\n## Goal\nShow the actual spec in the free preview.\n\n## Acceptance Criteria\n- The title, goal, and criterion are visible.\n",
            0,
        );
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let render = FakeProvider
            .render_with_ffmpeg(&storyboard, dir.path(), wrapper.to_str().unwrap())
            .unwrap();
        let mp4 =
            std::fs::read(dir.path().join(&render.prd_sha256).join(&render.asset_file)).unwrap();

        assert_eq!(render.model, "ffmpeg-fixture");
        assert!(render.degraded_reason.is_none());
        assert_ne!(mp4, FIXTURE_MP4);
        assert_eq!(&mp4[4..8], b"ftyp");
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
    fn degraded_reason_is_present_in_the_first_provenance_write() {
        let prd = prd_with("# Budget Fallback\n\n## Goal\nStay within budget.\n", 0);
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);
        let dir = tempfile::tempdir().unwrap();

        let render = FakeProvider
            .render_with_ffmpeg_reason(
                &storyboard,
                dir.path(),
                "definitely-no-ffmpeg-here",
                Some("render budget exhausted"),
            )
            .unwrap();
        let provenance = std::fs::read(
            dir.path()
                .join(&render.prd_sha256)
                .join(format!("{}.json", render.id)),
        )
        .unwrap();
        let persisted: VideoRender = serde_json::from_slice(&provenance).unwrap();

        assert_eq!(
            persisted.degraded_reason.as_deref(),
            Some("render budget exhausted")
        );
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
        assert!(render
            .degraded_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("ffmpeg")));
        let mp4 =
            std::fs::read(dir.path().join(&render.prd_sha256).join(&render.asset_file)).unwrap();
        assert_eq!(mp4, FIXTURE_MP4);
    }

    #[test]
    fn forced_encoder_failure_marks_embedded_fixture_degraded() {
        let dir = tempfile::tempdir().unwrap();
        let failing = dir.path().join("ffmpeg-fails");
        std::fs::write(&failing, "#!/bin/sh\nexit 1\n").unwrap();
        let mut perms = std::fs::metadata(&failing).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&failing, perms).unwrap();
        let prd = prd_with("# Encoder Failure\n\n## Goal\nStay honest.\n", 0);
        let storyboard = compile_storyboard(&prd, &distill(&prd), 8);

        let render = FakeProvider
            .render_with_ffmpeg(&storyboard, dir.path(), failing.to_str().unwrap())
            .unwrap();

        assert_eq!(render.model, "embedded-fixture");
        assert_eq!(render.status, "ready");
        assert!(render
            .degraded_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("spec-branded free preview unavailable")));
    }

    #[test]
    fn portable_args_use_ppm_loop_without_drawtext() {
        let args = ffmpeg_args(Path::new("spec.ppm"), Path::new("spec.mp4"));
        let joined = args.join(" ");
        assert!(joined.contains("-loop 1"));
        assert!(joined.contains("-framerate 30"));
        assert!(joined.contains("spec.ppm"));
        assert!(joined.contains("-c:v libx264"));
        assert!(!joined.contains("drawtext"));
    }

    #[test]
    fn spec_frame_rasterizes_the_extracted_title_goal_and_criterion() {
        let prd = prd_with(
            "# Cache Chaos\n\n## Goal\nShow the newest render.\n\n## Acceptance Criteria\n- The title, goal, and criterion are visible.\n",
            0,
        );
        let board = compile_storyboard(&prd, &distill(&prd), 8);
        assert_eq!(fixture_title(&board), "Cache Chaos");
        assert!(fixture_goal(&board).contains("Show the newest render"));
        assert!(fixture_criterion(&board).contains("title, goal, and criterion"));

        let baseline = spec_frame(&board);
        let mut title_variant = board.clone();
        title_variant.beats[0].caption = "Other Title just entered the chat".into();
        let mut goal_variant = board.clone();
        goal_variant.beats[0].spec_payload = "A different goal".into();
        let mut criterion_variant = board.clone();
        criterion_variant
            .beats
            .iter_mut()
            .find(|beat| beat.label == "Payload")
            .expect("compiled storyboard has a payload beat")
            .spec_payload = "A different criterion".into();

        assert!(region_diff(&baseline, &spec_frame(&title_variant), 72, 190) > 20);
        assert!(region_diff(&baseline, &spec_frame(&goal_variant), 206, 340) > 20);
        assert!(region_diff(&baseline, &spec_frame(&criterion_variant), 356, 550) > 20);
    }

    fn region_diff(first: &[u8], second: &[u8], y_start: u32, y_end: u32) -> usize {
        first
            .chunks_exact(3)
            .zip(second.chunks_exact(3))
            .enumerate()
            .filter(|(i, (a, b))| {
                let y = *i as u32 / FRAME_WIDTH;
                y >= y_start && y < y_end && a != b
            })
            .count()
    }

    #[test]
    fn ffmpeg_fixture_is_spec_specific() {
        let first_prd = prd_with("# Same Demo\n\n## Goal\nShow one thing.\n", 0);
        let second_prd = prd_with("# Same Demo\n\n## Goal\nShow another thing.\n", 0);
        let first_board = compile_storyboard(&first_prd, &distill(&first_prd), 8);
        let second_board = compile_storyboard(&second_prd, &distill(&second_prd), 8);
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
