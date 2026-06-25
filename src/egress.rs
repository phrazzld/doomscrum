//! Runtime data-egress disclosure. Enumerates exactly what spec-derived text
//! leaves the operator's machine, to which provider, via which code path — the
//! runtime affordance complement to README prose (backlog 022, security lane).
//!
//! The disclosure is a pure function of nothing (the payloads are static facts
//! about the codebase), so it is testable and feeds three surfaces:
//!   * `doomscrum egress` (CLI) — operator prints it before a paid run;
//!   * `GET /api/egress` (HTTP) — the feed UI surfaces it in the disclosure
//!     panel and the first-render consent flow;
//!   * the README / docs (prose) point here rather than restating the list.
//!
//! The list must enumerate BOTH egress payloads, not one: the full raw spec
//! goes to OpenRouter (scriptwriter), AND the spec title/goal/first-criterion
//! go to fal.ai (render prompt). The spec title is attacker-controlled (the
//! first `# ` line) and flows unfenced into the fal prompt, the PR title, the
//! commit message, and the branch slug — argv tokens (no shell injection), but
//! spec-derived text that egresses.

use serde::Serialize;

/// One named data-egress payload: what leaves, where it goes, and the code
/// path that sends it.
#[derive(Debug, Clone, Serialize)]
pub struct EgressPayload {
    /// Stable identifier (e.g. `scriptwriter-openrouter`).
    pub id: &'static str,
    /// Human-readable destination + provider.
    pub destination: &'static str,
    /// The provider that receives the data.
    pub provider: &'static str,
    /// The exact spec fields that egress, named precisely.
    pub fields: Vec<&'static str>,
    /// Source code location that composes and sends the payload.
    pub source: &'static str,
    /// When this egress is triggered.
    pub triggered_by: &'static str,
    /// Operator-relevant notes (caching, attacker-control, fencing).
    pub notes: &'static str,
}

/// The canonical, code-verified list of spec-derived data egress paths.
///
/// Order is intentional and load-bearing: the disclosure must enumerate BOTH
/// payloads, not one. Tests assert both ids are present.
pub fn payloads() -> Vec<EgressPayload> {
    vec![
        EgressPayload {
            id: "scriptwriter-openrouter",
            destination: "OpenRouter chat completions API",
            provider: "OpenRouter",
            fields: vec!["prd.raw — the full raw spec markdown, any format, any structure"],
            source: "src/scriptwriter.rs request_body — prd.raw is wrapped in an \
                     untrusted-data fence (util::wrap_untrusted_spec) and sent as the \
                     user turn; scriptwriter.rs:102",
            triggered_by: "script.mode = \"llm\" and a render or `doomscrum script` \
                            runs (the fixture/`fake` provider and template mode do \
                            not egress to OpenRouter)",
            notes: "The scriptwriter returns the spoken script + visual scene. \
                    Results are cached by spec sha256 + model, so a spec pays once \
                    and re-renders are free. The spec is carried as DATA inside a \
                    nonce'd fence so embedded directives cannot break out as \
                    instructions, but the raw text still leaves the machine.",
        },
        EgressPayload {
            id: "render-fal",
            destination: "fal.ai queue API (text-to-video model)",
            provider: "fal.ai",
            fields: vec![
                "spec title — the first `# ` line; attacker-controlled",
                "spec goal — the `## Goal` body (falls back to the title)",
                "first acceptance criterion — first bullet of `## Oracle` \
                 (or `## Acceptance Criteria`)",
            ],
            source: "src/distill.rs compile_with_format → plan_script → \
                     format_prompt → Storyboard.provider_prompt, sent by \
                     src/providers/fal.rs",
            triggered_by: "provider = \"fal\" (or any real cloud render provider) \
                            and a render runs; the `fake` fixture provider never \
                            egresses",
            notes: "These three spec fields are distilled into the spoken script \
                    (hook/goal/criterion) and embedded, quoted, inside the composed \
                    provider_prompt sent to fal.ai. The spec title is \
                    attacker-controlled and flows UNFENCED into the fal prompt, the \
                    PR title, the commit message, and the branch slug — argv tokens \
                    (no shell injection), but spec-derived text that egresses. \
                    Render provenance (provider, model, sha256, cost) is persisted \
                    per render; the prompt itself is not persisted to fal.ai beyond \
                    the single render call.",
        },
    ]
}

/// Render the disclosure as CLI-friendly text (mirrors the `doctor` report
/// style). Used by `doomscrum egress`.
pub fn render_cli() -> String {
    let mut out = String::new();
    out.push_str("DoomScrum data-egress disclosure\n");
    out.push_str("================================\n\n");
    out.push_str(
        "Spec-derived text that leaves this machine when real providers are \
         used. The free `fake` fixture provider and template script mode never \
         egress.\n\n",
    );
    for p in payloads() {
        out.push_str(&format!("▸ {} → {}\n", p.id, p.destination));
        out.push_str(&format!("   provider:     {}\n", p.provider));
        out.push_str("   spec fields:  ");
        out.push_str(&p.fields.join("; "));
        out.push('\n');
        out.push_str(&format!("   source:       {}\n", p.source));
        out.push_str(&format!("   triggered by: {}\n", p.triggered_by));
        out.push_str(&format!("   notes:        {}\n\n", p.notes));
    }
    out.push_str(
        "Confirm before a paid run: `doomscrum doctor` checks keys and config; \
         `doomscrum egress` prints this disclosure. The feed UI surfaces the \
         same list in its disclosure panel.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_egress_payloads_are_enumerated() {
        let ids: Vec<_> = payloads().iter().map(|p| p.id).collect();
        assert!(
            ids.contains(&"scriptwriter-openrouter"),
            "missing OpenRouter payload: {ids:?}"
        );
        assert!(ids.contains(&"render-fal"), "missing fal payload: {ids:?}");
        // Exactly the two known payloads — a new egress path must be added
        // here deliberately, not silently.
        assert_eq!(payloads().len(), 2, "egress list changed: {ids:?}");
    }

    #[test]
    fn openrouter_payload_names_prd_raw_and_scriptwriter_source() {
        let payloads = payloads();
        let p = payloads
            .iter()
            .find(|p| p.id == "scriptwriter-openrouter")
            .unwrap();
        assert!(
            p.fields.iter().any(|f| f.contains("prd.raw")),
            "must name prd.raw as the egressing field: {:?}",
            p.fields
        );
        assert!(
            p.source.contains("scriptwriter.rs"),
            "must point at scriptwriter.rs: {}",
            p.source
        );
        assert_eq!(p.provider, "OpenRouter");
    }

    #[test]
    fn fal_payload_names_title_goal_and_first_criterion() {
        let payloads = payloads();
        let p = payloads.iter().find(|p| p.id == "render-fal").unwrap();
        let joined = p.fields.join("\n");
        assert!(
            joined.contains("title"),
            "must name the spec title: {joined}"
        );
        assert!(joined.contains("goal"), "must name the spec goal: {joined}");
        assert!(
            joined.contains("criterion"),
            "must name the first acceptance criterion: {joined}"
        );
        assert!(
            p.source.contains("distill.rs"),
            "must point at distill.rs: {}",
            p.source
        );
        assert_eq!(p.provider, "fal.ai");
    }

    #[test]
    fn fal_payload_flags_attacker_controlled_title() {
        let payloads = payloads();
        let p = payloads.iter().find(|p| p.id == "render-fal").unwrap();
        assert!(
            p.notes.contains("attacker-controlled"),
            "must flag that the title is attacker-controlled: {}",
            p.notes
        );
    }

    #[test]
    fn render_cli_mentions_both_providers_and_both_ids() {
        let text = render_cli();
        assert!(text.contains("OpenRouter"), "{text}");
        assert!(text.contains("fal.ai"), "{text}");
        assert!(text.contains("scriptwriter-openrouter"), "{text}");
        assert!(text.contains("render-fal"), "{text}");
        assert!(text.contains("prd.raw"), "{text}");
    }
}
