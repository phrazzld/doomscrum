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

impl Dispatcher {
    /// Create and persist a queued receipt. Cheap and synchronous so the
    /// swipe endpoint can return it immediately; `run` does the work.
    pub fn create(&self, prd: &PrdSource, kind: DispatchKind) -> Result<DispatchReceipt> {
        std::fs::create_dir_all(&self.dispatches_dir)?;
        std::fs::create_dir_all(&self.worktrees_dir)?;
        let created_at = now_rfc3339();
        let id = sha256_hex(format!("{}:{:?}:{created_at}", prd.sha256, kind).as_bytes());
        let branch = format!(
            "specifi/{}-{}-{}",
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
        self.stage(receipt, "agent", &cmd, &worktree).await?;

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
                    &format!("specifi: agent output for {}", prd.title),
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
                    &format!("Specifi {}: {}", receipt.kind.verb(), prd.title),
                ),
                ("body_file", &body_file.to_string_lossy()),
                ("worktree", receipt.worktree.as_str()),
            ],
        );
        let result = self.run_cmd(receipt, "pr", &pr_cmd, &worktree).await?;
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
    ) -> Result<()> {
        let result = self.run_cmd(receipt, name, cmd, cwd).await?;
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
    ) -> Result<CmdResult> {
        use std::io::Write;
        let output = tokio::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .with_context(|| format!("spawning stage '{name}': {}", cmd.join(" ")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&receipt.agent_log)
        {
            let _ = writeln!(log, "==> stage {name}: {}", cmd.join(" "));
            let _ = log.write_all(&output.stdout);
            let _ = log.write_all(&output.stderr);
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
        std::fs::write(&path, serde_json::to_string_pretty(receipt)?)
            .with_context(|| format!("writing {}", path.display()))
    }
}

/// All dispatch receipts, newest first.
pub fn load_receipts(dispatches_dir: &Path) -> Result<Vec<DispatchReceipt>> {
    let mut receipts = Vec::new();
    let entries = match std::fs::read_dir(dispatches_dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(receipts),
        Err(err) => return Err(err.into()),
    };
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
                    receipts.push(receipt);
                }
            }
        }
    }
    receipts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(receipts)
}

fn build_prompt(kind: DispatchKind, prd: &PrdSource, branch: &str) -> String {
    match kind {
        DispatchKind::Implement => format!(
            "Implement the following spec completely.\n\n\
             You are working in a dedicated git worktree on branch `{branch}`. \
             Follow the repository's existing conventions, write tests where the repo has tests, \
             and run the repo's checks if any are configured. \
             Commit all of your work to the current branch with clear messages. Do not push.\n\n\
             Source spec ({path}):\n\n{raw}",
            branch = branch,
            path = prd.rel_path,
            raw = prd.raw,
        ),
        DispatchKind::Shape => format!(
            "Improve the following spec so it is ready for implementation. Do not implement it.\n\n\
             You are working in a dedicated git worktree on branch `{branch}`. \
             Edit the spec file `{path}` in place: sharpen the problem statement and goal, \
             add concrete testable acceptance criteria, surface ambiguities and open questions, \
             and add any context from this repository that an implementer would need. \
             Keep the existing markdown section structure. \
             Commit your changes to the current branch with a clear message. Do not push.\n\n\
             Current spec content:\n\n{raw}",
            branch = branch,
            path = prd.rel_path,
            raw = prd.raw,
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
         Dispatched from Specifi (swipe feed).\n\n\
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
        assert!(a.branch.starts_with("specifi/impl-demo-spec-"));
        let on_disk =
            std::fs::read_to_string(dir.path().join("dispatches").join(format!("{}.json", a.id)))
                .unwrap();
        assert!(on_disk.contains("queued"));

        let b = dispatcher.create(&prd(), DispatchKind::Shape).unwrap();
        assert!(b.branch.starts_with("specifi/shape-demo-spec-"));
        assert_ne!(a.branch, b.branch);
    }

    #[test]
    fn prompts_carry_full_spec_and_branch() {
        let p = prd();
        let implement = build_prompt(DispatchKind::Implement, &p, "specifi/impl-x");
        assert!(implement.contains("## Goal"));
        assert!(implement.contains("specifi/impl-x"));
        assert!(implement.contains("Do not push"));
        let shape = build_prompt(DispatchKind::Shape, &p, "specifi/shape-x");
        assert!(shape.contains("Do not implement it"));
        assert!(shape.contains("backlog.d/demo.md"));
    }
}
