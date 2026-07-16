use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::util::sha256_hex;

/// One backlog spec, either a local markdown file or an imported issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdSource {
    /// Content hash; doubles as the stable id for renders and decisions.
    pub id: String,
    pub sha256: String,
    /// Path relative to the synced repo root (e.g. `backlog.d/001-foo.md`
    /// or `github-issues/42.md`).
    pub rel_path: String,
    #[serde(skip)]
    pub abs_path: PathBuf,
    pub title: String,
    /// 0 = highest priority. Priority is filename sort order for markdown,
    /// issue number ascending for GitHub issues.
    pub priority: usize,
    pub raw: String,
    /// When the spec was imported from a GitHub issue, the issue number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_number: Option<u64>,
}

fn extract_title(raw: &str, filename: &str) -> String {
    raw.lines()
        .find_map(|line| line.strip_prefix("# ").map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            filename
                .trim_end_matches(".md")
                .replace(['-', '_'], " ")
                .trim()
                .to_string()
        })
}

/// Resolve the `gh` binary: `DOOMSCRUM_GH_BIN` may point at a custom binary
/// for testing; otherwise fall back to `gh` on PATH.
fn gh_bin() -> std::ffi::OsString {
    std::env::var_os("DOOMSCRUM_GH_BIN").unwrap_or_else(|| "gh".into())
}

/// Scan the backlog, highest priority first, capped at `max_items`.
///
/// `source` selects the adapter. `"markdown"` reads `backlog_dir` inside the
/// synced repo. `"github-issues"` lists open issues via the `gh` CLI.
pub fn scan(
    repo_root: &Path,
    source: &str,
    backlog_dir: &str,
    max_items: usize,
) -> Result<Vec<PrdSource>> {
    match source {
        "markdown" => scan_markdown(repo_root, backlog_dir, max_items),
        "github-issues" => scan_github_issues(repo_root, max_items),
        other => bail!("unknown backlog source {:?}", other),
    }
}

fn scan_markdown(repo_root: &Path, backlog_dir: &str, max_items: usize) -> Result<Vec<PrdSource>> {
    let dir = repo_root.join(backlog_dir);
    let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|ext| ext == "md")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| !n.starts_with('_') && !n.starts_with('.'))
            })
            .collect(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("reading {}", dir.display())),
    };
    files.sort();

    let mut prds = Vec::new();
    for (priority, path) in files.into_iter().take(max_items).enumerate() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let hash = sha256_hex(raw.as_bytes());
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let rel_path = path
            .strip_prefix(repo_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        prds.push(PrdSource {
            id: hash.clone(),
            sha256: hash,
            rel_path,
            abs_path: path,
            title: extract_title(&raw, &filename),
            priority,
            raw,
            issue_number: None,
        });
    }
    Ok(prds)
}

#[derive(Debug, Deserialize)]
struct GhLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    body: Option<String>,
    #[serde(default)]
    labels: Vec<GhLabel>,
}

/// List open GitHub issues via `gh` and map them to PrdSource.
/// Issues are sorted by number ascending (0 = smallest number = highest priority).
fn scan_github_issues(repo_root: &Path, max_items: usize) -> Result<Vec<PrdSource>> {
    scan_github_issues_with_bin(repo_root, max_items, &gh_bin())
}

fn scan_github_issues_with_bin(
    repo_root: &Path,
    max_items: usize,
    gh_bin: &OsStr,
) -> Result<Vec<PrdSource>> {
    if max_items == 0 {
        return Ok(Vec::new());
    }
    // Callers like the feed's scan_all pass usize::MAX for "everything", but
    // `gh --limit` parses an int64. Pagination beyond max_items is a non-goal,
    // so 1000 is the practical "all open issues".
    let limit = max_items.min(1000);
    let output = std::process::Command::new(gh_bin)
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--json",
            "number,title,body,labels,updatedAt",
            "--limit",
            &limit.to_string(),
        ])
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("spawning gh issue list in {}", repo_root.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "`gh issue list` failed in {}: {stderr}",
            repo_root.display()
        ));
    }

    let issues: Vec<GhIssue> =
        serde_json::from_slice(&output.stdout).with_context(|| "parsing `gh issue list` JSON")?;

    let mut issues = issues;
    // Stable priority: lowest issue number first.
    issues.sort_by_key(|i| i.number);

    let mut prds = Vec::new();
    for (priority, issue) in issues.into_iter().take(max_items).enumerate() {
        let body = issue.body.as_deref().unwrap_or("");
        let mut raw = format!("# {}\n\n{}\n", issue.title, body);
        raw.push_str("\n---\n\n");
        raw.push_str(&format!("Source: GitHub issue #{}\n", issue.number));

        let mut label_names: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
        label_names.sort_unstable();
        if !label_names.is_empty() {
            raw.push_str(&format!("Labels: {}\n", label_names.join(", ")));
        }

        let hash = sha256_hex(raw.as_bytes());
        let rel_path = format!("github-issues/{}.md", issue.number);
        prds.push(PrdSource {
            id: hash.clone(),
            sha256: hash,
            rel_path: rel_path.clone(),
            abs_path: repo_root.join(&rel_path),
            title: issue.title.clone(),
            priority,
            raw,
            issue_number: Some(issue.number),
        });
    }
    Ok(prds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn write_backlog(dir: &Path, files: &[(&str, &str)]) {
        let backlog = dir.join("backlog.d");
        std::fs::create_dir_all(&backlog).unwrap();
        for (name, body) in files {
            std::fs::write(backlog.join(name), body).unwrap();
        }
    }

    #[test]
    fn scans_in_filename_order_with_hashes() {
        let dir = tempfile::tempdir().unwrap();
        write_backlog(
            dir.path(),
            &[
                ("002-second.md", "# Second Spec\nbody"),
                ("001-first.md", "# First Spec\nbody"),
            ],
        );
        let prds = scan(dir.path(), "markdown", "backlog.d", 10).unwrap();
        assert_eq!(prds.len(), 2);
        assert_eq!(prds[0].title, "First Spec");
        assert_eq!(prds[0].priority, 0);
        assert_eq!(prds[1].title, "Second Spec");
        assert_eq!(prds[0].sha256.len(), 64);
        assert_eq!(prds[0].rel_path, "backlog.d/001-first.md");
        assert_ne!(prds[0].sha256, prds[1].sha256);
    }

    #[test]
    fn caps_to_max_items() {
        let dir = tempfile::tempdir().unwrap();
        let bodies: Vec<(String, String)> = (0..15)
            .map(|i| (format!("{i:03}-spec.md"), format!("# Spec {i}")))
            .collect();
        let refs: Vec<(&str, &str)> = bodies
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        write_backlog(dir.path(), &refs);
        let prds = scan(dir.path(), "markdown", "backlog.d", 10).unwrap();
        assert_eq!(prds.len(), 10);
        assert_eq!(prds[9].title, "Spec 9");
    }

    #[test]
    fn falls_back_to_filename_title_and_skips_underscored() {
        let dir = tempfile::tempdir().unwrap();
        write_backlog(
            dir.path(),
            &[
                ("003-no-heading.md", "just prose, no heading"),
                ("_done.md", "# Archived"),
            ],
        );
        let prds = scan(dir.path(), "markdown", "backlog.d", 10).unwrap();
        assert_eq!(prds.len(), 1);
        assert_eq!(prds[0].title, "003 no heading");
    }

    #[test]
    fn missing_backlog_dir_is_empty_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let prds = scan(dir.path(), "markdown", "backlog.d", 10).unwrap();
        assert!(prds.is_empty());
    }

    fn stub_gh(dir: &Path, fixture: &str) -> PathBuf {
        let stub = dir.join("gh");
        let script = format!(
            "#!/bin/sh\n\
             cat <<'EOF'\n\
             {}\n\
             EOF\n",
            fixture
        );
        std::fs::write(&stub, script).unwrap();
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&stub).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub, perms).unwrap();
        }
        stub
    }

    #[test]
    fn github_issues_sort_by_number_ascending_and_cap_max_items() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = r#"[
            {"number": 3, "title": "Third", "body": "body 3", "labels": []},
            {"number": 1, "title": "First", "body": "body 1", "labels": []},
            {"number": 2, "title": "Second", "body": "body 2", "labels": []}
        ]"#;
        let stub = stub_gh(dir.path(), fixture);
        let prds = scan_github_issues_with_bin(dir.path(), 2, stub.as_os_str()).unwrap();
        assert_eq!(prds.len(), 2);
        assert_eq!(prds[0].issue_number, Some(1));
        assert_eq!(prds[0].priority, 0);
        assert_eq!(prds[1].issue_number, Some(2));
        assert_eq!(prds[1].priority, 1);
        assert_eq!(prds[0].rel_path, "github-issues/1.md");
        assert_eq!(prds[0].title, "First");
    }

    #[test]
    fn github_issues_content_hash_is_stable_and_changes_with_body() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = r#"[{"number":7,"title":"T","body":"version a","labels":[]}]"#;
        let stub = stub_gh(dir.path(), fixture);
        let a = scan_github_issues_with_bin(dir.path(), 10, stub.as_os_str()).unwrap();
        let fixture2 = r#"[{"number":7,"title":"T","body":"version b","labels":[]}]"#;
        let stub2 = stub_gh(dir.path(), fixture2);
        let b = scan_github_issues_with_bin(dir.path(), 10, stub2.as_os_str()).unwrap();
        assert_eq!(a[0].issue_number, Some(7));
        assert_ne!(a[0].sha256, b[0].sha256, "edited issue must change hash");
        assert_eq!(a[0].sha256, a[0].id);
    }

    #[test]
    fn github_issues_empty_list_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let stub = stub_gh(dir.path(), "[]");
        let prds = scan_github_issues_with_bin(dir.path(), 10, stub.as_os_str()).unwrap();
        assert!(prds.is_empty());
    }
    #[test]
    fn github_issues_limit_is_clamped_to_a_value_gh_accepts() {
        // The feed's scan_all path passes usize::MAX; `gh --limit` parses an
        // int64 and rejects larger values, so the adapter must clamp.
        let dir = tempfile::tempdir().unwrap();
        let args_file = dir.path().join("gh-args.txt");
        let stub = dir.path().join("gh");
        let script = format!(
            "#!/bin/sh\nprintf '%s ' \"$@\" > {}\necho '[]'\n",
            args_file.display()
        );
        std::fs::write(&stub, script).unwrap();
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&stub).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub, perms).unwrap();
        }
        let prds = scan_github_issues_with_bin(dir.path(), usize::MAX, stub.as_os_str()).unwrap();
        assert!(prds.is_empty());
        let args = std::fs::read_to_string(&args_file).unwrap();
        let limit: u64 = args
            .split_whitespace()
            .skip_while(|a| *a != "--limit")
            .nth(1)
            .expect("gh invocation must pass --limit")
            .parse()
            .expect("--limit must be an integer");
        assert!(limit <= 1000, "--limit must be clamped for gh, got {limit}");
    }

    #[test]
    fn github_issues_gh_failure_is_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let stub = dir.path().join("gh");
        std::fs::write(&stub, "#!/bin/sh\necho 'boom' >&2\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&stub).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub, perms).unwrap();
        }
        let err = scan_github_issues_with_bin(dir.path(), 10, stub.as_os_str()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("gh issue list"), "{msg}");
        assert!(msg.contains("boom"), "{msg}");
    }

    #[test]
    fn github_issues_include_labels_in_footer() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = r#"[
            {"number": 5, "title": "T", "body": null, "labels": [
                {"name": "enhancement"},
                {"name": "bug"}
            ]}
        ]"#;
        let stub = stub_gh(dir.path(), fixture);
        let prds = scan_github_issues_with_bin(dir.path(), 10, stub.as_os_str()).unwrap();
        assert!(prds[0].raw.contains("Source: GitHub issue #5"));
        assert!(prds[0].raw.contains("Labels: bug, enhancement"));
    }
}
