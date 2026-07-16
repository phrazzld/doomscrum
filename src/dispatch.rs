use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

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
    /// Explicit shape action: sharpen the spec itself, open a PR with the improved spec.
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
    /// The spec's repo-relative path — the stable key across content edits
    /// (`prd_id`/`prd_sha256` are content hashes and change on re-shape). Used
    /// to badge a receipt `superseded` once its spec is re-shaped to a new sha.
    #[serde(default)]
    pub prd_rel_path: String,
    pub kind: DispatchKind,
    pub branch: String,
    pub worktree: String,
    /// queued | agent_running | opening_pr | pr_opened | completed_local |
    /// failed | cancelled
    pub status: String,
    pub stages: Vec<Stage>,
    /// Added+removed lines in the agent's diff — the triage size signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_lines: Option<u32>,
    /// The agent's own one-line summary (its HEAD commit subject).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    /// Live remote PR state, reconciled from `gh pr view` on feed polls:
    /// `open` | `merged` | `closed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_state: Option<String>,
    /// When [`Self::pr_state`] was last reconciled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_state_at: Option<String>,
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
    /// Serializes the queued→cancelled (cancel) and queued→agent_running (claim)
    /// transitions so a mis-swipe undo and the agent start can't both "win" —
    /// exactly one flips the receipt out of `queued`. Shared across the cancel
    /// request and the run task (cloned from AppCtx).
    pub state_lock: Arc<Mutex<()>>,
}

/// Outcome of claiming a queued dispatch for execution.
enum Claim {
    /// Was `queued`; now `agent_running` — proceed.
    Started,
    /// Cancelled during the undo window — bail with zero git side-effects.
    Cancelled,
    /// Not `queued` (already running/terminal) — do not double-run.
    Stale,
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
            prd_rel_path: prd.rel_path.clone(),
            kind,
            branch,
            worktree: worktree.to_string_lossy().to_string(),
            status: "queued".into(),
            stages: Vec::new(),
            diff_lines: None,
            plan: None,
            pr_url: None,
            pr_state: None,
            pr_state_at: None,
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

    /// Cancel a still-`queued` dispatch (the mis-swipe undo). Returns true if it
    /// was queued and is now `cancelled`; false if it already started or is
    /// unknown — so undo can never abort an agent that has touched git.
    pub fn cancel(&self, id: &str) -> Result<bool> {
        let _guard = self.lock();
        let Some(mut receipt) = self.load_one(id) else {
            return Ok(false);
        };
        if receipt.status != "queued" {
            return Ok(false); // already started or terminal — too late to undo
        }
        receipt.status = "cancelled".into();
        receipt.note = Some("cancelled during the undo window".into());
        receipt.updated_at = now_rfc3339();
        self.persist(&receipt)?;
        Ok(true)
    }

    /// Force a non-terminal dispatch to `failed` with an explanatory note —
    /// the recovery path for a dispatch whose driving task is gone (a panic in
    /// the detached run task, or a server crash that stranded the receipt).
    /// Terminal receipts are left alone. Returns true if the receipt flipped.
    pub fn mark_failed(&self, id: &str, note: &str) -> Result<bool> {
        let _guard = self.lock();
        let Some(mut receipt) = self.load_one(id) else {
            return Ok(false);
        };
        if matches!(
            receipt.status.as_str(),
            "pr_opened" | "completed_local" | "failed" | "cancelled"
        ) {
            return Ok(false);
        }
        receipt.status = "failed".into();
        receipt.note = Some(note.into());
        receipt.updated_at = now_rfc3339();
        self.persist(&receipt)?;
        Ok(true)
    }

    /// Boot-time reconcile: any receipt still in an in-flight status
    /// (`queued` / `agent_running` / `opening_pr`) was stranded by a crash —
    /// the tokio task that owned it died with the old process, so the status
    /// would otherwise stay frozen forever, and GC would keep protecting its
    /// orphaned worktree (`is_active_dispatch`). Flip each to `failed` so the
    /// feed shows the truth and GC can reclaim the worktree. Returns the
    /// receipts that were reconciled.
    pub fn reconcile_stranded(&self) -> Result<Vec<DispatchReceipt>> {
        let mut reconciled = Vec::new();
        for receipt in load_receipts(&self.dispatches_dir)? {
            let stranded = matches!(
                receipt.status.as_str(),
                "queued" | "agent_running" | "opening_pr"
            );
            if stranded
                && self.mark_failed(
                    &receipt.id,
                    "stranded by server restart — reconciled on boot",
                )?
            {
                if let Some(updated) = self.load_one(&receipt.id) {
                    reconciled.push(updated);
                }
            }
        }
        Ok(reconciled)
    }

    /// Best-effort PR outcome reconcile for feed reads. A stale or missing
    /// `gh` must not break `/api/state`; it just leaves the last known receipt
    /// state in place until a later poll succeeds.
    pub fn reconcile_pr_states(&self) -> Result<Vec<DispatchReceipt>> {
        let gh = std::env::var_os("DOOMSCRUM_GH_BIN").unwrap_or_else(|| "gh".into());
        self.reconcile_pr_states_with(&gh)
    }

    pub fn reconcile_pr_states_with(
        &self,
        gh: impl AsRef<std::ffi::OsStr>,
    ) -> Result<Vec<DispatchReceipt>> {
        for mut receipt in load_receipts(&self.dispatches_dir)? {
            if !should_reconcile_pr_state(&receipt) {
                continue;
            }
            let Some(url) = receipt.pr_url.as_deref() else {
                continue;
            };
            if let Ok(Some(state)) = query_pr_state(gh.as_ref(), &self.repo, url) {
                if receipt.pr_state.as_deref() != Some(state.as_str()) {
                    receipt.pr_state = Some(state);
                    receipt.pr_state_at = Some(now_rfc3339());
                    receipt.updated_at = now_rfc3339();
                    self.persist(&receipt)?;
                }
            }
        }
        load_receipts(&self.dispatches_dir)
    }

    /// Atomically transition a queued dispatch to `agent_running`, or report it
    /// cancelled/stale. Shares `state_lock` with [`cancel`](Self::cancel), so the
    /// undo and the agent start can never both win the race past `queued`.
    fn claim(&self, id: &str) -> Result<Claim> {
        let _guard = self.lock();
        let Some(mut receipt) = self.load_one(id) else {
            return Ok(Claim::Stale);
        };
        match receipt.status.as_str() {
            "queued" => {
                receipt.status = "agent_running".into();
                receipt.updated_at = now_rfc3339();
                // Claim only if the transition is durable: if this persist fails,
                // disk stays `queued` and a racing cancel could still "succeed"
                // while we run — so propagate the error and don't start.
                self.persist(&receipt)?;
                Ok(Claim::Started)
            }
            "cancelled" => Ok(Claim::Cancelled),
            _ => Ok(Claim::Stale),
        }
    }

    /// Hold the state lock for an atomic status transition. Tolerates poisoning
    /// (a panic mid-transition shouldn't wedge all future cancels/claims).
    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.state_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Load one receipt by id, if present and parseable.
    fn load_one(&self, id: &str) -> Option<DispatchReceipt> {
        let raw = std::fs::read_to_string(self.dispatches_dir.join(format!("{id}.json"))).ok()?;
        serde_json::from_str::<DispatchReceipt>(&raw).ok()
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
        // Atomically claim the dispatch before any git work. If a cancel won the
        // race (or already landed during the undo window), bail BEFORE the
        // worktree stage so a mis-swipe leaves zero git side-effects.
        match self.claim(&receipt.id)? {
            Claim::Started => receipt.status = "agent_running".into(),
            Claim::Cancelled => {
                receipt.status = "cancelled".into();
                return Ok(());
            }
            Claim::Stale => return Ok(()),
        }

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
                ("model", self.agent.agent_model.as_str()),
            ],
        );
        // `claim` already flipped the receipt to `agent_running` (and persisted
        // it); the worktree stage re-persisted that status. No re-assert needed.
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

        // Refuse to advance if the agent's diff carries a secret-shaped token.
        // An agent with filesystem access could write an accessible credential
        // (an env var, ~/.secrets, ~/.codex/auth.json) into a committed file,
        // which a push/PR would exfiltrate. 033 closed env egress; this closes
        // the committed-file egress. (Restricting what the agent can *read* is
        // tracked in ticket 039.)
        // Scan per-commit patches (`log -p`), not the aggregate tree diff: a
        // push ships the whole branch history, so a secret added in one commit
        // and removed in a later one would still be exfiltrated. --text forces
        // binary blobs to diff as text so a secret can't hide in a .bin file.
        let diff = self
            .git(
                &worktree,
                &["log", "-p", "--text", &format!("{base}..HEAD")],
            )
            .await?;
        if crate::secrets::diff_adds_secret(&diff, &crate::secrets::known_values()) {
            bail!(
                "blocked: agent output contains a secret-shaped token — refusing to \
                 push or open a PR that could exfiltrate a credential (log: {})",
                receipt.agent_log
            );
        }

        // Triage signals: the diff size (added+removed lines) drives the
        // fast-merge vs needs-review badge, and the agent's HEAD commit subject
        // is its one-line plan. Best-effort — never fail a dispatch over these.
        let numstat = self
            .git(&worktree, &["diff", "--numstat", &format!("{base}..HEAD")])
            .await
            .unwrap_or_default();
        receipt.diff_lines = Some(diff_line_count(&numstat));
        if let Ok(subject) = self
            .git(&worktree, &["log", "-1", "--format=%s", "HEAD"])
            .await
        {
            let subject = subject.trim();
            if !subject.is_empty() {
                receipt.plan = Some(subject.to_string());
            }
        }
        self.persist(receipt)?;

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

/// Diff size above which a dispatch is flagged needs-review rather than
/// fast-merge — the line between a glance and a real review.
const REVIEW_THRESHOLD_LINES: u32 = 80;

/// Total changed lines (added + removed) from `git diff --numstat` output.
/// Binary files (`-\t-`) contribute nothing.
fn diff_line_count(numstat: &str) -> u32 {
    // u64 + saturating arithmetic so a pathological (attacker-controlled) diff
    // can't overflow/panic or wrap to a small count; clamp to u32 for storage —
    // a multi-billion-line diff is unambiguously "needs-review".
    let total = numstat.lines().fold(0u64, |acc, l| {
        let mut cols = l.split('\t');
        let added = cols.next().and_then(|c| c.parse::<u64>().ok()).unwrap_or(0);
        let removed = cols.next().and_then(|c| c.parse::<u64>().ok()).unwrap_or(0);
        acc.saturating_add(added).saturating_add(removed)
    });
    total.min(u32::MAX as u64) as u32
}

/// Triage badge from a diff size: small diffs are `fast-merge`, larger ones
/// `needs-review`. Keeps swipe-dispatch from becoming a firehose of
/// unreviewable PRs.
pub fn review_size(diff_lines: u32) -> &'static str {
    if diff_lines <= REVIEW_THRESHOLD_LINES {
        "fast-merge"
    } else {
        "needs-review"
    }
}

fn should_reconcile_pr_state(receipt: &DispatchReceipt) -> bool {
    let Some(url) = receipt.pr_url.as_deref() else {
        return false;
    };
    if !is_github_pr_url(url) {
        return false;
    }
    !matches!(receipt.pr_state.as_deref(), Some("merged" | "closed"))
}

fn is_github_pr_url(url: &str) -> bool {
    url.starts_with("https://github.com/") && url.contains("/pull/")
}

#[derive(Deserialize)]
struct GhPrView {
    state: Option<String>,
    #[serde(rename = "mergedAt")]
    merged_at: Option<String>,
    #[serde(rename = "closedAt")]
    closed_at: Option<String>,
}

fn query_pr_state(gh: &std::ffi::OsStr, repo: &Path, pr_url: &str) -> Result<Option<String>> {
    let output = std::process::Command::new(gh)
        .args(["pr", "view", pr_url, "--json", "state,mergedAt,closedAt"])
        .current_dir(repo)
        .output()
        .with_context(|| "spawning gh pr view")?;
    if !output.status.success() {
        return Ok(None);
    }
    let view: GhPrView = serde_json::from_slice(&output.stdout)?;
    let state = view.state.unwrap_or_default().to_ascii_uppercase();
    if view.merged_at.is_some() || state == "MERGED" {
        Ok(Some("merged".into()))
    } else if state == "CLOSED" || view.closed_at.is_some() {
        Ok(Some("closed".into()))
    } else if state == "OPEN" {
        Ok(Some("open".into()))
    } else {
        Ok(None)
    }
}

/// True if this implement receipt targeted a spec version that has since been
/// re-shaped: a current spec shares its path but carries a different content
/// sha (`prd_id`/`prd_sha256` are content hashes, so a re-shape mints a new
/// spec and orphans the old receipt). Such a PR implements a stale spec — badge
/// it superseded rather than leave it dangling. `current` maps rel_path → sha256.
pub fn is_superseded(
    receipt: &DispatchReceipt,
    current: &std::collections::HashMap<String, String>,
) -> bool {
    if receipt.kind != DispatchKind::Implement || receipt.prd_rel_path.is_empty() {
        return false;
    }
    current
        .get(&receipt.prd_rel_path)
        .is_some_and(|cur_sha| cur_sha != &receipt.prd_sha256)
}

fn spec_source_label(prd: &PrdSource) -> String {
    prd.issue_number
        .map(|n| format!("GitHub issue #{n}"))
        .unwrap_or_else(|| prd.rel_path.clone())
}

fn build_prompt(kind: DispatchKind, prd: &PrdSource, branch: &str) -> String {
    // The spec body is untrusted (it can come from a foreign repo); fence it so
    // embedded directives can't hijack the agent. The trusted task instruction
    // stays outside the fence.
    let spec = crate::util::wrap_untrusted_spec(&prd.raw);
    let source_label = spec_source_label(prd);
    match kind {
        DispatchKind::Implement => format!(
            "Implement the following spec completely.\n\n\
             You are working in a dedicated git worktree on branch `{branch}`. \
             Follow the repository's existing conventions, write tests where the repo has tests, \
             and run the repo's checks if any are configured. \
             Commit all of your work to the current branch with clear messages. Do not push.\n\n\
             Source spec ({path}):\n\n{spec}",
            branch = branch,
            path = source_label,
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
            path = source_label,
            spec = spec,
        ),
    }
}

fn pr_body(receipt: &DispatchReceipt, prd: &PrdSource) -> String {
    let action = match receipt.kind {
        DispatchKind::Implement => "Implementation of",
        DispatchKind::Shape => "Spec shaping for",
    };
    let mut body = format!(
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
    );
    if let Some(n) = prd.issue_number {
        body.push_str(&format!("\nFixes #{n}\n"));
    }
    body
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
            issue_number: None,
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
            state_lock: Arc::new(Mutex::new(())),
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

    fn issue_prd(issue_number: u64) -> PrdSource {
        let raw = "# Issue Title\n\nIssue body.\n";
        PrdSource {
            id: sha256_hex(raw.as_bytes()),
            sha256: sha256_hex(raw.as_bytes()),
            rel_path: format!("github-issues/{issue_number}.md"),
            abs_path: PathBuf::from(format!("github-issues/{issue_number}.md")),
            title: "Issue Title".into(),
            priority: 0,
            raw: raw.into(),
            issue_number: Some(issue_number),
        }
    }

    fn dispatch_receipt(kind: DispatchKind) -> DispatchReceipt {
        DispatchReceipt {
            id: "rid".into(),
            prd_id: "pid".into(),
            prd_sha256: "sha".into(),
            prd_title: "T".into(),
            prd_rel_path: "p".into(),
            kind,
            branch: "b".into(),
            worktree: "/tmp/w".into(),
            status: "queued".into(),
            stages: Vec::new(),
            diff_lines: None,
            plan: None,
            pr_url: None,
            pr_state: None,
            pr_state_at: None,
            note: None,
            agent_log: "/tmp/l".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    #[test]
    fn pr_body_appends_fixes_for_issue_sourced_spec() {
        let receipt = dispatch_receipt(DispatchKind::Implement);
        let body = pr_body(&receipt, &issue_prd(42));
        assert!(body.contains("Fixes #42"), "{body}");
    }

    #[test]
    fn pr_body_omits_fixes_for_markdown_sourced_spec() {
        let receipt = dispatch_receipt(DispatchKind::Implement);
        let body = pr_body(&receipt, &prd());
        assert!(!body.contains("Fixes #"), "{body}");
    }

    #[test]
    fn prompt_names_issue_for_issue_sourced_spec() {
        let p = issue_prd(99);
        let implement = build_prompt(DispatchKind::Implement, &p, "doomscrum/impl-x");
        assert!(implement.contains("GitHub issue #99"), "{implement}");
        assert!(!implement.contains("github-issues/99.md"), "{implement}");
        let shape = build_prompt(DispatchKind::Shape, &p, "doomscrum/shape-x");
        assert!(shape.contains("GitHub issue #99"), "{shape}");
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
            state_lock: Arc::new(Mutex::new(())),
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

    fn run_git(repo: &std::path::Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git")
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    #[tokio::test]
    async fn dispatch_blocks_push_when_agent_diff_carries_a_secret() {
        // End-to-end: an agent that writes a credential into a committed file
        // must NOT advance to push/PR — the dispatch fails with a clear note.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        run_git(&repo, &["init", "-q", "-b", "main"]);
        run_git(&repo, &["config", "user.email", "t@t"]);
        run_git(&repo, &["config", "user.name", "t"]);
        run_git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("README.md"), "seed\n").unwrap();
        run_git(&repo, &["add", "-A"]);
        run_git(&repo, &["commit", "-qm", "seed"]);

        let agent = AgentConfig {
            implement_cmd: vec![
                "/bin/sh".into(),
                "-c".into(),
                "printf 'token = \"sk-or-v1-EXFIL1234567890\"\\n' > leaked.txt".into(),
            ],
            open_pr: false,
            env_allowlist: vec!["PATH".into(), "HOME".into()],
            ..AgentConfig::default()
        };
        let dispatcher = Dispatcher {
            repo: repo.clone(),
            dispatches_dir: dir.path().join("d"),
            worktrees_dir: dir.path().join("w"),
            agent,
            state_lock: Arc::new(Mutex::new(())),
        };
        let receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let done = dispatcher.run(receipt, prd()).await;

        assert_eq!(done.status, "failed", "dispatch must not advance: {done:?}");
        let note = done.note.unwrap_or_default();
        assert!(
            note.contains("secret-shaped token"),
            "expected a secret-block note, got: {note}"
        );
    }

    #[tokio::test]
    async fn dispatch_scan_sees_secrets_hidden_in_binary_files() {
        // A secret in a file git treats as binary must still block — the scan
        // diffs with --text so "Binary files differ" can't hide it.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        run_git(&repo, &["init", "-q", "-b", "main"]);
        run_git(&repo, &["config", "user.email", "t@t"]);
        run_git(&repo, &["config", "user.name", "t"]);
        run_git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("README.md"), "seed\n").unwrap();
        run_git(&repo, &["add", "-A"]);
        run_git(&repo, &["commit", "-qm", "seed"]);

        // A NUL byte makes git classify the file as binary.
        let agent = AgentConfig {
            implement_cmd: vec![
                "/bin/sh".into(),
                "-c".into(),
                "printf 'sk-or-v1-BINEXFIL1234567890\\0\\n' > leaked.bin".into(),
            ],
            open_pr: false,
            env_allowlist: vec!["PATH".into(), "HOME".into()],
            ..AgentConfig::default()
        };
        let dispatcher = Dispatcher {
            repo: repo.clone(),
            dispatches_dir: dir.path().join("d"),
            worktrees_dir: dir.path().join("w"),
            agent,
            state_lock: Arc::new(Mutex::new(())),
        };
        let receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let done = dispatcher.run(receipt, prd()).await;
        assert_eq!(
            done.status, "failed",
            "secret hidden in a binary file must block: {done:?}"
        );
    }

    #[test]
    fn cancel_flips_queued_to_cancelled_and_rejects_started() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        assert_eq!(receipt.status, "queued");
        // queued → cancellable
        assert!(dispatcher.cancel(&receipt.id).unwrap());
        assert_eq!(
            dispatcher.load_one(&receipt.id).unwrap().status,
            "cancelled"
        );
        // already-cancelled (not queued) → rejected; unknown id → rejected
        assert!(!dispatcher.cancel(&receipt.id).unwrap());
        assert!(!dispatcher.cancel("nope").unwrap());
    }

    #[test]
    fn claim_and_cancel_are_mutually_exclusive() {
        // The race codex flagged: claim (queued→agent_running) and cancel
        // (queued→cancelled) share state_lock, so exactly one wins.
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());

        // claim first → it transitions to agent_running; a later cancel is refused.
        let a = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        assert!(matches!(dispatcher.claim(&a.id).unwrap(), Claim::Started));
        assert_eq!(dispatcher.load_one(&a.id).unwrap().status, "agent_running");
        assert!(
            !dispatcher.cancel(&a.id).unwrap(),
            "cancel after the agent started must be rejected"
        );

        // cancel first → a later claim reports Cancelled and never starts.
        let b = dispatcher.create(&prd(), DispatchKind::Shape).unwrap();
        assert!(dispatcher.cancel(&b.id).unwrap());
        assert!(matches!(dispatcher.claim(&b.id).unwrap(), Claim::Cancelled));
        assert_eq!(dispatcher.load_one(&b.id).unwrap().status, "cancelled");
    }

    #[tokio::test]
    async fn cancelled_dispatch_runs_with_zero_git_side_effects() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let receipt = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        assert!(dispatcher.cancel(&receipt.id).unwrap());
        // run() must bail before the worktree stage — no git touched at all.
        let done = dispatcher.run(receipt.clone(), prd()).await;
        assert_eq!(done.status, "cancelled");
        assert!(
            !std::path::Path::new(&receipt.worktree).exists(),
            "cancel must leave no worktree dir"
        );
        assert!(
            done.stages.is_empty(),
            "no stages should run: {:?}",
            done.stages
        );
    }

    #[test]
    fn diff_line_count_sums_changes_skipping_binary() {
        assert_eq!(
            diff_line_count("10\t2\tsrc/a.rs\n0\t5\tsrc/b.rs\n-\t-\tlogo.png\n"),
            17
        );
        assert_eq!(diff_line_count(""), 0);
        // A pathological diff clamps to u32::MAX (stays needs-review) — no panic,
        // no wrap to a small fast-merge count.
        assert_eq!(diff_line_count("9999999999\t9999999999\tbig.bin"), u32::MAX);
        assert_eq!(
            review_size(diff_line_count("9999999999\t9999999999\tx")),
            "needs-review"
        );
    }

    #[test]
    fn review_size_badges_by_threshold() {
        assert_eq!(review_size(0), "fast-merge");
        assert_eq!(review_size(REVIEW_THRESHOLD_LINES), "fast-merge");
        assert_eq!(review_size(REVIEW_THRESHOLD_LINES + 1), "needs-review");
    }

    #[test]
    fn reconcile_stranded_fails_in_flight_receipts_and_leaves_terminal_ones() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());

        // queued → stranded; agent_running → stranded; terminal → untouched.
        let queued = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let running = dispatcher.create(&prd(), DispatchKind::Shape).unwrap();
        assert!(matches!(
            dispatcher.claim(&running.id).unwrap(),
            Claim::Started
        ));
        // A distinct spec: receipt ids hash (prd_sha256, kind, created_at) at
        // millisecond precision, so a same-spec same-kind receipt created in
        // the same instant would collide with `queued` and overwrite it.
        let other_raw = "# Other Spec\n\n## Goal\nShip the other thing.\n";
        let other = PrdSource {
            id: sha256_hex(other_raw.as_bytes()),
            sha256: sha256_hex(other_raw.as_bytes()),
            rel_path: "backlog.d/other.md".into(),
            abs_path: PathBuf::from("backlog.d/other.md"),
            title: "Other Spec".into(),
            priority: 0,
            raw: other_raw.into(),
            issue_number: None,
        };
        let mut done = dispatcher.create(&other, DispatchKind::Implement).unwrap();
        done.status = "pr_opened".into();
        dispatcher.persist(&done).unwrap();

        let reconciled = dispatcher.reconcile_stranded().unwrap();
        let mut ids: Vec<&str> = reconciled.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        let mut expected = vec![queued.id.as_str(), running.id.as_str()];
        expected.sort_unstable();
        assert_eq!(ids, expected);
        for r in &reconciled {
            assert_eq!(r.status, "failed");
            assert!(
                r.note.as_deref().unwrap_or_default().contains("stranded"),
                "{r:?}"
            );
        }
        assert_eq!(dispatcher.load_one(&done.id).unwrap().status, "pr_opened");
        // A second boot reconciles nothing — idempotent.
        assert!(dispatcher.reconcile_stranded().unwrap().is_empty());
    }

    #[test]
    fn mark_failed_flips_in_flight_but_never_terminal_receipts() {
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let r = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        assert!(dispatcher.mark_failed(&r.id, "task panicked").unwrap());
        let loaded = dispatcher.load_one(&r.id).unwrap();
        assert_eq!(loaded.status, "failed");
        assert_eq!(loaded.note.as_deref(), Some("task panicked"));
        // Already failed → refuse to overwrite; unknown id → false.
        assert!(!dispatcher.mark_failed(&r.id, "again").unwrap());
        assert!(!dispatcher.mark_failed("nope", "x").unwrap());
    }

    #[test]
    fn superseded_when_same_path_resolves_to_a_new_sha() {
        use std::collections::HashMap;
        let dir = tempfile::tempdir().unwrap();
        let dispatcher = test_dispatcher(dir.path());
        let r = dispatcher.create(&prd(), DispatchKind::Implement).unwrap();
        let path = r.prd_rel_path.clone();

        // same sha at the path → current, not superseded
        let mut cur = HashMap::from([(path.clone(), r.prd_sha256.clone())]);
        assert!(!is_superseded(&r, &cur));
        // spec re-shaped to a new sha at the same path → superseded
        cur.insert(path, "newsha".into());
        assert!(is_superseded(&r, &cur));
        // a shape receipt is never superseded; an unknown path isn't either
        let shape = dispatcher.create(&prd(), DispatchKind::Shape).unwrap();
        assert!(!is_superseded(&shape, &cur));
        assert!(!is_superseded(&r, &HashMap::new()));
    }
}
