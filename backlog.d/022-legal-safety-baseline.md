# Legal/safety baseline before public launch

Priority: P2 · Status: pending · Estimate: S

## Goal
Launch doesn't step on a rake: license chosen, AI-content disclosure stated, provider terms respected.

## Oracle
- [ ] OSS license decided + LICENSE file; trademark sanity check on the product name.
- [ ] README + site disclose AI-generated video and that spec content is sent to the video provider.
- [ ] fal.ai + model ToS reviewed for redistribution of generated clips in marketing.
- [ ] Runtime data-egress disclosure (UI + CLI) names exactly what spec text is sent to fal.ai and OpenRouter — not just README prose. (Groom 2026-06-17, security lane; the runtime-affordance complement to [[033-dispatch-untrusted-spec-hardening]].)
- [ ] The disclosure enumerates BOTH egress payloads, not one: `prd.raw` → OpenRouter (scriptwriter, `scriptwriter.rs:102`) **and** spec title/goal/first-criterion → fal render prompt (`distill.rs`). The spec *title* is attacker-controlled (first `# ` line) and flows unfenced into the fal prompt, the PR title, the commit message, and the branch slug — argv tokens (no shell-injection), but spec-derived text that egresses. (Groom 2026-06-25, security lane.)
