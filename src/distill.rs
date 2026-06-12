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
    /// The exact words the clip must speak — what
    /// scripts/check_script_fit.py verifies against the transcript.
    #[serde(default)]
    pub expected_script: String,
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
                    .map(|s| {
                        // Oracle bullets carry checkbox prefixes.
                        s.trim_start_matches("[ ]")
                            .trim_start_matches("[x]")
                            .trim()
                            .to_string()
                    })
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
    // Groomed tickets phrase their "done when" as `## Oracle` checkboxes;
    // either section is the spec's acceptance contract.
    let mut acceptance_criteria = bullets(section(&prd.raw, "Acceptance Criteria"));
    if acceptance_criteria.is_empty() {
        acceptance_criteria = bullets(section(&prd.raw, "Oracle"));
    }
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
/// 90s TV infomercials, cryptid selfie vlogs, Italian-brainrot creature
/// reveals, fake future documentaries, and gen-z explainers. Each spec is
/// assigned one deterministically so the feed has variety and re-renders
/// are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainrotFormat {
    FruitDrama,
    GenZExplainer,
    CryptidVlog,
    ItalianBrainrot,
    StreetInterview,
    Infomercial,
}

impl BrainrotFormat {
    pub const ALL: [BrainrotFormat; 6] = [
        BrainrotFormat::FruitDrama,
        BrainrotFormat::Infomercial,
        BrainrotFormat::CryptidVlog,
        BrainrotFormat::ItalianBrainrot,
        BrainrotFormat::StreetInterview,
        BrainrotFormat::GenZExplainer,
    ];

    pub fn tone(self) -> &'static str {
        match self {
            BrainrotFormat::FruitDrama => "fruit_drama_v3",
            BrainrotFormat::GenZExplainer => "genz_explainer_v3",
            BrainrotFormat::CryptidVlog => "cryptid_vlog_v3",
            BrainrotFormat::ItalianBrainrot => "italian_brainrot_v3",
            BrainrotFormat::StreetInterview => "street_interview_v4",
            BrainrotFormat::Infomercial => "infomercial_v1",
        }
    }
}

/// Rotate formats by feed position so consecutive videos never share a
/// format — variety across the feed beats per-content stability here.
pub fn format_for(priority: usize) -> BrainrotFormat {
    BrainrotFormat::ALL[priority % BrainrotFormat::ALL.len()]
}

/// Count spoken words: tokens with no alphanumeric content (a lone "—",
/// stray punctuation) take no time to say and don't count.
fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .count()
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

/// How many spoken words fit in a clip. Measured against real veo3.1
/// renders: characters pace nearer 2 words/sec than the 2.4 we first
/// assumed, and they idle for a beat before the first line — so we budget
/// 2.0 words/sec against a 2s reserve. The previous 2.4 w/s × 1.5s-reserve
/// budget produced clips that cut off mid-script.
pub fn word_budget(duration_sec: u32) -> usize {
    (((duration_sec as f64) - 2.0) * 2.0).max(5.0) as usize
}

/// Words a compressed line must never end on — a clipped line that trails
/// off with "so" or "until" reads like a cutoff, which is the exact thing
/// we're eliminating.
const DANGLERS: &[&str] = &[
    "so", "and", "or", "but", "the", "a", "an", "to", "of", "for", "on", "in", "with", "by",
    "that", "which", "until", "when", "while", "as", "is", "are", "its", "their", "it", "what",
    "who", "how", "why", "where", "whether", "every", "each", "instead", "without", "via", "from",
    "into", "than", "then", "also", "does", "do", "did", "can", "cannot", "could", "should",
    "would", "will", "must", "may", "might", "has", "have", "had", "be", "been", "was", "were",
    "not", "never",
];

/// Words that can open a mid-sentence clause run and still read like the
/// start of a sentence (articles, demonstratives, pronouns, quantifiers).
const RUN_STARTERS: &[&str] = &[
    "the", "a", "an", "this", "that", "these", "those", "it", "we", "you", "they", "no", "every",
    "each", "all", "our", "your", "its",
];

/// Words that open a subordinate intro phrase ("After a swipe, …"): a
/// clause run starting here is not a sentence on its own.
const INTRO_SUBORDINATORS: &[&str] = &[
    "after", "before", "when", "while", "once", "if", "as", "until", "during", "upon", "without",
    "unless", "since", "with",
];

/// Spoken lines double as on-screen captions: start them like sentences.
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Strip trailing connectives and punctuation so a line ends on a word
/// that can carry a full stop.
fn strip_danglers(text: &str) -> String {
    let norm = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let mut words: Vec<&str> = text.split_whitespace().collect();
    while words.len() > 2 {
        let last = norm(words.last().unwrap());
        if last.is_empty() || DANGLERS.contains(&last.as_str()) {
            words.pop();
            continue;
        }
        // A trailing "<preposition> <article> <word>" stub ("to a looping")
        // is a clipped phrase: the article hides the dangling preposition.
        if words.len() > 4 {
            let article = matches!(norm(words[words.len() - 2]).as_str(), "a" | "an" | "the");
            let prep_dangles = DANGLERS.contains(&norm(words[words.len() - 3]).as_str());
            if article && prep_dangles {
                words.truncate(words.len() - 3);
                continue;
            }
        }
        break;
    }
    words
        .join(" ")
        .trim_end_matches([',', ';', ':', '—', ' '])
        .trim_end()
        .to_string()
}

/// Remove em-dash asides ("— like this —") and parenthesized groups:
/// parentheticals read fine on the page but burn spoken-word budget
/// without carrying the core clause — and "(config `flag`, default 2)"
/// must never be read aloud.
fn remove_asides(text: &str) -> String {
    let mut out = text.replace('`', "");
    while let Some(first) = out.find('—') {
        let Some(second_rel) = out[first + '—'.len_utf8()..].find('—') else {
            break;
        };
        let second = first + '—'.len_utf8() + second_rel;
        let mut next = out[..first].trim_end().to_string();
        next.push(' ');
        next.push_str(out[second + '—'.len_utf8()..].trim_start());
        out = next;
    }
    while let Some(open) = out.find('(') {
        let Some(close_rel) = out[open..].find(')') else {
            break;
        };
        let mut next = out[..open].trim_end().to_string();
        next.push(' ');
        next.push_str(out[open + close_rel + 1..].trim_start());
        out = next;
    }
    out.trim().to_string()
}

/// The longest contiguous run of clause segments that fits the budget.
/// A whole clause reads like a sentence; a word-clipped fragment reads
/// like a cutoff — which is the artifact we're eliminating. Runs may span
/// weak boundaries (commas, em-dashes) but never strong ones (periods,
/// colons, semicolons): "state: renders" is not a spoken phrase.
fn best_clause_run(text: &str, max_words: usize) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for zone in text.split(['.', ';', ':']) {
        let mut segments: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        for (i, c) in zone.char_indices() {
            if matches!(c, ',' | '—') {
                if zone[start..i].trim().chars().any(char::is_alphanumeric) {
                    segments.push((start, i));
                }
                start = i + c.len_utf8();
            }
        }
        if zone[start..].trim().chars().any(char::is_alphanumeric) {
            segments.push((start, zone.len()));
        }
        for i in 0..segments.len() {
            // A run may only start mid-sentence if it begins like a noun
            // phrase — "the operator sees…" reads fine, "merges without…"
            // is a beheaded verb.
            let first = zone[segments[i].0..segments[i].1]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if i != 0 && !RUN_STARTERS.contains(&first.as_str()) {
                continue;
            }
            // "After a swipe" alone is an intro phrase, not a sentence.
            if INTRO_SUBORDINATORS.contains(&first.as_str()) {
                continue;
            }
            for j in i..segments.len() {
                let run = zone[segments[i].0..segments[j].1].trim();
                let count = words(run);
                if count > max_words {
                    break;
                }
                if count >= 3 && best.is_none_or(|(b, _)| count > b) {
                    best = Some((count, run));
                }
            }
        }
    }
    best.map(|(_, run)| run.to_string())
}

/// Compress spec prose into a coherent spoken line: drop em-dash asides,
/// cut subordinate "so/because/..." tails, then prefer whole clause runs
/// over word clipping, and always end on a real word.
pub fn tighten(text: &str, max_words: usize) -> String {
    let text = remove_asides(text.trim());
    let mut head = [" so ", " because ", " such that ", " in order to "]
        .iter()
        .filter_map(|d| text.find(d).map(|i| &text[..i]))
        .min_by_key(|s| s.len())
        .unwrap_or(&text)
        .trim_end_matches('.');
    if words(head) <= max_words {
        return strip_danglers(head);
    }
    // Over budget and opening with "With ffmpeg present, …"-style intro:
    // the intro phrase is never the payload — drop it and keep the clause.
    let first = head
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    if INTRO_SUBORDINATORS.contains(&first.as_str()) {
        if let Some(comma) = head.find(',') {
            head = head[comma + 1..].trim_start();
            if words(head) <= max_words {
                return strip_danglers(head);
            }
        }
    }
    if let Some(run) = best_clause_run(head, max_words) {
        return strip_danglers(&run);
    }
    let clipped = head
        .split_whitespace()
        .take(max_words.max(1))
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = strip_danglers(&clipped);
    // A clip that lands just after a conjunction ("…drain a wallet or
    // fork-bomb") amputated a coordination; drop the stump.
    let tail: Vec<&str> = out.split_whitespace().collect();
    if tail.len() > 3 {
        let second_last = tail[tail.len() - 2].to_lowercase();
        if second_last == "and" || second_last == "or" {
            out = strip_danglers(&tail[..tail.len() - 2].join(" "));
        }
    }
    out
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

/// Hook/goal phrasing templates. The criterion line stays canonical
/// ("Not done until …") — it is the verifiable spec anchor — but the rest
/// of the script gets seeded variety so the feed never reads samey.
/// Each entry: (hook prefix, hook suffix, goal prefix).
const SCRIPT_TEMPLATES: [(&str, &str, &str); 5] = [
    ("", "", ""),
    ("Breaking: ", "", ""),
    ("", " just dropped", ""),
    ("Nobody mentions ", "", ""),
    ("", "", "The mission: "),
];

pub fn plan_script(
    title: &str,
    goal: &str,
    criterion: &str,
    duration_sec: u32,
    seed: u64,
) -> SpokenScript {
    let budget = word_budget(duration_sec);
    // Template filler is a luxury: a template is only eligible when the
    // budget can absorb its overhead without starving the goal line.
    // Short clips always speak the classic form — every word is spec.
    let eligible: Vec<&(&str, &str, &str)> = SCRIPT_TEMPLATES
        .iter()
        .filter(|(pre, post, gpre)| {
            let overhead = words(pre) + words(post) + words(gpre);
            overhead == 0 || budget >= 18 + overhead
        })
        .collect();
    let (hook_pre, hook_post, goal_pre) = *eligible[(seed as usize) % eligible.len()];
    // The hook is a short title fragment; the goal line is the payload and
    // gets the bulk of the budget — the spec must come through clearly.
    // Template filler words are real spoken words: the hook's land in
    // `used`, the goal prefix's shrink the goal's own allowance.
    let hook = format!(
        "{hook_pre}{}{hook_post}.",
        tighten(title, budget.saturating_sub(2).clamp(1, 4))
    );
    let mut used = words(&hook);
    // On long clips, reserve room so the first acceptance criterion — the
    // "done when" that makes a spec legible — always gets spoken whole.
    let reserve = if budget >= 18 { 8 } else { 0 };
    let goal_line = format!(
        "{goal_pre}{}.",
        capitalize(&tighten(
            goal,
            budget
                .saturating_sub(used + reserve + words(goal_pre))
                .clamp(1, 11)
        ))
    );
    used += words(&goal_line);
    let remaining = budget.saturating_sub(used);
    // "Not done until" costs 3 words; only speak the criterion if at least
    // a few words of it fit — a truncated stump is worse than silence.
    let criterion = (remaining >= 6).then(|| {
        format!(
            "Not done until {}.",
            tighten(criterion, remaining.saturating_sub(3))
        )
    });
    SpokenScript {
        hook,
        goal: goal_line,
        criterion,
    }
}

/// Pick one ingredient from a pool, deterministically per seed. `salt`
/// decorrelates multiple pools fed by the same seed.
fn pick<'a>(pool: &[&'a str], seed: u64, salt: u64) -> &'a str {
    pool[((seed ^ (salt.wrapping_mul(0x9e3779b97f4a7c15))) % pool.len() as u64) as usize]
}

/// Build the video-model prompt for one format. The dialogue quotes the
/// actual spec text — the video must communicate the spec, not just vibe —
/// and the whole script must finish before the clip ends. Scene
/// ingredients (cast, setting, persona) are seeded per spec: anchors stay
/// (bigfoot vlogs, fruit soap operas, pitchmen), details stay fresh.
fn prompt_header(duration_sec: u32) -> String {
    format!(
        "Vertical 9:16 video, exactly {duration_sec} seconds, with native audio: \
         clear spoken dialogue, sound effects, and music as described. Huge bold \
         meme-style captions with thick outlines pop onto the screen word by word, \
         perfectly synced to the dialogue — the captions are a main character, and \
         a viewer with sound off must still be able to read the entire script. \
         High energy throughout: the camera never sits still — punch-in zooms, \
         whip pans, dramatic push-ins on every beat."
    )
}

fn prompt_pacing(word_count: usize, duration_sec: u32, full_text: &str) -> String {
    format!(
        "Dialogue starts within the first second — no silent intro. The complete spoken \
         script is exactly the quoted text above — {} words total. Every word must be \
         spoken at a natural energetic pace and FINISHED by second {} of the {}-second \
         video, leaving the last moments for a held reaction shot or visual beat. \
         Never cut off mid-sentence; the clip must feel like one complete unit. \
         Full script for reference: \"{}\"",
        word_count,
        duration_sec.saturating_sub(2).max(1),
        duration_sec,
        full_text,
    )
}

const PROMPT_GUARDRAIL: &str =
    "All spoken lines must stay faithful to the quoted script. Do not invent \
     shipped features, metrics, customer names, or implementation details. \
     Do not claim anything has shipped or that tests pass. \
     On-screen captions, if any, must match the dialogue.";

/// Wrap an externally-written scene + script in the standard provider-prompt
/// frame (caption/motion header, pacing contract, fidelity guardrail). The
/// LLM scriptwriter path composes its prompts through this so every render —
/// templated or LLM-written — carries the same verifiable speech contract.
pub fn compose_provider_prompt(scene: &str, script_text: &str, duration_sec: u32) -> String {
    let scene = format!(
        "{} The character delivers, every word crisp and clearly intelligible: \"{}\"",
        scene.trim_end(),
        script_text
    );
    format!(
        "{}\n{}\n{}\n{}",
        prompt_header(duration_sec),
        scene,
        prompt_pacing(words(script_text), duration_sec, script_text),
        PROMPT_GUARDRAIL
    )
}

fn format_prompt(
    format: BrainrotFormat,
    script: &SpokenScript,
    duration_sec: u32,
    seed: u64,
) -> String {
    let header = prompt_header(duration_sec);
    let hook = &script.hook;
    let goal = &script.goal;
    let scene = match format {
        BrainrotFormat::FruitDrama => {
            let accuser = pick(
                &[
                    "furious strawberry",
                    "seething grape",
                    "heartbroken peach",
                    "betrayed pineapple",
                ],
                seed,
                1,
            );
            let accused = pick(
                &[
                    "guilty mango",
                    "smug banana",
                    "scheming lime",
                    "innocent-looking blueberry",
                ],
                seed,
                2,
            );
            let set = pick(
                &[
                    "A sunlit kitchen counter",
                    "A picnic table at golden hour",
                    "A farmers-market stall at dawn",
                ],
                seed,
                3,
            );
            let confession = match &script.criterion {
                Some(c) => format!(" The {accused} looks away and whispers: \"{c}\""),
                None => format!(" The {accused} looks away in shame, silent."),
            };
            format!(
                "AI fruit drama soap opera. {set} shot like a telenovela: shallow depth of \
                 field, dramatic golden lighting, slow push-in. Two anthropomorphic fruits with \
                 big expressive eyes and mouths — a {accuser} and a {accused}. \
                 The {accuser} gasps, voice trembling with betrayal: \"{hook} {goal}\" \
                 Dramatic zoom.{confession} Thunder crack, telenovela string sting. Played completely straight."
            )
        }
        BrainrotFormat::GenZExplainer => {
            let where_ = pick(
                &[
                    "in their bedroom with a ring light",
                    "in a parked car, phone on the dashboard",
                    "at a dorm desk lit by RGB strips",
                ],
                seed,
                1,
            );
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\" Vine boom on the last line."),
                None => " Vine boom at the end.".to_string(),
            };
            format!(
                "Chaotic gen-z talking-head explainer. A twentysomething films a vertical selfie \
                 video {where_}, talking fast straight into the camera with punch-in zooms and \
                 big bold captions appearing word by word. \
                 They say, completely serious: \"{hook} {goal}\"{tail}"
            )
        }
        BrainrotFormat::CryptidVlog => {
            let cryptid = pick(
                &[
                    "Bigfoot",
                    "A chill yeti",
                    "The Mothman",
                    "A surprisingly photogenic swamp creature",
                ],
                seed,
                1,
            );
            let locale = pick(
                &[
                    "a sunny pine forest",
                    "a misty mountain trail",
                    "a golden autumn birch forest",
                ],
                seed,
                2,
            );
            let tail = match &script.criterion {
                Some(c) => format!(" Then, deadpan to camera: \"{c}\""),
                None => String::new(),
            };
            format!(
                "Found-footage cryptid vlog. {cryptid} holds a GoPro at arm's length while striding \
                 through {locale}, fur rustling, lens slightly fisheye, very influencer. \
                 In a chill deep voice he says: \"{hook} {goal}\"{tail} Birdsong, crunching \
                 footsteps, one dramatic zoom to his face at the end."
            )
        }
        BrainrotFormat::ItalianBrainrot => {
            let creature = pick(
                &[
                    "a giant espresso cup with muscular human legs and a crocodile head",
                    "a marble statue of a ballerina with a steaming cappuccino for a head",
                    "a three-legged shark in designer sneakers posing like a renaissance hero",
                    // The freedom slot: let the model invent, but fence it
                    // away from the defaults so it can't collapse back.
                    "one you invent yourself: fuse one everyday household object with one \
                     animal; do NOT use a crocodile, shark, or coffee cup; surprise us",
                ],
                seed,
                1,
            );
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\""),
                None => String::new(),
            };
            format!(
                "Italian brainrot creature reveal. A surreal hybrid creature — {creature} — \
                 strikes heroic poses on a marble plaza, camera orbiting, renaissance lighting, \
                 fully cinematic. A bombastic opera narrator with a slight Italian flair bellows \
                 over orchestral hits, every English word crisp and clearly intelligible: \
                 \"{hook} {goal}\"{tail} Deadpan, epic, absurd."
            )
        }
        BrainrotFormat::StreetInterview => {
            let year = pick(&["2080", "2147", "3024"], seed, 1);
            let who = pick(
                &[
                    "a retired developer in futuristic streetwear",
                    "a former scrum master turned hover-cab driver",
                    "the last human programmer, now a beloved celebrity",
                ],
                seed,
                2,
            );
            let tail = match &script.criterion {
                Some(c) => format!(" \"{c}\""),
                None => String::new(),
            };
            format!(
                "Fake documentary street interview from the year {year}. A reporter holds a \
                 microphone to {who} on a neon city sidewalk, vertical handheld framing, ambient \
                 hover-traffic, nostalgic synth piano. They smile wistfully at the horizon and \
                 say, warm and clear: \"{hook} {goal}\"{tail} \
                 Slow documentary push-in, flying cars drifting past in the background."
            )
        }
        BrainrotFormat::Infomercial => {
            let set = pick(
                &[
                    "a bright studio kitchen set",
                    "a wood-paneled garage workshop set",
                    "a late-night shopping-channel desk with a spinning product pedestal",
                ],
                seed,
                1,
            );
            let tail = match &script.criterion {
                Some(c) => {
                    format!(
                        " Hard cut closer: he leans into the lens, eyebrows raised, and adds: \"{c}\" \
                         A giant starburst graphic flashes behind him as the studio audience gasps."
                    )
                }
                None => " A giant starburst graphic flashes as the studio audience applauds."
                    .to_string(),
            };
            format!(
                "A 1990s late-night TV infomercial shot on slightly grainy videotape with cheesy \
                 synth music. An overjoyed pitchman in a loud blazer stands in {set}, \
                 gesturing wildly at the camera while colorful starburst graphics \
                 and price-tag stickers pop on and off the screen. He booms with game-show \
                 enthusiasm, every word crisp and clearly intelligible: \"{hook} {goal}\"{tail} \
                 Rapid cuts, canned applause, VHS tracking flicker on the final frame."
            )
        }
    };
    let pacing = prompt_pacing(script.word_count(), duration_sec, &script.full_text());
    format!("{header}\n{scene}\n{pacing}\n{PROMPT_GUARDRAIL}")
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

    // Seeded by spec content: the same spec always renders the same way
    // (stable re-renders), but different specs draw different ingredients.
    let seed = crate::util::spec_seed(&prd.sha256);
    let script = plan_script(
        &prd.title,
        &brief.goal,
        &first_criterion,
        target_duration_sec,
        seed,
    );
    let provider_prompt = format_prompt(format, &script, target_duration_sec, seed);

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
        expected_script: script.full_text(),
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
        // At 8s the budget is too tight for the criterion line; a longer
        // clip speaks it.
        assert!(!sb.provider_prompt.contains("Refresh shows newest render"));
        let long = compile_storyboard(&p, &brief, 12);
        assert!(long.provider_prompt.contains("Refresh shows newest render"));
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
        assert_eq!(format_for(1), BrainrotFormat::Infomercial);
        assert_eq!(format_for(2), BrainrotFormat::CryptidVlog);
        assert_eq!(format_for(3), BrainrotFormat::ItalianBrainrot);
        assert_eq!(format_for(4), BrainrotFormat::StreetInterview);
        assert_eq!(format_for(5), BrainrotFormat::GenZExplainer);
        assert_eq!(format_for(6), BrainrotFormat::FruitDrama);
        for i in 0..12 {
            assert_ne!(format_for(i), format_for(i + 1));
        }
    }

    #[test]
    fn script_phrasing_varies_with_seed_but_respects_budget() {
        let title = "Brainrot Vibe Meter";
        let goal = "Record a simple human vibe rating on each render";
        let criterion = "Render detail allows rating a clip from cursed to corporate";
        let texts: std::collections::HashSet<String> = (0..6)
            .map(|seed| plan_script(title, goal, criterion, 12, seed).full_text())
            .collect();
        // Different seeds produce genuinely different phrasings...
        assert!(texts.len() >= 3, "expected varied phrasings, got {texts:?}");
        for seed in 0..6 {
            let s = plan_script(title, goal, criterion, 12, seed);
            // ...every variant stays inside the spoken-word budget...
            assert!(
                s.word_count() <= word_budget(12),
                "seed {seed} over budget: {} words: {}",
                s.word_count(),
                s.full_text()
            );
            // ...and the canonical criterion line survives every template.
            assert!(
                s.criterion
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("Not done until"),
                "seed {seed} lost the criterion line: {:?}",
                s.criterion
            );
        }
        // Same seed, same script: re-renders stay stable.
        assert_eq!(
            plan_script(title, goal, criterion, 12, 3).full_text(),
            plan_script(title, goal, criterion, 12, 3).full_text()
        );
    }

    #[test]
    fn scene_ingredients_vary_with_seed_but_are_deterministic() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        let script = plan_script(&p.title, &brief.goal, "x", 12, 0);
        for format in BrainrotFormat::ALL {
            let scenes: std::collections::HashSet<String> = (0..8)
                .map(|seed| format_prompt(format, &script, 12, seed))
                .collect();
            assert!(scenes.len() >= 2, "{format:?} never varies its scene");
            assert_eq!(
                format_prompt(format, &script, 12, 5),
                format_prompt(format, &script, 12, 5),
                "{format:?} must be deterministic per seed"
            );
        }
    }

    #[test]
    fn prompts_demand_hypermaximalist_captions_and_motion() {
        let p = prd(SAMPLE);
        let brief = distill(&p);
        let sb = compile_with_format(&p, &brief, 12, BrainrotFormat::Infomercial);
        // 90s infomercial scene with the spec spoken by the pitchman.
        assert!(
            sb.provider_prompt.contains("infomercial"),
            "{}",
            sb.provider_prompt
        );
        assert!(
            sb.provider_prompt.contains("starburst"),
            "{}",
            sb.provider_prompt
        );
        // Shared header: captions are oversized meme text and the camera keeps moving.
        for format in BrainrotFormat::ALL {
            let prompt = compile_with_format(&p, &brief, 12, format).provider_prompt;
            assert!(prompt.contains("meme-style captions"), "{format:?}");
            assert!(prompt.contains("never sits still"), "{format:?}");
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
                compile_with_format(&p, &brief, 12, format)
                    .provider_prompt
                    .contains("Refresh shows newest render"),
                "{format:?} must speak the first acceptance criterion when the budget allows"
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

    /// Regressions caught on the real backlog: word-clipping produced
    /// fragments like "Stand up CI: every." — a clause run must win.
    #[test]
    fn tighten_prefers_whole_clauses_over_fragments() {
        assert_eq!(
            tighten("Stand up CI: every PR gated by fmt, clippy, tests", 4),
            "Stand up CI"
        );
        assert_eq!(
            tighten(
                "Garbage-collect generated state: renders, worktrees, media",
                4
            ),
            "Garbage-collect generated state"
        );
        // No clause boundary: clip, then drop danglers like "every".
        assert_eq!(
            tighten("Throttle and budget every money path", 4),
            "Throttle and budget"
        );
        assert_eq!(
            tighten("Stream media instead of buffering whole files in memory", 4),
            "Stream media"
        );
        // A mid-sentence clause run that fits beats a clipped opening.
        assert_eq!(
            tighten(
                "After a swipe, the operator sees what the agent is doing",
                8
            ),
            "the operator sees what the agent is doing"
        );
        // Em-dash asides are dropped whole: the core clause survives.
        assert_eq!(
            tighten("No branch — human or agent — merges without review", 7),
            "No branch merges without review"
        );
        // A clip landing right after a conjunction drops the stump.
        assert_eq!(
            tighten(
                "No user action can drain a wallet or fork-bomb the machine",
                9
            ),
            "No user action can drain a wallet"
        );
    }

    #[test]
    fn script_always_fits_the_word_budget() {
        let long_goal = "make the gallery always show the latest MP4 provenance for every \
                         spec in the backlog even when providers drift and budgets explode"
            .to_string();
        let long_criterion = "a route level test proves that the newest successful render is \
                              selected by provenance timestamp and never an older stale render";
        for duration in [4u32, 6, 8, 12] {
            for seed in 0..6 {
                let script = plan_script(
                    "Cache Chaos Exorcism",
                    &long_goal,
                    long_criterion,
                    duration,
                    seed,
                );
                assert!(
                    script.word_count() <= word_budget(duration),
                    "{} words exceeds budget {} at {duration}s seed {seed}",
                    script.word_count(),
                    word_budget(duration)
                );
            }
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
            0,
        );
        assert!(tight.criterion.is_none());
        assert!(tight.word_count() <= word_budget(4));

        // Roomy budget: criterion spoken.
        let roomy = plan_script("Spec", "Ship it.", "tests pass on refresh", 8, 0);
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
        assert!(sb.provider_prompt.contains("FINISHED by second 6"));
        assert!(sb
            .provider_prompt
            .contains("Dialogue starts within the first second"));
        assert!(sb.provider_prompt.contains("Never cut off mid-sentence"));
        assert!(sb.provider_prompt.contains("words total"));
    }
}
