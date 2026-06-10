use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Loaded from `specifi.toml` in the project root. Every field has a default,
/// so the file (and any table in it) is optional.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub repo: RepoConfig,
    pub feed: FeedConfig,
    pub video: VideoConfig,
    pub agent: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoConfig {
    /// The repository Specifi is synced to. Backlog is read from here and
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
            state_dir: ".specifi".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeedConfig {
    /// Cap the feed to the top N specs by priority (filename order).
    pub max_items: usize,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self { max_items: 10 }
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
    pub estimate_usd: f64,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            provider: "fake".into(),
            fal_model: "fal-ai/veo3.1/fast".into(),
            fal_base_url: "https://queue.fal.run".into(),
            max_duration_sec: 8,
            estimate_usd: 1.5,
        }
    }
}

/// Agent command templates. Placeholders substituted per dispatch:
/// `{worktree}`, `{prompt}`, `{branch}`, `{spec_path}`, `{title}`, `{body_file}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub implement_cmd: Vec<String>,
    pub shape_cmd: Vec<String>,
    pub pr_cmd: Vec<String>,
    /// When false, dispatch stops after the agent commits (no push, no PR).
    pub open_pr: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        let codex = |_: &str| -> Vec<String> {
            [
                "codex",
                "exec",
                "--cd",
                "{worktree}",
                "--sandbox",
                "workspace-write",
                "{prompt}",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        };
        Self {
            implement_cmd: codex("implement"),
            shape_cmd: codex("shape"),
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
            open_pr: true,
        }
    }
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("specifi.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
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
        assert_eq!(cfg.video.provider, "fake");
        assert_eq!(cfg.repo.backlog_dir, "backlog.d");
        assert!(cfg.agent.open_pr);
        assert_eq!(cfg.agent.implement_cmd[0], "codex");
    }

    #[test]
    fn partial_toml_overrides_only_named_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("specifi.toml"),
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
