use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::backlog::PrdSource;
use crate::config::{substitute, AgentConfig};
use crate::util::{now_rfc3339, sha256_hex, short, slug};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchKind {
    /// Right swipe: implement the spec, open a PR.
    Implement,
    /// Left swipe: sharpen the spec itself, open a PR with the improved spec.
    Shape,
}

impl DispatchKind {
    fn verb(self) -> &'static str {
        match self {
            DispatchKind::Implement => "impl",
            DispatchKind::Shape => "shape",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub ok: bool,
}

/// Durable record of one agent dispatch, persisted after every stage so the
/// feed can show live status and nothing is lost if the server dies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchReceipt {
    pub id: String,
    pub prd_id: String,
    pub prd_sha256: String,
    pub prd_title: String,
    pub kind: DispatchKind,
    pub branch: String,
    pub worktree: String,
    /// queued | agent_running | opening_pr | pr_opened | completed_local | failed
    pub status: String,
    pub stages: Vec<Stage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub agent_log: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Dispatcher {
    /// The synced repo agents work against.
    pub repo: PathBuf,
    pub dispatches_dir: PathBuf,
    pub worktrees_dir: PathBuf,
    pub agent: AgentConfig,
}

struct CmdResult {
    exit_code: Option<i32>,
    stdout: String,
}

/// How a spawned stage's process environment is built.
///
/// `run_cmd` is the single spawn path for every stage. The git worktree, the
/// `git push`, and the `gh pr create` stages are trusted (our own argv) and
/// need the operator's credentials, so they `Inherit`. The **agent** stage runs
/// untrusted spec content from a possibly-foreign repo, so it gets an
/// `Allowlist`: `env_clear` then re-add only the named vars, keeping DoomScrum's
/// service keys and git push tokens out of the agent's reach entirely.
#[derive(Clone, Copy)]
enum EnvPolicy<'a> {
    Inherit,
    Allowlist(&'a [String]),
}

/// The (key, value) pairs an allowlisted agent stage may inherit: the configured
/// allowlist, minus any service-secret name (a hard denylist that defends against
/// a misconfigured `env_allowlist` listing `FAL_API_KEY` and friends), keeping
/// only the vars `lookup` actually resolves. `lookup` is injected so the policy
/// is testable without mutating the process environment.
fn agent_env(allow: &[String], lookup: impl Fn(&str) -> Option<String>) -> Vec<(&str, String)> {
    allow
        .iter()
        .map(String::as_str)
        .filter(|k| !crate::secrets::is_service_secret_name(k))
        .filter_map(|k| lookup(k).map(|v| (k, v)))
        .collect()
}

impl Dispatcher {
    /// Create and persist a queued receipt. Cheap and synchronous so the
    /// swipe endpoint can return it immediately; `run` does the work.
    pub fn create(&self, prd: &PrdSource, kind: DispatchKind) -> Result<DispatchReceipt> {
        std::fs::create_dir_all(&self.dispatches_dir)?;
        std::fs::create_dir_all(&self.worktrees_dir)?;
        let created_at = now_rfc3339();
        let id = sha256_hex(format!("{}:{:?}:{created_at}", prd.sha256, kind).as_bytes());
        let branch = format!(
            "doomscrum/{}-{}-{}",
            kind.verb(),
            slug(&prd.title),
            short(&id)
        );
        let worktree = self.worktrees_dir.join(branch.replace('/', "-"));
        let receipt = DispatchReceipt {
            id: id.clone(),
            prd_id: prd.id.clone(),
            prd_sha256: prd.sha256.clone(),
            prd_title: prd.title.clone(),
            kind,
            branch,
            worktree: worktree.to_string_lossy().to_string(),
            status: "queued".into(),
            stages: Vec::new(),
            pr_url: None,
            note: None,
            agent_log: self
                .dispatches_dir
                .join(format!("{id}.agent.log"))
                .to_string_lossy()
                .to_string(),
            created_at,
            updated_at: now_rfc3339(),
        };
        self.persist(&receipt)?;
        Ok(receipt)
    }

    /// Full pipeline: worktree → agent → commit → push → PR.
    /// Never panics the server; every failure lands in the receipt.
    pub async fn run(&self, mut receipt: DispatchReceipt, prd: PrdSource) -> DispatchReceipt {
        match self.run_inner(&mut receipt, &prd).await {
            Ok(()) => receipt,
            Err(err) => {
                receipt.status = "failed".into();
                receipt.note = Some(format!("{err:#}"));
                let _ = self.persist(&receipt);
                receipt
            }
        }
    }

    async fn run_inner(&self, receipt: &mut DispatchReceipt, prd: &PrdSource) -> Result<()> {
        let worktree = PathBuf::from(&receipt.worktree);

        // 1. Fresh worktree on a fresh branch.
        self.stage(
            receipt,
            "worktree",
            &[
                "git".into(),
                "-C".into(),
                self.repo.to_string_lossy().to_string(),
                "worktree".into(),
                "add".into(),
                receipt.worktree.clone(),
                "-b".into(),
                receipt.branch.clone(),
            ],
            &self.repo,
            EnvPolicy::Inherit,
        )
        .await?;
        let base = self.git(&worktree, &["rev-parse", "HEAD"]).await?;

        // 2. Run the agent with the spec as its mission.
        let prompt = build_prompt(receipt.kind, prd, &receipt.branch);
        let template = match receipt.kind {
            DispatchKind::Implement => &self.agent.implement_cmd,
            DispatchKind::Shape => &self.agent.shape_cmd,
        };
        let cmd = substitute(
            template,
            &[
                ("worktree", receipt.worktree.as_str()),
                ("prompt", prompt.as_str()),
                ("branch", receipt.branch.as_str()),
                ("spec_path", prd.rel_path.as_str()),
                ("title", prd.title.as_str()),
            ],
        );
        receipt.status = "agent_running".into();
        self.persist(receipt)?;
        // The agent stage is the one untrusted execution surface: its prompt
        // carries foreign-repo spec content. Scrub its environment to the
        // allowlist so DoomScrum's keys and git tokens are never in reach.
        self.stage(
            receipt,
            "agent",
            &cmd,
            &worktree,
            EnvPolicy::Allowlist(&self.agent.env_allowlist),
        )
        .await?;

        // 3. Commit anything the agent left uncommitted.
        let dirty = !self
            .git(&worktree, &["status", "--porcelain"])
            .await?
            .is_empty();
        if dirty {
            self.git(&worktree, &["add", "-A"]).await?;
            self.git(
                &worktree,
                &[
                    "commit",
                    "-m",
                    &format!("doomscrum: agent output for {}", prd.title),
                ],
            )
            .await?;
        }
        let commits = self
            .git(
                &worktree,
                &["rev-list", "--count", &format!("{base}..HEAD")],
            )
            .await?;
        if commits.trim() == "0" {
            bail!("agent produced no commits and no changes");
        }

        if !self.agent.open_pr {
            receipt.status = "completed_local".into();
            receipt.note = Some("open_pr disabled; branch left local".into());
            self.persist(receipt)?;
            return Ok(());
        }

        // 4. Push + PR, if the synced repo has a remote.
        let has_origin = self
            .git(&worktree, &["remote", "get-url", "origin"])
            .await
            .is_ok();
        if !has_origin {
            receipt.status = "completed_local".into();
            receipt.note = Some("no origin remote; branch left local".into());
            self.persist(receipt)?;
            return Ok(());
        }
        receipt.status = "opening_pr".into();
        self.persist(receipt)?;
        self.stage(
            receipt,
            "push",
            &[
                "git".into(),
                "push".into(),
                "-u".into(),
                "origin".into(),
                receipt.branch.clone(),
            ],
            &worktree,
            EnvPolicy::Inherit,
        )
        .await?;

        let body_file = self
            .dispatches_dir
            .join(format!("{}.pr-body.md", receipt.id));
        std::fs::write(&body_file, pr_body(receipt, prd))?;
        let pr_cmd = substitute(
            &self.agent.pr_cmd,
            &[
                ("branch", receipt.branch.as_str()),
                (
                    "title",
                    &format!("DoomScrum {}: {}", receipt.kind.verb(), prd.title),
                ),
                ("body_file", &body_file.to_string_lossy()),
                ("worktree", receipt.worktree.as_str()),
            ],
        );
        let result = self
            .run_cmd(receipt, "pr", &pr_cmd, &worktree, EnvPolicy::Inherit)
            .await?;
        receipt.pr_url = result
            .stdout
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| l.starts_with("http"))
            .map(String::from);
        receipt.status = "pr_opened".into();
        self.persist(receipt)?;
        Ok(())
    }

    /// Run a command as a named stage; record it; error if it fails.
    async fn stage(
        &self,
        receipt: &mut DispatchReceipt,
        name: &str,
        cmd: &[String],
        cwd: &Path,
        env: EnvPolicy<'_>,
    ) -> Result<()> {
        let result = self.run_cmd(receipt, name, cmd, cwd, env).await?;
        if result.exit_code != Some(0) {
            bail!(
                "stage '{name}' failed with exit code {:?} (log: {})",
                result.exit_code,
                receipt.agent_log
            );
        }
        Ok(())
    }

    async fn run_cmd(
        &self,
        receipt: &mut DispatchReceipt,
        name: &str,
        cmd: &[String],
        cwd: &Path,
        env: EnvPolicy<'_>,
    ) -> Result<CmdResult> {
        use std::io::Write;
        let mut command = tokio::process::Command::new(&cmd[0]);
        command
            .args(&cmd[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let EnvPolicy::Allowlist(allow) = env {
            // Build the child env from scratch: drop everything, then re-add only
            // the allowlisted, non-service-secret vars. DoomScrum's keys and git
            // push tokens never reach the untrusted agent.
            command.env_clear();
            for (key, val) in agent_env(allow, |k| std::env::var(k).ok()) {
                command.env(key, val);
            }
        }
        let output = command
            .output()
            .await
            .with_context(|| format!("spawning stage '{name}': {}", cmd.join(" ")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&receipt.agent_log)
        {
            // Redact before persisting: a credential that reaches the agent's
            // stdout must never land on disk (defense in depth behind the env
            // scrub). The header carries the full agent argv (incl. the spec
            // prompt), so it is redacted too.
            let stderr = String::from_utf8_lossy(&output.stderr);
            let block = format!("==> stage {name}: {}\n{stdout}{stderr}", cmd.join(" "));
            let _ = log.write_all(crate::secrets::redact_env(&block).as_bytes());
        }
        let exit_code = output.status.code();
        receipt.stages.push(Stage {
            name: name.into(),
            command: cmd.to_vec(),
            exit_code,
            ok: exit_code == Some(0),
        });
        receipt.updated_at = now_rfc3339();
        self.persist(receipt)?;
        Ok(CmdResult { exit_code, stdout })
    }

    async fn git(&self, cwd: &Path, args: &[&str]) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await?;
        if !output.status.success() {
            bail!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn persist(&self, receipt: &DispatchReceipt) -> Result<()> {
        std::fs::create_dir_all(&self.dispatches_dir)?;
        let path = self.dispatches_dir.join(format!("{}.json", receipt.id));
        // Redact on write (data-at-rest) and again on read (see load_receipts),
        // so neither the on-disk JSON nor any serving route carries a raw secret.
        let known = crate::secrets::known_values();
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&redacted_receipt(receipt, &known))?,
        )
        .with_context(|| format!("writing {}", path.display()))
    }
}

/// A copy of `receipt` with secrets masked in the fields that get served over
/// HTTP or written to disk: `note` (error text can echo a credential-bearing
/// remote URL) and every `stage.command` (the agent argv embeds the untrusted
/// spec, which could carry a key-shaped token). `known` is the caller-resolved
/// key set so a batch (load) resolves it once.
fn redacted_receipt(receipt: &DispatchReceipt, known: &[String]) -> DispatchReceipt {
    let mut r = receipt.clone();
    r.note = r.note.as_deref().map(|n| crate::secrets::redact(n, known));
    for stage in &mut r.stages {
        for arg in &mut stage.command {
            *arg = crate::secrets::redact(arg, known);
        }
    }
    r
}

/// All dispatch receipts, newest first. Receipts are redacted on read so that
/// even ones persisted before redaction shipped (or by an older build) cannot
/// leak a secret-shaped token through any route that serves them.
pub fn load_receipts(dispatches_dir: &Path) -> Result<Vec<DispatchReceipt>> {
    let mut receipts = Vec::new();
    let entries = match std::fs::read_dir(dispatches_dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(receipts),
        Err(err) => return Err(err.into()),
    };
    let known = crate::secrets::known_values();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_receipt = path.extension().is_some_and(|e| e == "json")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !n.ends_with(".pr-body.json"));
        if is_receipt {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(receipt) = serde_json::from_str::<DispatchReceipt>(&raw) {
                    receipts.push(redacted_receipt(&receipt, &known));
                }
            }
        }
    }
    receipts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(receipts)
}

fn build_prompt(kind: DispatchKind, prd: &PrdSource, branch: &str) -> String {
    // The spec body is untrusted (it can come from a foreign repo); fence it so
    // embedded directives can't hijack the agent. The trusted task instruction
    // stays outside the fence.
    let spec = crate::util::wrap_untrusted_spec(&prd.raw);
    match kind {
        DispatchKind::Implement => format!(
            "Implement the following spec completely.\n\n\
             You are working in a dedicated git worktree on branch `{branch}`. \
             Follow the repository's existing conventions, write tests where the repo has tests, \
             and run the repo's checks if any are configured. \
             Commit all of your work to the current branch with clear messages. Do not push.\n\n\
             Source spec ({path}):\n\n{spec}",
            branch = branch,
            path = prd.rel_path,
            spec = spec,
        ),
        DispatchKind::Shape => format!(
            "Improve the following spec so it is ready for implementation. Do not implement it.\n\n\
             You are working in a dedicated git worktree on branch `{branch}`. \
             Edit the spec file `{path}` in place: sharpen the problem statement and goal, \
             add concrete testable acceptance criteria, surface ambiguities and open questions, \
             and add any context from this repository that an implementer would need. \
             Keep the existing markdown section structure. \
             Commit your changes to the current branch with a clear message. Do not push.\n\n\
             Current spec content:\n\n{spec}",
            branch = branch,
            path = prd.rel_path,
            spec = spec,
        ),
    }
}

fn pr_body(receipt: &DispatchReceipt, prd: &PrdSource) -> String {
    let action = match receipt.kind {
        DispatchKind::Implement => "Implementation of",
        DispatchKind::Shape => "Spec shaping for",
    };
    format!(
        "{action} `{path}`.\n\n\
         Dispatched from DoomScrum (swipe feed).\n\n\
         - Spec: `{path}`\n\
         - Spec sha256: `{sha}`\n\
         - Dispatch id: `{id}`\n\
         - Branch: `{branch}`\n",
        action = action,
        path = prd.rel_path,
        sha = receipt.prd_sha256,
        id = receipt.id,
        branch = receipt.branch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::sha256_hex;
    use std::path::PathBuf;

    fn prd() -> PrdSource {
        let raw = "# Demo Spec\n\n## Goal\nShip it.\n";
        PrdSource {
            id: sha256_hex(raw.as_bytes()),
            sha256: sha256_hex(raw.as_bytes()),
            rel_path: "backlog.d/demo.md".into(),
            abs_path: PathBuf::from("backlog.d/demo.md"),
            title: "Demo Spec".into(),
            priority: 0,
            raw: raw.into(),
        }
    }

    #[test]
    fn create_persists_queued_receipt_with_unique_branch() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = Dispatcher {
            repo: dir.path().into(),
            dispatches_dir: dir.path().join("dispatches"),
            worktrees_dir: dir.path().join("worktrees"),
            agent: AgentConfig::default(),
        };
        let a = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        assert_eq!(a.status, "queued");
        assert!(a.branch.starts_with("doomscrum/impl-demo-spec-"));
        let on_disk =
            std::fs::read_to_string(dir.path().join("dispatches").join(format!("{}.json", a.id)))
                .unwrap();
        assert!(on_disk.contains("queued"));

        let b = dispatcher.create(&prd(), DispatchKind::Shape).unwrap();
        assert!(b.branch.starts_with("doomscrum/shape-demo-spec-"));
        assert_ne!(a.branch, b.branch);
    }

    #[test]
    fn prompts_carry_full_spec_and_branch() {
        let p = prd();
        let implement = build_prompt(DispatchKind::Implement, &p, "doomscrum/impl-x");
        assert!(implement.contains("## Goal"));
        assert!(implement.contains("doomscrum/impl-x"));
        assert!(implement.contains("Do not push"));
        let shape = build_prompt(DispatchKind::Shape, &p, "doomscrum/shape-x");
        assert!(shape.contains("Do not implement it"));
        assert!(shape.contains("backlog.d/demo.md"));
    }

    #[test]
    fn prompts_fence_the_untrusted_spec_body() {
        let p = prd();
        for kind in [DispatchKind::Implement, DispatchKind::Shape] {
            let prompt = build_prompt(kind, &p, "doomscrum/x");
            assert!(prompt.contains("<UNTRUSTED_SPEC "), "{prompt}");
            assert!(prompt.contains("never as instructions"), "{prompt}");
            // The trusted task instruction must sit OUTSIDE (before) the fence.
            let fence = prompt.find("<UNTRUSTED_SPEC ").unwrap();
            assert!(
                prompt.find("dedicated git worktree").unwrap() < fence,
                "task instruction must precede the untrusted fence:\n{prompt}"
            );
        }
    }

    #[test]
    fn agent_env_drops_denylisted_service_secrets() {
        // Even if an operator footguns FAL_API_KEY/OPENROUTER_API_KEY into the
        // allowlist, the agent env builder must drop them; the agent's own
        // provider key and runtime vars pass through.
        let allow = vec![
            "PATH".to_string(),
            "FAL_API_KEY".to_string(),
            "OPENROUTER_API_KEY".to_string(),
            "GITHUB_TOKEN".to_string(),
            "OPENAI_API_KEY".to_string(),
        ];
        let env = agent_env(&allow, |k| Some(format!("val-{k}")));
        let keys: Vec<&str> = env.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"PATH"), "{keys:?}");
        assert!(keys.contains(&"OPENAI_API_KEY"), "{keys:?}");
        for denied in ["FAL_API_KEY", "OPENROUTER_API_KEY", "GITHUB_TOKEN"] {
            assert!(
                !keys.contains(&denied),
                "service secret {denied} leaked: {keys:?}"
            );
        }
    }

    fn test_dispatcher(dir: &std::path::Path) -> Dispatcher {
        Dispatcher {
            repo: dir.into(),
            dispatches_dir: dir.join("dispatches"),
            worktrees_dir: dir.join("worktrees"),
            agent: AgentConfig::default(),
        }
    }

    #[tokio::test]
    async fn agent_stage_scrubs_env_to_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let mut receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let envfile = dir.path().join("childenv.txt");
        // The agent command dumps its own (child) environment to a file.
        let cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("env > '{}'", envfile.display()),
        ];
        // Allowlist PATH only. HOME is present in this test process's env but
        // deliberately excluded — the falsifier is HOME reaching the child.
        let allow = vec!["PATH".to_string()];
        dispatcher
            .run_cmd(
                &mut receipt,
                "agent",
                &cmd,
                dir.path(),
                EnvPolicy::Allowlist(&allow),
            )
            .await
            .unwrap();
        // Inspect only the child's variable NAMES — never echo env *values*,
        // which would leak the operator's real secrets into the test log.
        let dumped = std::fs::read_to_string(&envfile).unwrap();
        let keys: Vec<&str> = dumped
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .collect();
        assert!(
            keys.contains(&"PATH"),
            "allowlisted PATH must pass through; child keys: {keys:?}"
        );
        assert!(
            !keys.contains(&"HOME"),
            "HOME is in the parent env but excluded from the allowlist; \
             it must be scrubbed from the child; child keys: {keys:?}"
        );
    }

    #[tokio::test]
    async fn trusted_stage_inherits_env() {
        // Push/PR stages need the operator's git/gh credentials, so Inherit
        // must keep the parent env (HOME present here proves no scrub).
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let mut receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let envfile = dir.path().join("inherited.txt");
        // Write only whether HOME survived — never the whole environment.
        let cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!(
                "test -n \"$HOME\" && echo present > '{}'",
                envfile.display()
            ),
        ];
        dispatcher
            .run_cmd(&mut receipt, "push", &cmd, dir.path(), EnvPolicy::Inherit)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&envfile).unwrap_or_default().trim(),
            "present",
            "trusted stages must inherit the parent env (HOME)"
        );
    }

    #[tokio::test]
    async fn agent_log_redacts_secret_shaped_output() {
        // The falsifier: a spec coaxes the agent into printing a key. Even if a
        // key reaches stdout, it must not be persisted to the agent log.
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let mut receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'agent says sk-or-v1-ABCdef1234567890\\n'".to_string(),
        ];
        dispatcher
            .run_cmd(
                &mut receipt,
                "agent",
                &cmd,
                dir.path(),
                EnvPolicy::Allowlist(&dispatcher.agent.env_allowlist),
            )
            .await
            .unwrap();
        let log = std::fs::read_to_string(&receipt.agent_log).unwrap();
        assert!(
            !log.contains("sk-or-v1-ABCdef1234567890"),
            "persisted agent log leaked a key:\n{log}"
        );
        assert!(log.contains("[REDACTED]"), "{log}");
    }

    #[test]
    fn served_receipts_are_redacted_even_when_persisted_raw() {
        // Simulate a receipt written by an older build (before redaction): raw
        // secret-shaped tokens in `note` + `stage.command` on disk. Every route
        // (/api/dispatches, /api/state, /log) reads via load_receipts, which must
        // mask them — persist-time redaction alone wouldn't cover history.
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let mut receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        receipt.stages.push(Stage {
            name: "agent".into(),
            command: vec![
                "codex".into(),
                "spec body had sk-or-v1-PLANTED1234567890 in it".into(),
            ],
            exit_code: Some(0),
            ok: true,
        });
        receipt.note = Some("git push failed: https://x:ghp_NOTE1234567890@github.com".into());
        // Write RAW, bypassing persist's redaction, to mimic a pre-existing file.
        let path = dispatcher
            .dispatches_dir
            .join(format!("{}.json", receipt.id));
        std::fs::write(&path, serde_json::to_string_pretty(&receipt).unwrap()).unwrap();

        let loaded = load_receipts(&dispatcher.dispatches_dir).unwrap();
        let r = loaded.iter().find(|r| r.id == receipt.id).unwrap();
        let blob = format!("{r:?}");
        assert!(
            !blob.contains("sk-or-v1-PLANTED1234567890"),
            "stage command leaked on read: {blob}"
        );
        assert!(
            !blob.contains("ghp_NOTE1234567890"),
            "note leaked on read: {blob}"
        );
        assert!(blob.contains("[REDACTED]"), "{blob}");
    }
}
