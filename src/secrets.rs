use std::path::Path;

/// Resolve a secret from the environment first, then from a `~/.secrets`-style
/// file of `KEY=value` / `export KEY=value` lines. Never logs values.
pub fn get(names: &[&str]) -> Option<String> {
    for name in names {
        if let Ok(value) = std::env::var(name) {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    let home = std::env::var("HOME").ok()?;
    from_file(Path::new(&home).join(".secrets").as_path(), names)
}

pub fn from_file(path: &Path, names: &[&str]) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some((key, value)) = line.split_once('=') {
            if names.contains(&key.trim()) {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_export_and_quoted_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".secrets");
        std::fs::write(
            &path,
            "# comment\nexport FAL_API_KEY=\"abc123\"\nOTHER='zzz'\n",
        )
        .unwrap();
        assert_eq!(
            from_file(&path, &["FAL_API_KEY", "FAL_KEY"]).as_deref(),
            Some("abc123")
        );
        assert_eq!(from_file(&path, &["OTHER"]).as_deref(), Some("zzz"));
        assert!(from_file(&path, &["MISSING"]).is_none());
    }
}
