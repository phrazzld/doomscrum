//! Preflight sanity checks ("doctor") and first-run scaffolding ("init").
//! 043 children 2 & 3.
//!
//! The verdict logic is a pure function of [`Facts`] — the CLI gathers those
//! facts via real env / process / filesystem lookups, then hands them here — so
//! the pass/warn/fail logic is testable without touching the environment, the
//! network, or `gh`.

/// Severity of a single check. Ordered `Ok < Warn < Fail` so the overall
/// verdict is `max` across checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    pub fix: Option<String>,
}

/// Resolved environment facts. Gathered by the CLI (`gather_facts`); consumed by
/// [`evaluate`]. Keeping I/O out of the evaluator makes the verdict testable.
#[derive(Debug, Clone)]
pub struct Facts {
    /// The default `opencode` agent is configured (vs a codex/claude override).
    pub agent_is_opencode: bool,
    /// `script.mode == "llm"` — the LLM scriptwriter is active and needs OpenRouter.
    pub script_llm_mode: bool,
    /// `OPENROUTER_API_KEY` resolvable from env or `~/.secrets` (scriptwriter).
    pub openrouter_key: bool,
    /// opencode has a *stored* OpenRouter credential. Required because the
    /// dispatched agent runs with a scrubbed env, so an env-only key won't reach
    /// it — only `opencode auth login`'s stored credential survives.
    pub opencode_stored_auth: bool,
    /// `gh` CLI is authenticated (PR creation).
    pub gh_authed: bool,
    /// The synced repo path is a git work tree.
    pub repo_is_git: bool,
    /// The synced repo has a push remote (else PRs can't open; branch stays local).
    pub repo_has_remote: bool,
    /// `video.provider == "fal"` (real paid renders).
    pub provider_is_fal: bool,
    /// FAL key resolvable from env or `~/.secrets`.
    pub fal_key: bool,
}

/// Pure verdict: the ordered list of checks for these facts.
pub fn evaluate(f: &Facts) -> Vec<Check> {
    let mut checks = Vec::new();

    if f.script_llm_mode {
        checks.push(if f.openrouter_key {
            ok("openrouter (scriptwriter)", "OPENROUTER_API_KEY found")
        } else {
            Check {
                name: "openrouter (scriptwriter)",
                status: Status::Fail,
                detail: "script.mode is \"llm\" but OPENROUTER_API_KEY is not set (env or ~/.secrets)".into(),
                fix: Some(
                    "export OPENROUTER_API_KEY=… (or add it to ~/.secrets), or set script.mode = \"templates\""
                        .into(),
                ),
            }
        });
    }

    if f.agent_is_opencode {
        checks.push(if f.opencode_stored_auth {
            ok("opencode agent auth", "opencode has a stored OpenRouter credential")
        } else {
            Check {
                name: "opencode agent auth",
                status: Status::Fail,
                detail: "the dispatched opencode agent runs with a scrubbed environment, so it needs a STORED credential — an env-only OPENROUTER_API_KEY will not reach it".into(),
                fix: Some("run `opencode auth login` and choose OpenRouter".into()),
            }
        });
    }

    checks.push(if f.gh_authed {
        ok("github (gh) auth", "gh is authenticated")
    } else {
        Check {
            name: "github (gh) auth",
            status: Status::Fail,
            detail: "gh CLI is not authenticated; dispatches cannot open PRs".into(),
            fix: Some("run `gh auth login`".into()),
        }
    });

    checks.push(if f.repo_is_git {
        ok("synced repo", "the synced path is a git repository")
    } else {
        Check {
            name: "synced repo",
            status: Status::Fail,
            detail: "[repo].path is not a git repository; dispatch needs one to create worktrees"
                .into(),
            fix: Some("point [repo].path at a git repo (or run `doomscrum init`)".into()),
        }
    });

    // The remote check is only meaningful for an actual git repo.
    if f.repo_is_git {
        checks.push(if f.repo_has_remote {
            ok("git remote", "the repo has a push remote")
        } else {
            Check {
                name: "git remote",
                status: Status::Warn,
                detail: "no git remote; agents still run and commit, but PRs cannot be opened — the branch stays local".into(),
                fix: Some("add an `origin` remote to open PRs".into()),
            }
        });
    }

    checks.push(if !f.provider_is_fal {
        ok(
            "render provider",
            "using the free offline fixture provider (no FAL key needed)",
        )
    } else if f.fal_key {
        ok("render provider", "provider=fal and a FAL key is set")
    } else {
        Check {
            name: "render provider",
            status: Status::Fail,
            detail: "video.provider = \"fal\" but no FAL_API_KEY found (env or ~/.secrets)".into(),
            fix: Some(
                "set FAL_API_KEY, or use the free fixture provider (provider = \"fake\")".into(),
            ),
        }
    });

    checks
}

fn ok(name: &'static str, detail: &str) -> Check {
    Check {
        name,
        status: Status::Ok,
        detail: detail.into(),
        fix: None,
    }
}

/// The overall verdict: the worst status across all checks.
pub fn worst(checks: &[Check]) -> Status {
    checks.iter().map(|c| c.status).max().unwrap_or(Status::Ok)
}

/// Render the report as text for the `doctor` command.
pub fn format_report(checks: &[Check]) -> String {
    let mut out = String::new();
    for c in checks {
        let marker = match c.status {
            Status::Ok => "ok  ",
            Status::Warn => "warn",
            Status::Fail => "FAIL",
        };
        out.push_str(&format!("[{marker}] {}: {}\n", c.name, c.detail));
        if let Some(fix) = &c.fix {
            out.push_str(&format!("       ↳ fix: {fix}\n"));
        }
    }
    let verdict = match worst(checks) {
        Status::Ok => "all checks passed — ready to swipe.",
        Status::Warn => "ready, with warnings (see above).",
        Status::Fail => "not ready — resolve the FAIL items above.",
    };
    out.push_str(&format!("\n{verdict}\n"));
    out
}

/// A starter `doomscrum.toml` for `init`, pointed at `repo_path`. Built from the
/// real defaults (so it dogfoods the opencode/OpenRouter default and always
/// round-trips through [`crate::config::Config::load`]).
pub fn starter_config_toml(repo_path: &str) -> String {
    let mut cfg = crate::config::Config::default();
    cfg.repo.path = repo_path.to_string();
    let body = toml::to_string_pretty(&cfg).expect("default config serializes to toml");
    format!(
        "# DoomScrum config. Generated by `doomscrum init`.\n\
         # Setup: `opencode auth login` (OpenRouter) · `gh auth login` · optional FAL_API_KEY.\n\
         # Then `doomscrum doctor` to verify, `doomscrum serve` to swipe.\n\n{body}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_good() -> Facts {
        Facts {
            agent_is_opencode: true,
            script_llm_mode: true,
            openrouter_key: true,
            opencode_stored_auth: true,
            gh_authed: true,
            repo_is_git: true,
            repo_has_remote: true,
            provider_is_fal: false,
            fal_key: false,
        }
    }

    #[test]
    fn all_good_setup_passes() {
        let checks = evaluate(&all_good());
        assert_eq!(worst(&checks), Status::Ok);
    }

    #[test]
    fn missing_openrouter_in_llm_mode_fails() {
        let f = Facts {
            openrouter_key: false,
            ..all_good()
        };
        let checks = evaluate(&f);
        assert_eq!(worst(&checks), Status::Fail);
        assert!(checks
            .iter()
            .any(|c| c.name == "openrouter (scriptwriter)" && c.status == Status::Fail));
    }

    #[test]
    fn templates_mode_does_not_require_openrouter() {
        let f = Facts {
            script_llm_mode: false,
            openrouter_key: false,
            ..all_good()
        };
        let checks = evaluate(&f);
        // No scriptwriter-openrouter check is emitted at all in templates mode.
        assert!(!checks.iter().any(|c| c.name == "openrouter (scriptwriter)"));
        assert_eq!(worst(&checks), Status::Ok);
    }

    #[test]
    fn opencode_without_stored_auth_fails() {
        let f = Facts {
            opencode_stored_auth: false,
            ..all_good()
        };
        let checks = evaluate(&f);
        assert!(checks
            .iter()
            .any(|c| c.name == "opencode agent auth" && c.status == Status::Fail));
    }

    #[test]
    fn unauthenticated_gh_fails() {
        let f = Facts {
            gh_authed: false,
            ..all_good()
        };
        assert_eq!(worst(&evaluate(&f)), Status::Fail);
    }

    #[test]
    fn non_git_repo_fails_and_skips_the_remote_check() {
        let f = Facts {
            repo_is_git: false,
            repo_has_remote: false,
            ..all_good()
        };
        let checks = evaluate(&f);
        assert!(checks
            .iter()
            .any(|c| c.name == "synced repo" && c.status == Status::Fail));
        // No remote check when it isn't even a git repo.
        assert!(!checks.iter().any(|c| c.name == "git remote"));
    }

    #[test]
    fn git_repo_without_remote_warns_but_does_not_fail() {
        let f = Facts {
            repo_has_remote: false,
            ..all_good()
        };
        let checks = evaluate(&f);
        assert!(checks
            .iter()
            .any(|c| c.name == "git remote" && c.status == Status::Warn));
        // A missing remote is a warning (branch stays local), not a hard fail.
        assert_eq!(worst(&checks), Status::Warn);
    }

    #[test]
    fn fal_provider_without_key_fails_but_fixture_provider_is_fine() {
        let needs_key = Facts {
            provider_is_fal: true,
            fal_key: false,
            ..all_good()
        };
        assert_eq!(worst(&evaluate(&needs_key)), Status::Fail);

        let fixture = Facts {
            provider_is_fal: false,
            fal_key: false,
            ..all_good()
        };
        assert_eq!(worst(&evaluate(&fixture)), Status::Ok);
    }

    #[test]
    fn starter_config_round_trips_through_config_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doomscrum.toml"),
            starter_config_toml("../some-repo"),
        )
        .unwrap();
        let cfg = crate::config::Config::load(dir.path()).unwrap();
        assert_eq!(cfg.repo.path, "../some-repo");
        // Dogfoods the opencode default.
        assert_eq!(cfg.agent.implement_cmd[0], "opencode");
    }
}
