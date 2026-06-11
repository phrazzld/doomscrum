//! LLM scriptwriter: the full raw spec — any format, any structure — goes
//! to a creative-writing LLM that returns the spoken script and the visual
//! scene. Output is cached by spec content hash + model, so a spec never
//! pays for its script twice and re-renders stay stable. The deterministic
//! template planner in `distill` remains only as the offline fallback.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::backlog::PrdSource;
use crate::config::ScriptConfig;
use crate::distill::{self, word_budget, Storyboard};

/// One LLM-written script, as cached on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmScript {
    pub model: String,
    /// The exact words the clip must speak (transcript-verified).
    pub script: String,
    /// Visual scene for the video model — character, setting, energy.
    pub scene: String,
}

fn cache_path(dir: &Path, prd_sha: &str, model: &str, duration_sec: u32) -> PathBuf {
    let slug: String = model
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    dir.join(format!("{prd_sha}-{slug}-{duration_sec}s.json"))
}

/// Strip optional markdown fences and parse the model's JSON reply.
pub fn parse_reply(content: &str, model: &str) -> Result<LlmScript> {
    let trimmed = content.trim();
    let body = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.trim_end_matches("```"))
        .unwrap_or(trimmed)
        .trim();
    #[derive(Deserialize)]
    struct Reply {
        script: String,
        scene: String,
    }
    let reply: Reply = serde_json::from_str(body)
        .with_context(|| format!("scriptwriter returned non-JSON: {body:.200}"))?;
    anyhow::ensure!(
        !reply.script.trim().is_empty() && !reply.scene.trim().is_empty(),
        "scriptwriter returned empty script or scene"
    );
    Ok(LlmScript {
        model: model.to_string(),
        script: reply.script.trim().to_string(),
        scene: reply.scene.trim().to_string(),
    })
}

fn system_prompt(duration_sec: u32) -> String {
    let budget = word_budget(duration_sec);
    format!(
        "You write scripts for {duration_sec}-second vertical brainrot videos. Each video \
         communicates one software backlog spec — the user input is the raw spec file, in \
         whatever format it happens to be. Your job: the tightest, clearest, funniest \
         possible articulation of WHAT the spec wants and (when stated) WHEN it counts as \
         done. The spec is the content; the brainrot is the delivery.\n\
         Reply with STRICT JSON, no markdown fences, exactly two keys:\n\
         {{\"script\": \"...\", \"scene\": \"...\"}}\n\
         script: the complete spoken dialogue. HARD LIMIT {budget} words — it must be \
         finishable in {duration_sec} seconds at an energetic pace. Punchy, meme-cadence, \
         zero filler; every word earns its place. Never invent features, metrics, or claims \
         the spec doesn't make. No hashtags, no emoji.\n\
         scene: one vivid paragraph for a text-to-video model describing the character and \
         setting that DELIVERS the script. Go absurd. House favorites for inspiration — a \
         talking-fruit soap opera, a 90s infomercial pitchman, a cryptid filming a selfie \
         vlog in the woods, an Italian-brainrot hybrid creature reveal, a street interview \
         in the year 3024, a deadpan gen-z explainer — but inventing something equally \
         unhinged is encouraged. Describe who speaks, where, and the camera energy. Do NOT \
         include the dialogue text in the scene; it is appended separately."
    )
}

async fn call_llm(
    cfg: &ScriptConfig,
    api_key: &str,
    prd: &PrdSource,
    duration_sec: u32,
) -> Result<LlmScript> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.9,
        "messages": [
            {"role": "system", "content": system_prompt(duration_sec)},
            {"role": "user", "content": prd.raw},
        ],
    });
    let resp = client
        .post(format!("{}/chat/completions", cfg.base_url.trim_end_matches('/')))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("scriptwriter request failed")?;
    let status = resp.status();
    let payload: serde_json::Value = resp.json().await.context("scriptwriter response body")?;
    anyhow::ensure!(
        status.is_success(),
        "scriptwriter HTTP {status}: {}",
        payload
            .get("error")
            .map(|e| e.to_string())
            .unwrap_or_default()
    );
    let content = payload["choices"][0]["message"]["content"]
        .as_str()
        .context("scriptwriter reply had no message content")?;
    let script = parse_reply(content, &cfg.model)?;
    let budget = word_budget(duration_sec);
    let count = script.script.split_whitespace().count();
    anyhow::ensure!(
        count <= budget + 3,
        "scriptwriter blew the word budget: {count} words for a {budget}-word \
         {duration_sec}s clip — re-run to re-roll"
    );
    Ok(script)
}

/// Get the LLM script for a spec: cache hit, or one paid call (~$0.002)
/// whose result is written through to the cache.
pub async fn write_script(
    cfg: &ScriptConfig,
    api_key: Option<&str>,
    prd: &PrdSource,
    duration_sec: u32,
    cache_dir: &Path,
) -> Result<LlmScript> {
    let path = cache_path(cache_dir, &prd.sha256, &cfg.model, duration_sec);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(cached) = serde_json::from_str::<LlmScript>(&raw) {
            return Ok(cached);
        }
    }
    let key = api_key.context(
        "script.mode is \"llm\" but no OPENROUTER_API_KEY found (env or ~/.secrets); \
         set the key or set [script] mode = \"templates\" in doomscrum.toml",
    )?;
    let script = call_llm(cfg, key, prd, duration_sec).await?;
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(&path, serde_json::to_string_pretty(&script)?)?;
    Ok(script)
}

/// Compile the storyboard for a render. Real (paid) renders in "llm" mode
/// require an LLM script — no silent fallback to templates under spend.
/// The fake provider and "templates" mode use the deterministic planner.
pub async fn storyboard(
    script_cfg: &ScriptConfig,
    api_key: Option<&str>,
    prd: &PrdSource,
    duration_sec: u32,
    cache_dir: &Path,
    paid_render: bool,
) -> Result<Storyboard> {
    let mut board =
        distill::compile_storyboard(prd, &distill::distill(prd), duration_sec);
    if script_cfg.mode != "llm" || !paid_render {
        return Ok(board);
    }
    let llm = write_script(script_cfg, api_key, prd, duration_sec, cache_dir).await?;
    board.provider_prompt =
        distill::compose_provider_prompt(&llm.scene, &llm.script, duration_sec);
    board.expected_script = llm.script;
    board.tone = format!("llm:{}", llm.model);
    Ok(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prd(raw: &str) -> PrdSource {
        PrdSource {
            id: "i".into(),
            sha256: "ab".repeat(32),
            rel_path: "backlog.d/x.md".into(),
            abs_path: std::path::PathBuf::new(),
            title: "T".into(),
            priority: 0,
            raw: raw.into(),
        }
    }

    #[test]
    fn parse_reply_handles_fences_and_rejects_empties() {
        let ok = parse_reply(
            "```json\n{\"script\": \"ship it\", \"scene\": \"a moose with a podcast\"}\n```",
            "m",
        )
        .unwrap();
        assert_eq!(ok.script, "ship it");
        assert_eq!(ok.scene, "a moose with a podcast");
        assert!(parse_reply("{\"script\": \"\", \"scene\": \"x\"}", "m").is_err());
        assert!(parse_reply("not json at all", "m").is_err());
    }

    #[tokio::test]
    async fn cache_hit_needs_no_key_and_no_network() {
        let dir = tempfile::tempdir().unwrap();
        let p = prd("# Anything");
        let cached = LlmScript {
            model: "moonshotai/kimi-k2.6".into(),
            script: "the cache speaks".into(),
            scene: "a filing cabinet with eyes".into(),
        };
        let path = cache_path(dir.path(), &p.sha256, &cached.model, 12);
        std::fs::write(&path, serde_json::to_string(&cached).unwrap()).unwrap();
        let cfg = ScriptConfig::default();
        let got = write_script(&cfg, None, &p, 12, dir.path()).await.unwrap();
        assert_eq!(got.script, "the cache speaks");
    }

    #[tokio::test]
    async fn llm_mode_without_key_fails_loudly_for_paid_renders() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ScriptConfig::default();
        let err = storyboard(&cfg, None, &prd("# Spec"), 12, dir.path(), true)
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("OPENROUTER_API_KEY"));
    }

    #[tokio::test]
    async fn fake_renders_use_the_offline_planner() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ScriptConfig::default();
        let board = storyboard(
            &cfg,
            None,
            &prd("# Spec\n\n## Goal\nDo the thing.\n"),
            12,
            dir.path(),
            false,
        )
        .await
        .unwrap();
        assert!(!board.expected_script.is_empty());
        assert!(!board.tone.starts_with("llm:"));
    }

    #[tokio::test]
    async fn llm_storyboard_carries_script_through_the_standard_frame() {
        let dir = tempfile::tempdir().unwrap();
        let p = prd("totally unstructured spec, no headings, vibes only");
        let cached = LlmScript {
            model: "moonshotai/kimi-k2.6".into(),
            script: "fix the vibes. not done until the vibes are fixed".into(),
            scene: "a sentient lava lamp hosting a courtroom show".into(),
        };
        std::fs::write(
            cache_path(dir.path(), &p.sha256, &cached.model, 12),
            serde_json::to_string(&cached).unwrap(),
        )
        .unwrap();
        let cfg = ScriptConfig::default();
        let board = storyboard(&cfg, None, &p, 12, dir.path(), true).await.unwrap();
        assert_eq!(board.expected_script, cached.script);
        assert!(board.provider_prompt.contains(&cached.scene));
        assert!(board.provider_prompt.contains(&cached.script));
        assert!(board.provider_prompt.contains("FINISHED by second"));
        assert_eq!(board.tone, "llm:moonshotai/kimi-k2.6");
    }
}
