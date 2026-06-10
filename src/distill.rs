use serde::{Deserialize, Serialize};

use crate::backlog::PrdSource;
use crate::util::sha256_hex;

/// Structured extraction of a markdown spec, used to script the video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBrief {
    pub prd_id: String,
    pub goal: String,
    pub user: String,
    pub problem: String,
    pub acceptance_criteria: Vec<String>,
    pub risk_notes: Vec<String>,
    pub ambiguity_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beat {
    pub label: String,
    pub spec_payload: String,
    pub visual_prompt: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storyboard {
    pub id: String,
    pub prd_id: String,
    pub prd_sha256: String,
    pub brief_hash: String,
    pub tone: String,
    pub target_duration_sec: u32,
    pub aspect_ratio: String,
    pub beats: Vec<Beat>,
    pub provider_prompt: String,
}

/// Return the body of a `## Heading` section, if present.
fn section<'a>(raw: &'a str, heading: &str) -> Option<String> {
    let mut collecting = false;
    let mut body: Vec<&'a str> = Vec::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if collecting {
                break;
            }
            collecting = rest.trim().eq_ignore_ascii_case(heading);
            continue;
        }
        if collecting {
            body.push(line);
        }
    }
    let text = body.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn bullets(text: Option<String>) -> Vec<String> {
    text.map(|t| {
        t.lines()
            .filter_map(|line| {
                let line = line.trim();
                line.strip_prefix("- ")
                    .or_else(|| line.strip_prefix("* "))
                    .map(|s| s.trim().to_string())
            })
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

pub fn distill(prd: &PrdSource) -> SpecBrief {
    let goal = section(&prd.raw, "Goal").unwrap_or_else(|| prd.title.clone());
    let user = section(&prd.raw, "User").unwrap_or_else(|| "Local operator".into());
    let problem = section(&prd.raw, "Problem").unwrap_or_default();
    let acceptance_criteria = bullets(section(&prd.raw, "Acceptance Criteria"));
    let risk_notes = bullets(section(&prd.raw, "Risk"))
        .into_iter()
        .chain(
            section(&prd.raw, "Risk")
                .filter(|t| !t.starts_with('-') && !t.starts_with('*'))
                .map(|t| t.lines().next().unwrap_or_default().trim().to_string())
                .filter(|t| !t.is_empty()),
        )
        .collect::<Vec<_>>();
    // Dedup while preserving order (plain-text risk section yields one entry).
    let mut seen = std::collections::HashSet::new();
    let risk_notes: Vec<String> = risk_notes
        .into_iter()
        .filter(|r| seen.insert(r.clone()))
        .collect();

    let ambiguity_flags = if acceptance_criteria.is_empty() {
        vec!["No acceptance criteria found.".to_string()]
    } else {
        Vec::new()
    };

    SpecBrief {
        prd_id: prd.id.clone(),
        goal,
        user,
        problem,
        acceptance_criteria,
        risk_notes,
        ambiguity_flags,
    }
}

pub fn compile_storyboard(
    prd: &PrdSource,
    brief: &SpecBrief,
    target_duration_sec: u32,
) -> Storyboard {
    let first_criterion = brief
        .acceptance_criteria
        .first()
        .cloned()
        .unwrap_or_else(|| "Acceptance criteria are missing.".into());
    let first_risk = brief
        .risk_notes
        .first()
        .cloned()
        .unwrap_or_else(|| "No explicit risk recorded.".into());

    let beats = vec![
        Beat {
            label: "Hook".into(),
            spec_payload: brief.goal.clone(),
            visual_prompt: format!(
                "A chaotic vertical short where a software spec bursts into frame like cursed breaking news: {}",
                brief.goal
            ),
            caption: format!("{} just entered the chat", prd.title),
        },
        Beat {
            label: "Stake".into(),
            spec_payload: brief.user.clone(),
            visual_prompt: "An operator cockpit floods with markdown files, neon warning stickers, and absurd meme captions.".into(),
            caption: format!("User: {}", brief.user),
        },
        Beat {
            label: "Payload".into(),
            spec_payload: first_criterion.clone(),
            visual_prompt: "Fast jump cuts show the first acceptance criterion as giant karaoke subtitles over glitchy terminal chaos.".into(),
            caption: first_criterion,
        },
        Beat {
            label: "Risk Check".into(),
            spec_payload: first_risk.clone(),
            visual_prompt: "The clip becomes a fake safety PSA about vague specs driving agent work without receipts.".into(),
            caption: first_risk,
        },
        Beat {
            label: "Decision".into(),
            spec_payload: "Swipe right to dispatch an implementation agent, left to send it back for shaping, up to skip.".into(),
            visual_prompt: "End card with exaggerated swipe arrows, all pointing back to the source spec hash.".into(),
            caption: "Swipe like you mean it".into(),
        },
    ];

    let acceptance_line = if brief.acceptance_criteria.is_empty() {
        "No acceptance criteria found".to_string()
    } else {
        brief.acceptance_criteria.join(" | ")
    };
    let provider_prompt = [
        format!("Generate a vertical 9:16 shortform video around {target_duration_sec} seconds with native audio and an energetic voiceover."),
        "Tone: goofy, anti-corporate, meme culture, high energy. The voiceover must accurately narrate the spec content below — translate it into brainrot style, do not change its meaning.".into(),
        format!("Title: {}", prd.title),
        format!("Audience/User: {}", brief.user),
        format!("Goal: {}", brief.goal),
        format!("Acceptance criteria: {acceptance_line}"),
        format!("Risk: {}", brief.risk_notes.first().cloned().unwrap_or_else(|| "unlisted".into())),
        "Do not invent shipped features, metrics, customer names, or implementation details. Do not claim anything has shipped or that tests pass.".into(),
    ]
    .join("\n");

    let brief_hash = sha256_hex(serde_json::to_string(brief).unwrap_or_default().as_bytes());
    Storyboard {
        id: sha256_hex(format!("{}:{}:brainrot_v1", prd.sha256, brief_hash).as_bytes()),
        prd_id: prd.id.clone(),
        prd_sha256: prd.sha256.clone(),
        brief_hash,
        tone: "brainrot_v1".into(),
        target_duration_sec,
        aspect_ratio: "9:16".into(),
        beats,
        provider_prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn prd(raw: &str) -> PrdSource {
        PrdSource {
            id: sha256_hex(raw.as_bytes()),
            sha256: sha256_hex(raw.as_bytes()),
            rel_path: "backlog.d/test.md".into(),
            abs_path: PathBuf::from("backlog.d/test.md"),
            title: "Test Spec".into(),
            priority: 0,
            raw: raw.into(),
        }
    }

    const SAMPLE: &str = "# Test Spec\n\n## User\nOperators reviewing changes.\n\n## Problem\nStale previews.\n\n## Goal\nAlways show the latest provenance.\n\n## Acceptance Criteria\n- Refresh shows newest render.\n- Old JSON preserved.\n\n## Risk\nCould hide a provider failure.\n";

    #[test]
    fn distill_extracts_sections() {
        let brief = distill(&prd(SAMPLE));
        assert_eq!(brief.user, "Operators reviewing changes.");
        assert_eq!(brief.goal, "Always show the latest provenance.");
        assert_eq!(brief.acceptance_criteria.len(), 2);
        assert_eq!(brief.risk_notes, vec!["Could hide a provider failure."]);
        assert!(brief.ambiguity_flags.is_empty());
    }

    #[test]
    fn distill_flags_missing_acceptance_criteria() {
        let brief = distill(&prd("# Vague\n\n## Goal\nDo something.\n"));
        assert_eq!(brief.ambiguity_flags, vec!["No acceptance criteria found."]);
        assert_eq!(brief.user, "Local operator");
    }

    #[test]
    fn storyboard_narrates_spec_accurately() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        let sb = compile_storyboard(&p, &brief, 8);
        assert_eq!(sb.beats.len(), 5);
        assert_eq!(sb.aspect_ratio, "9:16");
        assert!(sb.provider_prompt.contains("Test Spec"));
        assert!(sb
            .provider_prompt
            .contains("Always show the latest provenance."));
        assert!(sb.provider_prompt.contains("Refresh shows newest render."));
        assert!(sb
            .provider_prompt
            .contains("Do not invent shipped features"));
        assert_eq!(sb.prd_sha256, p.sha256);
    }

    #[test]
    fn storyboard_id_is_deterministic_for_same_spec() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        assert_eq!(
            compile_storyboard(&p, &brief, 8).id,
            compile_storyboard(&p, &brief, 8).id
        );
    }
}
