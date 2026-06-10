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

/// Live brainrot formats (researched 2026-06): AI fruit soap operas,
/// Italian-brainrot creature reveals, cryptid selfie vlogs, fake future
/// documentaries, and unhinged gen-z explainers. Each spec is assigned one
/// deterministically so the feed has variety and re-renders are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainrotFormat {
    FruitDrama,
    GenZExplainer,
    CryptidVlog,
    ItalianBrainrot,
    StreetInterview,
}

impl BrainrotFormat {
    pub const ALL: [BrainrotFormat; 5] = [
        BrainrotFormat::FruitDrama,
        BrainrotFormat::GenZExplainer,
        BrainrotFormat::CryptidVlog,
        BrainrotFormat::ItalianBrainrot,
        BrainrotFormat::StreetInterview,
    ];

    pub fn tone(self) -> &'static str {
        match self {
            BrainrotFormat::FruitDrama => "fruit_drama_v1",
            BrainrotFormat::GenZExplainer => "genz_explainer_v1",
            BrainrotFormat::CryptidVlog => "cryptid_vlog_v1",
            BrainrotFormat::ItalianBrainrot => "italian_brainrot_v1",
            BrainrotFormat::StreetInterview => "street_interview_v1",
        }
    }
}

/// Rotate formats by feed position so consecutive videos never share a
/// format — variety across the feed beats per-content stability here.
pub fn format_for(priority: usize) -> BrainrotFormat {
    BrainrotFormat::ALL[priority % BrainrotFormat::ALL.len()]
}

/// Word-boundary clip so spec text fits in ~8 seconds of spoken dialogue.
fn clip(text: &str, max_chars: usize) -> String {
    let text = text.trim().trim_end_matches('.');
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        if out.len() + word.len() + 1 > max_chars {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        text.chars().take(max_chars).collect()
    } else {
        out
    }
}

/// Build the video-model prompt for one format. The dialogue quotes the
/// actual spec text — the video must communicate the spec, not just vibe.
fn format_prompt(
    format: BrainrotFormat,
    title: &str,
    goal: &str,
    criterion: &str,
    duration_sec: u32,
) -> String {
    let header = format!(
        "Vertical 9:16 video, exactly {duration_sec} seconds, with native audio: \
         clear spoken dialogue, sound effects, and music as described."
    );
    let scene = match format {
        BrainrotFormat::FruitDrama => format!(
            "AI fruit drama soap opera. A sunlit kitchen counter shot like a telenovela: \
             shallow depth of field, dramatic golden lighting, slow push-in. Two anthropomorphic \
             fruits with big expressive eyes and mouths — a furious strawberry and a guilty mango. \
             The strawberry gasps, voice trembling with betrayal: \"{title}. {goal}.\" \
             Dramatic zoom. The mango looks away and whispers: \"And it is not over until {criterion}.\" \
             Thunder crack, telenovela string sting, audible gasp. Played completely straight."
        ),
        BrainrotFormat::GenZExplainer => format!(
            "Unhinged gen-z talking-head explainer. A twentysomething with a ring light films a \
             vertical selfie video in their bedroom, talking fast straight into the camera with \
             punch-in zooms on every sentence and big bold captions appearing word by word. \
             They say, deadly serious: \"POV: the backlog just dropped {title}. {goal}. \
             And we are NOT done until {criterion}. No cap.\" Vine boom sound effect on the last line."
        ),
        BrainrotFormat::CryptidVlog => format!(
            "Found-footage cryptid vlog. Bigfoot holds a GoPro at arm's length while striding \
             through a sunny pine forest, fur rustling, lens slightly fisheye, very influencer. \
             In a chill deep voice he says: \"Yo what's good fam, today we are shipping {title}. \
             {goal}. We do NOT log off until {criterion}.\" Birdsong, crunching footsteps, \
             one dramatic zoom to his face at the end."
        ),
        BrainrotFormat::ItalianBrainrot => format!(
            "Italian brainrot creature reveal. A surreal hybrid creature — a giant espresso cup \
             with muscular human legs and a crocodile head — strikes heroic poses on a marble \
             plaza, camera orbiting, renaissance lighting, fully cinematic. A bombastic \
             pseudo-Italian opera narrator bellows over orchestral hits: \
             \"Specifissimo {title}! {goal}! Non è finito until {criterion}!\" Deadpan, epic, absurd."
        ),
        BrainrotFormat::StreetInterview => format!(
            "Fake documentary street interview from the year 2080. A reporter holds a microphone \
             to an elderly retired gen-z developer on a city sidewalk, vertical handheld framing, \
             ambient traffic, mournful piano. The reporter asks: \"Do you remember {title}?\" \
             The retiree stares into the distance, voice cracking: \"{goal}. We swore we were not \
             done until {criterion}.\" Slow documentary push-in on their eyes."
        ),
    };
    let guardrail = "All spoken lines must stay faithful to the quoted spec text. Do not invent \
                     shipped features, metrics, customer names, or implementation details. \
                     Do not claim anything has shipped or that tests pass. \
                     On-screen captions, if any, must match the dialogue.";
    format!("{header}\n{scene}\n{guardrail}")
}

pub fn compile_storyboard(
    prd: &PrdSource,
    brief: &SpecBrief,
    target_duration_sec: u32,
) -> Storyboard {
    compile_with_format(prd, brief, target_duration_sec, format_for(prd.priority))
}

pub fn compile_with_format(
    prd: &PrdSource,
    brief: &SpecBrief,
    target_duration_sec: u32,
    format: BrainrotFormat,
) -> Storyboard {
    let first_criterion = brief
        .acceptance_criteria
        .first()
        .cloned()
        .unwrap_or_else(|| "someone writes actual acceptance criteria".into());
    let first_risk = brief
        .risk_notes
        .first()
        .cloned()
        .unwrap_or_else(|| "No explicit risk recorded.".into());

    let goal_line = clip(&brief.goal, 110);
    let criterion_line = clip(&first_criterion, 90);
    let provider_prompt = format_prompt(
        format,
        &prd.title,
        &goal_line,
        &criterion_line,
        target_duration_sec,
    );

    let beats = vec![
        Beat {
            label: "Cold Open".into(),
            spec_payload: brief.goal.clone(),
            visual_prompt: format!("{:?} cold open for: {}", format, brief.goal),
            caption: format!("{} just entered the chat", prd.title),
        },
        Beat {
            label: "Stake".into(),
            spec_payload: brief.user.clone(),
            visual_prompt: "Who this is for, delivered in-character.".into(),
            caption: format!("User: {}", brief.user),
        },
        Beat {
            label: "Payload".into(),
            spec_payload: first_criterion.clone(),
            visual_prompt: "The first acceptance criterion as the emotional climax.".into(),
            caption: first_criterion,
        },
        Beat {
            label: "Risk Check".into(),
            spec_payload: first_risk.clone(),
            visual_prompt: "The risk note as a fake safety PSA.".into(),
            caption: first_risk,
        },
        Beat {
            label: "Decision".into(),
            spec_payload: "Swipe right to dispatch an implementation agent, left to send it back for shaping, up to skip.".into(),
            visual_prompt: "End card with exaggerated swipe arrows, all pointing back to the source spec hash.".into(),
            caption: "Swipe like you mean it".into(),
        },
    ];

    let brief_hash = sha256_hex(serde_json::to_string(brief).unwrap_or_default().as_bytes());
    Storyboard {
        id: sha256_hex(format!("{}:{}:{}", prd.sha256, brief_hash, format.tone()).as_bytes()),
        prd_id: prd.id.clone(),
        prd_sha256: prd.sha256.clone(),
        brief_hash,
        tone: format.tone().into(),
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
        // The dialogue carries the actual spec content.
        assert!(sb.provider_prompt.contains("Test Spec"));
        assert!(sb
            .provider_prompt
            .contains("Always show the latest provenance"));
        assert!(sb.provider_prompt.contains("Refresh shows newest render"));
        assert!(sb.provider_prompt.contains("Do not invent"));
        assert!(sb.provider_prompt.contains("9:16"));
        assert!(sb.provider_prompt.contains("native audio"));
        assert_eq!(sb.prd_sha256, p.sha256);
        assert_eq!(sb.tone, format_for(p.priority).tone());
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

    #[test]
    fn consecutive_feed_positions_never_share_a_format() {
        assert_eq!(format_for(0), BrainrotFormat::FruitDrama);
        assert_eq!(format_for(1), BrainrotFormat::GenZExplainer);
        assert_eq!(format_for(2), BrainrotFormat::CryptidVlog);
        assert_eq!(format_for(3), BrainrotFormat::ItalianBrainrot);
        assert_eq!(format_for(4), BrainrotFormat::StreetInterview);
        assert_eq!(format_for(5), BrainrotFormat::FruitDrama);
        for i in 0..10 {
            assert_ne!(format_for(i), format_for(i + 1));
        }
    }

    #[test]
    fn every_format_quotes_the_spec_as_dialogue() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        for format in BrainrotFormat::ALL {
            let sb = compile_with_format(&p, &brief, 8, format);
            assert_eq!(sb.tone, format.tone());
            assert!(
                sb.provider_prompt
                    .contains("Always show the latest provenance"),
                "{format:?} must speak the goal: {}",
                sb.provider_prompt
            );
            assert!(
                sb.provider_prompt.contains("Refresh shows newest render"),
                "{format:?} must speak the first acceptance criterion"
            );
            assert!(sb.provider_prompt.contains("native audio"), "{format:?}");
            assert!(sb.provider_prompt.contains("Do not invent"), "{format:?}");
        }
        // Different formats produce genuinely different scenes.
        let a = compile_with_format(&p, &brief, 8, BrainrotFormat::FruitDrama).provider_prompt;
        let b = compile_with_format(&p, &brief, 8, BrainrotFormat::CryptidVlog).provider_prompt;
        assert_ne!(a, b);
    }

    #[test]
    fn clip_respects_word_boundaries() {
        assert_eq!(clip("Ship the thing.", 110), "Ship the thing");
        let long = "word ".repeat(50);
        let clipped = clip(&long, 40);
        assert!(clipped.len() <= 40);
        assert!(!clipped.ends_with(' '));
        assert!(clipped.starts_with("word"));
    }
}
