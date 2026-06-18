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

/// Secret names whose *values* may surface in agent stdout/stderr and must be
/// masked from the persisted log and the `/log` route. These are the operator's
/// service keys (DoomScrum's own + git push tokens) — distinct from the
/// dispatched agent's provider key, which the agent legitimately needs. Grouped
/// so each logical secret is resolved by [`get`] (env, then `~/.secrets`),
/// catching a key that lives only in the file — exactly what an agent's
/// file-read could surface.
const SECRET_GROUPS: &[&[&str]] = &[
    &["FAL_API_KEY", "FAL_KEY"],
    &["OPENROUTER_API_KEY"],
    &["GH_TOKEN", "GITHUB_TOKEN"],
];

/// Token prefixes that reliably mark a credential regardless of source — e.g.
/// `sk-…` (OpenAI/OpenRouter), GitHub `ghp_`/`gho_`/`ghs_`/`ghu_`/`ghr_`, and
/// fine-grained `github_pat_…`. Deliberately omits `fal-`: FAL *model ids*
/// (`fal-ai/veo3.1/…`) are not secrets and appear in logs constantly; the FAL
/// key is masked by value instead (see [`SECRET_GROUPS`]).
const SECRET_PREFIXES: &[&str] = &["sk-", "ghp_", "gho_", "ghs_", "ghu_", "ghr_", "github_pat_"];

const REDACTED: &str = "[REDACTED]";

/// Mask credential-shaped tokens in text bound for a log file or HTTP response.
/// `extra` holds literal secret values to mask by exact match (the operator's
/// resolved keys); kept as a parameter so the core stays pure and testable
/// without touching the environment. Masks, in order: exact `extra` values,
/// then `Bearer <token>` headers and any token carrying a [`SECRET_PREFIXES`]
/// prefix. Non-secret text (model ids, URLs, prose) is left untouched.
pub fn redact(text: &str, extra: &[String]) -> String {
    // 1. Exact known secret values (the operator's resolved keys) — masks any
    //    format, including FAL's `id:secret` shape that has no clean prefix.
    let mut s = text.to_string();
    for v in extra {
        if v.len() >= 6 {
            s = s.replace(v.as_str(), REDACTED);
        }
    }
    // 2. Credential-shaped tokens and `Bearer <token>` headers. Walk token runs
    //    so surrounding prose, model ids, and punctuation are preserved.
    let emit = |out: &mut String, token: &str, prev: &str| {
        out.push_str(if looks_secret(token, prev) {
            REDACTED
        } else {
            token
        });
    };
    let mut out = String::with_capacity(s.len());
    let mut token = String::new();
    let mut prev = String::new();
    for ch in s.chars() {
        // `:` is a token char so a FAL `id:secret` key stays one token (the
        // shape check needs both halves); URLs/`Authorization:` survive because
        // they fail the secret tests below.
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '/' | '=' | '.' | '~' | ':')
        {
            token.push(ch);
            continue;
        }
        if !token.is_empty() {
            emit(&mut out, &token, &prev);
            prev = std::mem::take(&mut token);
        }
        out.push(ch);
    }
    if !token.is_empty() {
        emit(&mut out, &token, &prev);
    }
    out
}

/// A token is credential-shaped if it follows a `Bearer` header, carries a known
/// secret prefix, or matches the FAL `id:secret` key shape. Both checks split
/// compound tokens so a credential embedded after a delimiter is still caught —
/// a git remote URL `https://user:ghp_xxx@host`, an env-dump `KEY=sk-…`, or a
/// glued `FAL_API_KEY=id:secret`. The length floor keeps a bare `sk-` readable.
fn looks_secret(token: &str, prev: &str) -> bool {
    if prev.eq_ignore_ascii_case("bearer") && token.len() >= 8 {
        return true;
    }
    // Prefix creds: split on every delimiter (incl. `:`) so `u:ghp_xxx` exposes
    // the `ghp_…` segment.
    if token
        .split(['/', ':', '@', '='])
        .any(|seg| seg.len() >= 8 && SECRET_PREFIXES.iter().any(|p| seg.starts_with(p)))
    {
        return true;
    }
    // FAL `id:secret` keys *use* `:`, so split only on the other delimiters and
    // shape-check each segment (catches a glued `KEY=id:secret`).
    token.split(['/', '@', '=']).any(looks_like_fal_key)
}

/// FAL keys are `<key_id>:<secret>` — a hex/uuid id and a long hex secret. This
/// masks FAL-shaped tokens by *shape* (rotated keys, old logs, keys pasted into
/// a spec) where exact-value redaction would miss them. Narrow enough to spare
/// timestamps (`12:34:56`) and URLs (`https://…`): both halves must be hex.
fn looks_like_fal_key(token: &str) -> bool {
    let Some((id, secret)) = token.split_once(':') else {
        return false;
    };
    let hexish = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    id.len() >= 8
        && hexish(id)
        && secret.len() >= 16
        && secret.chars().all(|c| c.is_ascii_hexdigit())
}

/// True if `key` names one of DoomScrum's service secrets or a git push token —
/// vars the untrusted agent must never receive, even if an operator mistakenly
/// adds one to `agent.env_allowlist`. The agent's *own* provider keys
/// (`OPENAI_API_KEY`/`ANTHROPIC_API_KEY`) are not service secrets.
pub fn is_service_secret_name(key: &str) -> bool {
    SECRET_GROUPS.iter().any(|group| group.contains(&key))
}

/// The operator's secret values, resolved from the environment and `~/.secrets`
/// (via [`get`]), for exact-match redaction. Cheap; call per log write / route
/// hit.
pub fn known_values() -> Vec<String> {
    SECRET_GROUPS
        .iter()
        .filter_map(|names| get(names))
        .filter(|v| v.len() >= 6)
        .collect()
}

/// True if any *added* line of a unified `git diff` carries a secret-shaped
/// token (or a `known` key value) — so a dispatch can refuse to push a diff that
/// would exfiltrate a credential into a PR. Context/removed lines are ignored: a
/// pre-existing secret in surrounding context is not a new leak.
///
/// Parses hunk structure rather than matching `+` prefixes, so an added line
/// whose *content* begins with `+` (rendered `++…`/`+++…`) is still scanned —
/// a naive `!starts_with("+++")` filter would skip it. Only lines inside a
/// hunk (after `@@`) count; the `+++ b/path` file header sits before any hunk.
/// Feed it `git diff --text` so binary blobs are diffed as text, not hidden
/// behind "Binary files differ".
pub fn diff_adds_secret(diff: &str, known: &[String]) -> bool {
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        // The file-header block (`diff --git`, `--- `, `+++ b/path`, `index`)
        // precedes the first `@@`, so it is never in_hunk — just skip it.
        if !in_hunk {
            continue;
        }
        // Inside a hunk, classify by the marker: a `+` line is added content
        // (even if its content begins with `+`, rendering `++…`/`+++…`). A bare
        // `diff --git` (no marker) is the next file's header, ending the hunk.
        if let Some(added) = line.strip_prefix('+') {
            if redact(added, known) != added {
                return true;
            }
        } else if line.starts_with("diff --git") {
            in_hunk = false;
        }
    }
    false
}

/// Convenience: [`redact`] against the environment-resolved [`known_values`].
pub fn redact_env(text: &str) -> String {
    redact(text, &known_values())
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

    #[test]
    fn redact_masks_key_shaped_tokens_and_leaves_prose() {
        let line = "calling api with sk-or-v1-abcDEF1234567890 and Authorization: Bearer ghp_TOKEN9876543210 done";
        let out = redact(line, &[]);
        assert!(!out.contains("sk-or-v1-abcDEF1234567890"), "{out}");
        assert!(!out.contains("ghp_TOKEN9876543210"), "{out}");
        assert!(out.contains(REDACTED), "{out}");
        // surrounding prose survives
        assert!(out.starts_with("calling api with "), "{out}");
        assert!(out.ends_with(" done"), "{out}");
        assert!(out.contains("Authorization:"), "{out}");
    }

    #[test]
    fn redact_masks_operator_key_by_exact_value() {
        // The falsifier: a spec body that coaxes the agent into echoing the key.
        let fake_fal = "2b8c4d9e:f1a2b3c4d5e6f7a8b9c0d1e2"; // FAL-style id:secret
        let line = format!("the agent printed FAL_API_KEY={fake_fal} oops");
        let out = redact(&line, &[fake_fal.to_string()]);
        assert!(!out.contains(fake_fal), "{out}");
        assert!(out.contains(REDACTED), "{out}");
    }

    #[test]
    fn redact_masks_fal_shaped_id_secret_tokens() {
        // A FAL key shape (id:secret hex) that is NOT the current resolved value
        // must still be masked by shape — rotated keys, old logs, pasted keys.
        let key = "2b8c4d9e1f0a:f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6";
        let line = format!("old log had FAL key {key} in it");
        let out = redact(&line, &[]);
        assert!(!out.contains(key), "{out}");
        assert!(out.contains(REDACTED), "{out}");
        assert!(out.starts_with("old log had FAL key "), "{out}");
        assert!(out.ends_with(" in it"), "{out}");
        // UUID-form id is also masked.
        let uuid = "2b8c4d9e-1155-4219-9131-35e91d9cd609:0718eee656adb2ca36a8b7092da41005";
        assert!(redact(uuid, &[]).contains(REDACTED));
        // Glued env-dump form `KEY=id:secret` is masked by shape, not just by
        // exact value — the `=` must not poison the id half.
        let glued = "FAL_API_KEY=2b8c4d9e1f0a:f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6";
        let out2 = redact(glued, &[]);
        assert!(
            !out2.contains("2b8c4d9e1f0a:f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6"),
            "{out2}"
        );
        assert!(out2.contains(REDACTED), "{out2}");
    }

    #[test]
    fn redact_masks_credentials_embedded_in_compound_tokens() {
        // A git remote URL with an inline credential (the shape a failed
        // `git push` echoes into a receipt note) — the token doesn't *start*
        // with the prefix, so segment splitting must catch it.
        let line = "remote: https://user:ghp_DEADBEEF1234567890@github.com/x.git rejected";
        let out = redact(line, &[]);
        assert!(!out.contains("ghp_DEADBEEF1234567890"), "{out}");
        assert!(out.contains("[REDACTED]"), "{out}");
        assert!(out.ends_with(" rejected"), "{out}");
    }

    #[test]
    fn redact_spares_timestamps_and_urls_despite_colons() {
        // `:` is a token char now, but neither of these is hex:hex shaped.
        let line = "at 12:34:56 fetched https://queue.fal.run/path ok";
        assert_eq!(redact(line, &[]), line);
    }

    #[test]
    fn diff_adds_secret_flags_only_added_credentials() {
        // A credential introduced on an added line is a leak…
        let leak = "\
diff --git a/x b/x
--- a/x
+++ b/x
@@ -0,0 +1,2 @@
+let key = \"sk-or-v1-EXFIL1234567890\";
+fn main() {}";
        assert!(diff_adds_secret(leak, &[]));

        // …but a clean hunk (incl. a fal-ai model id) is fine…
        let clean = "\
diff --git a/m b/m
--- a/m
+++ b/m
@@ -0,0 +1,2 @@
+use fal_ai;
+const MODEL: &str = \"fal-ai/veo3.1/fast\";";
        assert!(!diff_adds_secret(clean, &[]));

        // …and a secret only in CONTEXT or REMOVED lines is not a new leak.
        let not_added = "\
@@ -1,2 +1,2 @@
 context sk-or-v1-OLDCONTEXT1234567890
-gone ghp_REMOVED1234567890
+clean line";
        assert!(!diff_adds_secret(not_added, &[]));

        // Known operator key values are caught by exact match on added lines.
        let by_value = "@@ -0,0 +1 @@\n+echo SENTINEL-FAL-aaaa-bbbb-cccc";
        assert!(diff_adds_secret(
            by_value,
            &["SENTINEL-FAL-aaaa-bbbb-cccc".into()]
        ));
    }

    #[test]
    fn diff_adds_secret_scans_every_commit_in_log_output() {
        // Fed `git log -p` output: a secret ADDED in one commit and REMOVED in a
        // later one must still be flagged — `git push` ships the whole history.
        // Commit headers and indented messages must not derail the hunk parser.
        let log = "\
commit bbbbbbb
Author: a <a@b>

    remove the secret

diff --git a/s.txt b/s.txt
--- a/s.txt
+++ /dev/null
@@ -1 +0,0 @@
-sk-or-v1-HISTEXFIL1234567890

commit aaaaaaa
Author: a <a@b>

    add the secret

diff --git a/s.txt b/s.txt
--- /dev/null
+++ b/s.txt
@@ -0,0 +1 @@
+sk-or-v1-HISTEXFIL1234567890";
        assert!(diff_adds_secret(log, &[]));
    }

    #[test]
    fn diff_adds_secret_catches_plus_prefixed_content() {
        // Bypass guard: an added line whose CONTENT starts with `++` renders as
        // `+++…`. A naive `!starts_with("+++")` filter would skip it; hunk-aware
        // parsing must still scan it (only the pre-hunk `+++ b/path` is exempt).
        let evasion = "\
diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1,2 @@
 ctx
+++ leaked = \"sk-or-v1-EXFIL1234567890\";";
        assert!(diff_adds_secret(evasion, &[]));
    }

    #[test]
    fn service_secret_names_are_recognized() {
        for k in [
            "FAL_API_KEY",
            "FAL_KEY",
            "OPENROUTER_API_KEY",
            "GH_TOKEN",
            "GITHUB_TOKEN",
        ] {
            assert!(is_service_secret_name(k), "{k} should be a service secret");
        }
        // The agent's own provider keys are NOT service secrets (it needs them).
        assert!(!is_service_secret_name("OPENAI_API_KEY"));
        assert!(!is_service_secret_name("ANTHROPIC_API_KEY"));
        assert!(!is_service_secret_name("PATH"));
    }

    #[test]
    fn redact_does_not_touch_fal_model_ids() {
        // FAL *model ids* are not secrets and must stay legible in logs.
        let line = "render via fal-ai/veo3.1/fast at https://queue.fal.run";
        assert_eq!(redact(line, &[]), line);
    }

    #[test]
    fn redact_ignores_short_or_bare_prefixes() {
        // A literal "sk-" in prose, or too-short token, is not a credential.
        let line = "the sk- prefix denotes a secret key";
        assert_eq!(redact(line, &[]), line);
    }
}
