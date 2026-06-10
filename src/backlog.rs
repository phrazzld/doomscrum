use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::util::sha256_hex;

/// One markdown spec from the synced repo's backlog directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdSource {
    /// Content hash; doubles as the stable id for renders and decisions.
    pub id: String,
    pub sha256: String,
    /// Path relative to the synced repo root (e.g. `backlog.d/001-foo.md`).
    pub rel_path: String,
    #[serde(skip)]
    pub abs_path: PathBuf,
    pub title: String,
    /// 0 = highest priority. Priority is filename sort order.
    pub priority: usize,
    pub raw: String,
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

/// Scan the backlog directory, highest priority first, capped at `max_items`.
pub fn scan(repo_root: &Path, backlog_dir: &str, max_items: usize) -> Result<Vec<PrdSource>> {
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
        });
    }
    Ok(prds)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let prds = scan(dir.path(), "backlog.d", 10).unwrap();
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
        let prds = scan(dir.path(), "backlog.d", 10).unwrap();
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
        let prds = scan(dir.path(), "backlog.d", 10).unwrap();
        assert_eq!(prds.len(), 1);
        assert_eq!(prds[0].title, "003 no heading");
    }

    #[test]
    fn missing_backlog_dir_is_empty_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let prds = scan(dir.path(), "backlog.d", 10).unwrap();
        assert!(prds.is_empty());
    }
}
