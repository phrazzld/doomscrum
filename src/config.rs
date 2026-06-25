use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Loaded from `doomscrum.toml` in the project root. Every field has a default,
/// so the file (and any table in it) is optional.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Active render profile (a key of `profiles`). Empty = base [video]
    /// config as written. CLI `--profile` overrides this.
    pub profile: String,
    pub repo: RepoConfig,
    pub feed: FeedConfig,
    pub video: VideoConfig,
    pub script: ScriptConfig,
    pub agent: AgentConfig,
    /// Named video overrides so cheap local iteration and real content
    /// generation coexist in one file (`[profiles.dev]`, `[profiles.content]`).
    pub profiles: std::collections::BTreeMap<String, ProfileConfig>,
}

/// Partial video override applied when its profile is active. Unset fields
/// keep the base [video] values; `mix = []` explicitly clears the mix.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfileConfig {
    pub provider: Option<String>,
    pub fal_model: Option<String>,
    pub max_duration_sec: Option<u32>,
    pub max_total_spend_usd: Option<f64>,
    pub max_daily_spend_usd: Option<f64>,
    pub mix: Option<Vec<MixEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoConfig {
    /// The repository DoomScrum is synced to. Backlog is read from here and
    /// agent worktrees are created from here.
    pub path: String,
    /// Backlog directory inside the synced repo. One markdown file per spec.
    pub backlog_dir: String,
    /// Runtime state (renders, events, dispatch receipts, worktrees).
    pub state_dir: String,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            path: ".".into(),
            backlog_dir: "backlog.d".into(),
            state_dir: ".doomscrum".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeedConfig {
    /// Cap the feed to the top N specs by priority (filename order).
    pub max_items: usize,
    /// How many specs ahead of the viewport cursor to keep warm with
    /// just-in-time real renders. Serving the feed renders at most this many
    /// specs in the window `[cursor, cursor + prefetch_depth)`; specs deeper in
    /// the feed cost nothing until the cursor approaches them. 0 disables JIT
    /// rendering entirely (renders only happen on explicit `generate`).
    pub prefetch_depth: usize,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            max_items: 10,
            prefetch_depth: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    /// "fake" (embedded fixture, free, offline) or "fal" (real generation).
    pub provider: String,
    pub fal_model: String,
    pub fal_base_url: String,
    pub max_duration_sec: u32,
    /// veo3.1/fast with audio at 720p is $0.15/s on fal (verified 2026-06).
    pub price_per_second_usd: f64,
    /// Hard wallet guard: real renders are refused once estimated total
    /// spend (summed from render provenance) would exceed this.
    pub max_total_spend_usd: f64,
    /// Independent daily guard for real renders. Exceeding it returns 429
    /// from HTTP routes and aborts CLI generation before provider startup.
    pub max_daily_spend_usd: f64,
    /// Weighted render portfolio. When non-empty, each spec draws one
    /// (model, duration) deterministically by content hash — most specs
    /// land on cheap/short pipelines, a weighted few on hero models, and
    /// the average cost drops without making every clip the same.
    pub mix: Vec<MixEntry>,
}

/// One pipeline in the render mix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixEntry {
    pub model: String,
    pub duration_sec: u32,
    /// Relative draw weight: weight 3 is picked ~3x as often as weight 1.
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    1
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            provider: "fake".into(),
            fal_model: "fal-ai/veo3.1/fast".into(),
            fal_base_url: "https://queue.fal.run".into(),
            max_duration_sec: 8,
            price_per_second_usd: 0.15,
            max_total_spend_usd: 25.0,
            max_daily_spend_usd: 5.0,
            mix: Vec::new(),
        }
    }
}

impl VideoConfig {
    /// Resolve this spec's pipeline: a clone of the config with the
    /// mix-drawn model and duration applied. Deterministic per spec hash
    /// (stable re-renders); the identity when no mix is configured.
    pub fn with_pipeline(&self, spec_sha: &str) -> VideoConfig {
        let mut cfg = self.clone();
        if self.mix.is_empty() {
            return cfg;
        }
        let seed = crate::util::spec_seed(spec_sha);
        let total: u64 = self.mix.iter().map(|m| u64::from(m.weight.max(1))).sum();
        let mut x = seed % total;
        for entry in &self.mix {
            let w = u64::from(entry.weight.max(1));
            if x < w {
                cfg.fal_model = entry.model.clone();
                cfg.max_duration_sec = entry.duration_sec;
                return cfg;
            }
            x -= w;
        }
        cfg
    }
}

/// How spoken scripts are written. Specs arrive in arbitrary shapes from
/// arbitrary repos, so the default is an LLM reading the full raw spec;
/// the deterministic template planner survives only as the offline path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptConfig {
    /// "llm" (default — real renders refuse to fall back silently) or
    /// "templates" (deterministic, offline, free).
    pub mode: String,
    /// OpenAI-compatible chat-completions model id. gpt-5.4-mini won the
    /// 2026-06-11 script bench (~$0.004/script); kimi-k2.5 is the budget
    /// runner-up. Re-run scripts/script_bench.py before changing.
    pub model: String,
    /// OpenAI-compatible API base. OpenRouter by default (one key, any
    /// model); key resolved from OPENROUTER_API_KEY (env or ~/.secrets).
    pub base_url: String,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            mode: "llm".into(),
            model: "openai/gpt-5.4-mini".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
        }
    }
}

/// Agent command templates. Placeholders substituted per dispatch:
/// `{worktree}`, `{prompt}`, `{branch}`, `{spec_path}`, `{title}`, `{model}`,
/// `{body_file}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub implement_cmd: Vec<String>,
    pub shape_cmd: Vec<String>,
    pub pr_cmd: Vec<String>,
    /// The model the default `opencode` agent runs, in opencode's
    /// `provider/model` form. Substituted into `{model}` in the agent commands,
    /// so switching models is a one-line `agent_model = "..."` change. Ignored
    /// by agent commands that don't reference `{model}` (e.g. a codex override).
    pub agent_model: String,
    /// Environment variables the *untrusted* agent stage is allowed to inherit.
    /// The agent runs spec content that can come from a foreign repo, so its
    /// process env is built from this allowlist alone (`env_clear` + re-add) —
    /// never the parent env. The default is the minimum to run a CLI (PATH/HOME/
    /// locale/XDG): no API keys, since the agent's output is committed and
    /// pushed, so any key in its env can be exfiltrated into a PR. Service-secret
    /// names (`FAL_API_KEY`/`OPENROUTER_API_KEY`/git tokens) are dropped even if
    /// listed. Operators whose agent authenticates via an env var add it here,
    /// accepting that exposure.
    pub env_allowlist: Vec<String>,
    /// When false, dispatch stops after the agent commits (no push, no PR).
    pub open_pr: bool,
    /// Maximum agent runs allowed at once; additional swipes persist queued
    /// receipts and start when a slot opens.
    pub max_concurrent_dispatches: usize,
    /// Seconds a dispatch sits `queued` and cancellable before the agent starts
    /// — the mis-swipe undo window. Cancelling within it leaves zero git
    /// side-effects (no worktree, branch, or PR). 0 disables the window.
    pub undo_window_sec: u64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        // The default dispatched agent is the `opencode` CLI on OpenRouter (043).
        // `opencode run --dir <wt> -m <provider/model> <prompt>` runs the agent
        // non-interactively in the worktree. opencode authenticates from its own
        // credential file (`~/.local/share/opencode/auth.json`, written by
        // `opencode auth login`), reached through HOME like codex's auth.json —
        // so no API key needs to enter the agent env.
        let opencode = || -> Vec<String> {
            [
                "opencode",
                "run",
                "--dir",
                "{worktree}",
                "-m",
                "{model}",
                "{prompt}",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        };
        Self {
            implement_cmd: opencode(),
            shape_cmd: opencode(),
            agent_model: "openrouter/z-ai/glm-5.2".to_string(),
            pr_cmd: [
                "gh",
                "pr",
                "create",
                "--head",
                "{branch}",
                "--title",
                "{title}",
                "--body-file",
                "{body_file}",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            env_allowlist: [
                // Process + toolchain essentials any agent CLI needs to run.
                "PATH",
                "HOME",
                "USER",
                "LOGNAME",
                "SHELL",
                "TERM",
                "TMPDIR",
                "TZ",
                "LANG",
                "LC_ALL",
                "LC_CTYPE",
                // XDG dirs — agents that keep config/credentials there.
                "XDG_CONFIG_HOME",
                "XDG_CACHE_HOME",
                "XDG_DATA_HOME",
                // NB: NO provider API keys (OPENROUTER_API_KEY/OPENAI_API_KEY) by
                // default. The default agent (`opencode`) authenticates from its
                // credential file (~/.local/share/opencode/auth.json, written by
                // `opencode auth login`), reached through HOME — so it needs no
                // key in env. Any key in the agent's env can be written to a file
                // the dispatcher then commits and pushes, so operators whose agent
                // authenticates via an env var add it here, accepting that it is
                // exposed to spec-driven execution.
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            open_pr: true,
            max_concurrent_dispatches: 2,
            undo_window_sec: 5,
        }
    }
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_with_profile(root, None)
    }

    pub fn load_with_profile(root: &Path, profile: Option<&str>) -> Result<Self> {
        let path = root.join("doomscrum.toml");
        let mut cfg: Config = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?
        } else {
            Self::default()
        };
        if let Some(name) = profile {
            cfg.profile = name.to_string();
        }
        cfg.apply_active_profile()?;
        Ok(cfg)
    }

    fn apply_active_profile(&mut self) -> Result<()> {
        if self.profile.is_empty() {
            return Ok(());
        }
        let Some(p) = self.profiles.get(&self.profile).cloned() else {
            let known: Vec<&str> = self.profiles.keys().map(String::as_str).collect();
            anyhow::bail!(
                "unknown profile {:?}; available profiles: {}",
                self.profile,
                if known.is_empty() {
                    "(none defined)".to_string()
                } else {
                    known.join(", ")
                }
            );
        };
        if let Some(v) = p.provider {
            self.video.provider = v;
        }
        if let Some(v) = p.fal_model {
            self.video.fal_model = v;
        }
        if let Some(v) = p.max_duration_sec {
            self.video.max_duration_sec = v;
        }
        if let Some(v) = p.max_total_spend_usd {
            self.video.max_total_spend_usd = v;
        }
        if let Some(v) = p.max_daily_spend_usd {
            self.video.max_daily_spend_usd = v;
        }
        if let Some(v) = p.mix {
            self.video.mix = v;
        }
        Ok(())
    }
}

/// Substitute `{placeholder}` values into a command template.
pub fn substitute(template: &[String], vars: &[(&str, &str)]) -> Vec<String> {
    template
        .iter()
        .map(|arg| {
            let mut out = arg.clone();
            for (key, value) in vars {
                out = out.replace(&format!("{{{key}}}"), value);
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.feed.max_items, 10);
        assert_eq!(cfg.feed.prefetch_depth, 3);
        assert_eq!(cfg.video.provider, "fake");
        assert_eq!(cfg.video.max_daily_spend_usd, 5.0);
        assert_eq!(cfg.repo.backlog_dir, "backlog.d");
        assert!(cfg.agent.open_pr);
        assert_eq!(cfg.agent.max_concurrent_dispatches, 2);
        assert_eq!(cfg.agent.undo_window_sec, 5);
        assert_eq!(cfg.agent.implement_cmd[0], "opencode");
    }

    #[test]
    fn default_agent_is_opencode_glm_on_openrouter() {
        let agent = AgentConfig::default();
        // The default dispatched agent is the opencode CLI on OpenRouter (043).
        assert_eq!(
            agent.implement_cmd,
            vec![
                "opencode",
                "run",
                "--dir",
                "{worktree}",
                "-m",
                "{model}",
                "{prompt}"
            ]
        );
        assert_eq!(agent.shape_cmd, agent.implement_cmd);
        // Model is a one-line change via the {model} placeholder; default GLM 5.2.
        assert!(agent.implement_cmd.iter().any(|a| a == "{model}"));
        assert_eq!(agent.agent_model, "openrouter/z-ai/glm-5.2");
    }

    #[test]
    fn default_agent_command_substitutes_to_a_concrete_opencode_invocation() {
        let agent = AgentConfig::default();
        let cmd = substitute(
            &agent.implement_cmd,
            &[
                ("worktree", "/tmp/wt"),
                ("model", agent.agent_model.as_str()),
                ("prompt", "do the thing"),
            ],
        );
        assert_eq!(
            cmd,
            vec![
                "opencode",
                "run",
                "--dir",
                "/tmp/wt",
                "-m",
                "openrouter/z-ai/glm-5.2",
                "do the thing",
            ]
        );
    }

    #[test]
    fn agent_env_allowlist_excludes_doomscrum_secrets_keeps_runtime() {
        let allow = AgentConfig::default().env_allowlist;
        // Runtime essentials the default agent (opencode) needs to start + auth.
        assert!(allow.iter().any(|k| k == "PATH"), "{allow:?}");
        assert!(allow.iter().any(|k| k == "HOME"), "{allow:?}");
        // DoomScrum's service keys + git push tokens must never reach the agent.
        for forbidden in [
            "FAL_API_KEY",
            "FAL_KEY",
            "OPENROUTER_API_KEY",
            "GH_TOKEN",
            "GITHUB_TOKEN",
        ] {
            assert!(
                !allow.iter().any(|k| k == forbidden),
                "allowlist leaks {forbidden}: {allow:?}"
            );
        }
    }

    #[test]
    fn partial_toml_overrides_only_named_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doomscrum.toml"),
            r#"
[feed]
max_items = 3

[agent]
implement_cmd = ["echo", "{worktree}"]
"#,
        )
        .unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.feed.max_items, 3);
        assert_eq!(cfg.agent.implement_cmd, vec!["echo", "{worktree}"]);
        // untouched tables keep defaults
        assert_eq!(cfg.video.provider, "fake");
        assert_eq!(cfg.agent.pr_cmd[0], "gh");
    }

    #[test]
    fn render_mix_parses_and_picks_deterministically() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doomscrum.toml"),
            r#"
[video]
fal_model = "fal-ai/sora-2/text-to-video"
max_duration_sec = 12

[[video.mix]]
model = "fal-ai/ltx-2.3/text-to-video/fast"
duration_sec = 8
weight = 3

[[video.mix]]
model = "fal-ai/sora-2/text-to-video"
duration_sec = 12
weight = 1
"#,
        )
        .unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.video.mix.len(), 2);

        // Same spec hash always draws the same pipeline (stable re-renders).
        let a = cfg.video.with_pipeline("00aa11bb22cc33dd44ee");
        let b = cfg.video.with_pipeline("00aa11bb22cc33dd44ee");
        assert_eq!(a.fal_model, b.fal_model);
        assert_eq!(a.max_duration_sec, b.max_duration_sec);

        // Across many hashes both entries get picked, weighted toward
        // the cheap one.
        let mut cheap = 0;
        let mut hero = 0;
        for i in 0..32u32 {
            let sha = format!("{i:016x}");
            let v = cfg.video.with_pipeline(&sha);
            if v.fal_model.contains("ltx") {
                assert_eq!(v.max_duration_sec, 8);
                cheap += 1;
            } else {
                assert_eq!(v.max_duration_sec, 12);
                hero += 1;
            }
        }
        assert!(cheap > hero, "weights ignored: cheap={cheap} hero={hero}");
        assert!(hero > 0, "hero entry never drawn");
    }

    #[test]
    fn empty_mix_keeps_the_single_configured_pipeline() {
        let cfg = VideoConfig::default();
        let v = cfg.with_pipeline("deadbeef00000000");
        assert_eq!(v.fal_model, cfg.fal_model);
        assert_eq!(v.max_duration_sec, cfg.max_duration_sec);
    }

    fn profile_toml() -> &'static str {
        r#"
profile = "dev"

[video]
provider = "fal"
fal_model = "fal-ai/sora-2/text-to-video"
max_duration_sec = 12

[[video.mix]]
model = "fal-ai/ltx-2.3/text-to-video/fast"
duration_sec = 8
weight = 5

[[video.mix]]
model = "fal-ai/sora-2/text-to-video"
duration_sec = 12
weight = 1

[profiles.dev]
provider = "fake"
mix = []

[profiles.content]
provider = "fal"
"#
    }

    #[test]
    fn active_profile_overrides_video_at_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doomscrum.toml"), profile_toml()).unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        // dev profile: free provider, mix cleared, base model untouched.
        assert_eq!(cfg.video.provider, "fake");
        assert!(cfg.video.mix.is_empty());
        assert_eq!(cfg.video.fal_model, "fal-ai/sora-2/text-to-video");
    }

    #[test]
    fn cli_profile_override_beats_the_toml_profile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doomscrum.toml"), profile_toml()).unwrap();
        let cfg = Config::load_with_profile(dir.path(), Some("content")).unwrap();
        // content profile leaves the render mix intact.
        assert_eq!(cfg.video.provider, "fal");
        assert_eq!(cfg.video.mix.len(), 2);
    }

    #[test]
    fn unknown_profile_is_an_error_naming_the_options() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doomscrum.toml"), profile_toml()).unwrap();
        let err = Config::load_with_profile(dir.path(), Some("nope")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("nope"), "{msg}");
        assert!(msg.contains("dev"), "{msg}");
    }

    #[test]
    fn no_profile_keys_keeps_legacy_behavior() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.profile, "");
        assert!(cfg.profiles.is_empty());
    }

    #[test]
    fn substitute_replaces_all_placeholders() {
        let cmd = substitute(
            &[
                "run".into(),
                "--cd".into(),
                "{worktree}".into(),
                "{prompt}".into(),
            ],
            &[("worktree", "/tmp/wt"), ("prompt", "do the thing")],
        );
        assert_eq!(cmd, vec!["run", "--cd", "/tmp/wt", "do the thing"]);
    }
}
