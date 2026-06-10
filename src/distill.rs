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
            BrainrotFormat::FruitDrama => "fruit_drama_v2",
            BrainrotFormat::GenZExplainer => "genz_explainer_v2",
            BrainrotFormat::CryptidVlog => "cryptid_vlog_v2",
            BrainrotFormat::ItalianBrainrot => "italian_brainrot_v2",
            BrainrotFormat::StreetInterview => "street_interview_v2",
        }
    }
}

/// Rotate formats by feed position so consecutive videos never share a
/// format — variety across the feed beats per-content stability here.
pub fn format_for(priority: usize) -> BrainrotFormat {
    BrainrotFormat::ALL[priority % BrainrotFormat::ALL.len()]
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Keep the first `max_words` words, dropping any trailing period.
pub fn clip_words(text: &str, max_words: usize) -> String {
    text.trim()
        .trim_end_matches('.')
        .split_whitespace()
        .take(max_words.max(1))
        .collect::<Vec<_>>()
        .join(" ")
}

/// How many spoken words fit in a clip: an energetic AI voiceover lands
/// around 2.4 words/sec, and we reserve a 1.5s ending beat so the dialogue
/// never cuts mid-sentence.
pub fn word_budget(duration_sec: u32) -> usize {
    (((duration_sec as f64) - 1.5) * 2.4).max(6.0) as usize
}

/// Words a compressed line must never end on — a clipped line that trails
/// off with "so" or "until" reads like a cutoff, which is the exact thing
/// we're eliminating.
const DANGLERS: &[&str] = &[
    "so", "and", "or", "but", "the", "a", "an", "to", "of", "for", "on", "in", "with", "by",
    "that", "which", "until", "when", "while", "as", "is", "are", "its", "their", "it",
];

/// Compress spec prose into a coherent spoken line: keep the head clause
/// (cut subordinate "so/because/..." tails), clip to the word budget, then
/// drop trailing connectives so the line ends on a real word.
pub fn tighten(text: &str, max_words: usize) -> String {
    let text = text.trim();
    let head = [" so ", " because ", " such that ", " in order to "]
        .iter()
        .filter_map(|d| text.find(d).map(|i| &text[..i]))
        .min_by_key(|s| s.len())
        .unwrap_or(text);
    let mut words: Vec<&str> = head
        .trim_end_matches('.')
        .split_whitespace()
        .take(max_words.max(1))
        .collect();
    while words.len() > 3 {
        let last = words
            .last()
            .unwrap()
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if DANGLERS.contains(&last.as_str()) {
            words.pop();
        } else {
            break;
        }
    }
    words
        .join(" ")
        .trim_end_matches([',', ';', ':'])
        .to_string()
}

/// The complete spoken script for one clip, guaranteed to fit the word
/// budget. Line 1 is the title hook, line 2 the goal, line 3 the first
/// acceptance criterion — included only when the budget allows it whole.
pub struct SpokenScript {
    pub hook: String,
    pub goal: String,
    pub criterion: Option<String>,
}

impl SpokenScript {
    pub fn word_count(&self) -> usize {
        words(&self.hook) + words(&self.goal) + self.criterion.as_deref().map_or(0, words)
    }

    fn full_text(&self) -> String {
        match &self.criterion {
            Some(c) => format!("{} {} {}", self.hook, self.goal, c),
            None => format!("{} {}", self.hook, self.goal),
        }
    }
}

pub fn plan_script(title: &str, goal: &str, criterion: &str, duration_sec: u32) -> SpokenScript {
    let budget = word_budget(duration_sec);
    // The hook never eats the whole budget: the goal always gets ≥2 words.
    let hook = format!(
        "{}.",
        clip_words(title, budget.saturating_sub(2).clamp(1, 6))
    );
    let mut used = words(&hook);
    let goal_line = format!(
        "{}.",
        tighten(goal, budget.saturating_sub(used).clamp(1, 11))
    );
    used += words(&goal_line);
    let remaining = budget.saturating_sub(used);
    // "Not done until" costs 3 words; only speak the criterion if at least
    // a few words of it fit — a truncated stump is worse than silence.
    let criterion =
        (remaining >= 6).then(|| format!("Not done until {}.", tighten(criterion, remaining - 3)));
    SpokenScript {
        hook,
        goal: goal_line,
        criterion,
    }
}

/// Build the video-model prompt for one format. The dialogue quotes the
/// actual spec text — the video must communicate the spec, not just vibe —
/// and the whole script must finish before the clip ends.
fn format_prompt(format: BrainrotFormat, script: &SpokenScript, duration_sec: u32) -> String {
    let header = format!(
        "Vertical 9:16 video, exactly {duration_sec} seconds, with native audio: \
         clear spoken dialogue, sound effects, and music as described."
    );
    let hook = &script.hook;
    let goal = &script.goal;
    let scene = match format {
        BrainrotFormat::FruitDrama => {
            let mango = match &script.criterion {
                Some(c) => format!(" The mango looks away and whispers: \"{c}\""),
                None => " The mango looks away in shame, silent.".to_string(),
            };
            format!(
                "AI fruit drama soap opera. A sunlit kitchen counter shot like a telenovela: \
                 shallow depth of field, dramatic golden lighting, slow push-in. Two anthropomorphic \
                 fruits with big expressive eyes and mouths — a furious strawberry and a guilty mango. \
                 The strawberry gasps, voice trembling with betrayal: \"{hook} {goal}\" \
                 Dramatic zoom.{mango} Thunder crack, telenovela string sting. Played completely straight."
            )
        }
        BrainrotFormat::GenZExplainer => {
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\" Vine boom on the last line."),
                None => " Vine boom at the end.".to_string(),
            };
            format!(
                "Unhinged gen-z talking-head explainer. A twentysomething with a ring light films a \
                 vertical selfie video in their bedroom, talking fast straight into the camera with \
                 punch-in zooms and big bold captions appearing word by word. \
                 They say, deadly serious: \"{hook} {goal}\"{tail}"
            )
        }
        BrainrotFormat::CryptidVlog => {
            let tail = match &script.criterion {
                Some(c) => format!(" Then, deadpan to camera: \"{c}\""),
                None => String::new(),
            };
            format!(
                "Found-footage cryptid vlog. Bigfoot holds a GoPro at arm's length while striding \
                 through a sunny pine forest, fur rustling, lens slightly fisheye, very influencer. \
                 In a chill deep voice he says: \"{hook} {goal}\"{tail} Birdsong, crunching \
                 footsteps, one dramatic zoom to his face at the end."
            )
        }
        BrainrotFormat::ItalianBrainrot => {
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\""),
                None => String::new(),
            };
            format!(
                "Italian brainrot creature reveal. A surreal hybrid creature — a giant espresso cup \
                 with muscular human legs and a crocodile head — strikes heroic poses on a marble \
                 plaza, camera orbiting, renaissance lighting, fully cinematic. A bombastic \
                 pseudo-Italian opera narrator bellows over orchestral hits: \"{hook} {goal}\"{tail} \
                 Deadpan, epic, absurd."
            )
        }
        BrainrotFormat::StreetInterview => {
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\""),
                None => String::new(),
            };
            format!(
                "Fake documentary street interview from the year 2080. A reporter holds a microphone \
                 to an elderly retired gen-z developer on a city sidewalk, vertical handheld framing, \
                 ambient traffic, mournful piano. The retiree stares into the distance, voice \
                 cracking: \"{hook} {goal}\"{tail} Slow documentary push-in on their eyes."
            )
        }
    };
    let pacing = format!(
        "The complete spoken script is exactly the quoted text above — {} words total. \
         Every word must be spoken at a natural energetic pace and FINISHED by second {} of the \
         video, ending on a held reaction shot or visual beat. Never cut off mid-sentence; \
         the clip must feel like one complete unit. Full script for reference: \"{}\"",
        script.word_count(),
        duration_sec.saturating_sub(1),
        script.full_text(),
    );
    let guardrail = "All spoken lines must stay faithful to the quoted script. Do not invent \
                     shipped features, metrics, customer names, or implementation details. \
                     Do not claim anything has shipped or that tests pass. \
                     On-screen captions, if any, must match the dialogue.";
    format!("{header}\n{scene}\n{pacing}\n{guardrail}")
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

    let script = plan_script(
        &prd.title,
        &brief.goal,
        &first_criterion,
        target_duration_sec,
    );
    let provider_prompt = format_prompt(format, &script, target_duration_sec);

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
    fn clip_words_keeps_whole_words() {
        assert_eq!(clip_words("Ship the thing.", 10), "Ship the thing");
        assert_eq!(clip_words("one two three four", 2), "one two");
        assert_eq!(clip_words("solo", 0), "solo"); // never empty
    }

    #[test]
    fn tighten_cuts_at_clause_boundaries_and_danglers() {
        // Subordinate clause dropped whole, not mid-truncated.
        assert_eq!(
            tighten(
                "Add a cache busting path for generated render metadata so the gallery always shows the latest MP4 provenance.",
                11
            ),
            "Add a cache busting path for generated render metadata"
        );
        assert_eq!(
            tighten("Record a simple human vibe rating on each render so bad models can be culled later.", 11),
            "Record a simple human vibe rating on each render"
        );
        // Never ends on a dangling connective.
        assert_eq!(
            tighten("block run-intent gestures completely and", 10),
            "block run-intent gestures completely"
        );
        // Short coherent lines pass through.
        assert_eq!(tighten("Ship it.", 11), "Ship it");
    }

    #[test]
    fn script_always_fits_the_word_budget() {
        let long_goal = "make the gallery always show the latest MP4 provenance for every \
                         spec in the backlog even when providers drift and budgets explode"
            .to_string();
        let long_criterion = "a route level test proves that the newest successful render is \
                              selected by provenance timestamp and never an older stale render";
        for duration in [4u32, 6, 8, 12] {
            let script = plan_script("Cache Chaos Exorcism", &long_goal, long_criterion, duration);
            assert!(
                script.word_count() <= word_budget(duration),
                "{} words exceeds budget {} at {duration}s",
                script.word_count(),
                word_budget(duration)
            );
        }
    }

    #[test]
    fn criterion_is_whole_or_absent() {
        // Tight budget: criterion dropped entirely, not stumped.
        let tight = plan_script(
            "A Very Long Spec Title Here",
            "a goal sentence that uses up most of the available word budget for sure",
            "this criterion will not fit",
            4,
        );
        assert!(tight.criterion.is_none());
        assert!(tight.word_count() <= word_budget(4));

        // Roomy budget: criterion spoken.
        let roomy = plan_script("Spec", "Ship it.", "tests pass on refresh", 8);
        assert!(roomy.criterion.is_some());
        assert!(roomy
            .criterion
            .as_deref()
            .unwrap()
            .contains("Not done until"));
    }

    #[test]
    fn prompt_demands_complete_script_before_clip_ends() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        let sb = compile_storyboard(&p, &brief, 8);
        assert!(sb.provider_prompt.contains("FINISHED by second 7"));
        assert!(sb.provider_prompt.contains("Never cut off mid-sentence"));
        assert!(sb.provider_prompt.contains("words total"));
    }
}
