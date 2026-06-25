# Legal/safety baseline before public launch

Priority: P2 · Status: done · Estimate: S

## Goal
Launch doesn't step on a rake: license chosen, AI-content disclosure stated, provider terms respected.

## Oracle
- [x] OSS license decided + LICENSE file; trademark sanity check on the product name.
- [x] README + site disclose AI-generated video and that spec content is sent to the video provider.
- [x] fal.ai + model ToS reviewed for redistribution of generated clips in marketing.
- [x] Runtime data-egress disclosure (UI + CLI) names exactly what spec text is sent to fal.ai and OpenRouter — not just README prose. (Groom 2026-06-17, security lane; the runtime-affordance complement to [[033-dispatch-untrusted-spec-hardening]].)
- [x] The disclosure enumerates BOTH egress payloads, not one: `prd.raw` → OpenRouter (scriptwriter, `scriptwriter.rs:102`) **and** spec title/goal/first-criterion → fal render prompt (`distill.rs`). The spec *title* is attacker-controlled (first `# ` line) and flows unfenced into the fal prompt, the PR title, the commit message, and the branch slug — argv tokens (no shell-injection), but spec-derived text that egresses. (Groom 2026-06-25, security lane.)

## Done
- `LICENSE` (MIT) added; `license = "MIT"` in `Cargo.toml`; trademark sanity check
  recorded in `docs/LEGAL.md` (re-run before commercial registration).
- `docs/LEGAL.md` documents the fal.ai + model + OpenRouter ToS review for
  marketing redistribution (snapshot 2026-06-25; re-verify per campaign).
- README "Legal / safety disclosure" section states AI-generated video +
  enumerates both egress payloads with source locations.
- Runtime data-egress disclosure (not just prose):
  - `doomscrum egress` (CLI) prints the enumeration.
  - `GET /api/egress` (HTTP) returns it as JSON.
  - `assets/index.html` `egress` chip + overlay panel surfaces it in the feed
    UI; the splash notes videos are AI-generated.
- `src/egress.rs` is the code-verified single source: it enumerates BOTH
  payloads — `prd.raw` → OpenRouter (`scriptwriter.rs:102`) and spec
  title/goal/first-criterion → fal (`distill.rs`) — and flags the
  attacker-controlled title. Tests assert both ids, both source paths, and
  the attacker-controlled note.
