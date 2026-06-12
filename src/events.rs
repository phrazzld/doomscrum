use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::util::{now_rfc3339, sha256_hex};

/// Durable local decision/lifecycle event, appended to `events.ndjson`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub prd_id: String,
    pub prd_sha256: String,
    /// "rendered" | "skip" | "dispatch_implement" | "dispatch_shape" | "vibe_rating"
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    pub created_at: String,
}

pub fn append(
    events_path: &Path,
    prd_id: &str,
    prd_sha256: &str,
    kind: &str,
    note: Option<String>,
) -> Result<Event> {
    let created_at = now_rfc3339();
    let event = Event {
        id: sha256_hex(format!("{prd_id}:{kind}:{created_at}").as_bytes()),
        prd_id: prd_id.into(),
        prd_sha256: prd_sha256.into(),
        kind: kind.into(),
        note,
        render_id: None,
        rating: None,
        created_at,
    };
    append_event(events_path, &event)
}

pub fn append_rating(
    events_path: &Path,
    prd_id: &str,
    prd_sha256: &str,
    render_id: &str,
    rating: &str,
) -> Result<Event> {
    let created_at = now_rfc3339();
    let event = Event {
        id: sha256_hex(
            format!("{prd_id}:vibe_rating:{render_id}:{rating}:{created_at}").as_bytes(),
        ),
        prd_id: prd_id.into(),
        prd_sha256: prd_sha256.into(),
        kind: "vibe_rating".into(),
        note: Some(format!("{render_id}:{rating}")),
        render_id: Some(render_id.into()),
        rating: Some(rating.into()),
        created_at,
    };
    append_event(events_path, &event)
}

fn append_event(events_path: &Path, event: &Event) -> Result<Event> {
    if let Some(parent) = events_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path)
        .with_context(|| format!("opening {}", events_path.display()))?;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;
    Ok(event.clone())
}

pub fn read_all(events_path: &Path) -> Result<Vec<Event>> {
    match std::fs::read_to_string(events_path) {
        Ok(raw) => Ok(raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/events.ndjson");
        append(&path, "prd1", "hash1", "skip", None).unwrap();
        append(
            &path,
            "prd1",
            "hash1",
            "dispatch_implement",
            Some("dispatched".into()),
        )
        .unwrap();
        let events = read_all(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "skip");
        assert_eq!(events[1].note.as_deref(), Some("dispatched"));
        assert_eq!(events[1].prd_sha256, "hash1");
        assert!(events[1].render_id.is_none());
    }

    #[test]
    fn append_rating_records_render_and_rating() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/events.ndjson");
        append_rating(&path, "prd1", "hash1", "render-1", "cursed").unwrap();

        let events = read_all(&path).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "vibe_rating");
        assert_eq!(events[0].render_id.as_deref(), Some("render-1"));
        assert_eq!(events[0].rating.as_deref(), Some("cursed"));
    }

    #[test]
    fn missing_ledger_reads_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_all(&dir.path().join("none.ndjson"))
            .unwrap()
            .is_empty());
    }
}
