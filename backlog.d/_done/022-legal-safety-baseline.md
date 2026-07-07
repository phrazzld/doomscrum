# Legal/safety baseline before public launch

Priority: P2 · Status: pending · Estimate: S

## Goal
Launch doesn't step on a rake: license chosen, AI-content disclosure stated, provider terms respected.

## Oracle
- [x] OSS license decided (MIT, Misty Step LLC) + LICENSE file.
- [ ] Trademark clearance for the product name — NOT done (no search performed); see docs/LEGAL.md "Trademark (TODO)".
- [x] README + site disclose AI-generated video and that spec content is sent to the video provider.
- [ ] fal.ai + model + OpenRouter ToS reviewed for redistribution — NOT done; tracked as a pre-launch TODO in docs/LEGAL.md.
- [x] Runtime data-egress disclosure (UI + CLI) names exactly what spec text is sent to fal.ai and OpenRouter — not just README prose. (Groom 2026-06-17, security lane; the runtime-affordance complement to [[033-dispatch-untrusted-spec-hardening]].)
- [x] The disclosure enumerates BOTH egress payloads, not one: `prd.raw` → OpenRouter (scriptwriter, `scriptwriter.rs:102`) **and** spec title/goal/first-criterion → fal render prompt (`distill.rs`). The spec *title* is attacker-controlled (first `# ` line) and flows unfenced into the fal prompt, the PR title, the commit message, and the branch slug — argv tokens (no shell-injection), but spec-derived text that egresses. (Groom 2026-06-25, security lane.)

## Shipped (engineering — PR #8, merged 2026-06-25)
- `LICENSE` (MIT, Misty Step LLC) added; `license = "MIT"` already in `Cargo.toml`.
- README "Legal / safety disclosure" section states AI-generated video +
  enumerates both egress payloads with source locations.
- Runtime data-egress disclosure (not just prose):
  - `doomscrum egress` (CLI) prints the enumeration.
  - `GET /api/egress` (HTTP) returns it as JSON.
  - `assets/index.html` `egress` chip + overlay panel surfaces it in the feed
    UI; the splash notes videos are AI-generated.
- `src/egress.rs` is the code-verified single source: it enumerates BOTH
  payloads — `prd.raw` → OpenRouter (`scriptwriter.rs`) and spec
  title/goal/first-criterion → fal (`distill.rs`) — and flags the
  attacker-controlled title. `egress::summary()` is the one prose source for
  CLI + HTTP. Tests assert both ids, both source paths, and the attacker note.

## Remaining (pre-launch — requires human/counsel, NOT done)
- Trademark clearance search for "DoomScrum" (none performed yet).
- fal.ai + model + OpenRouter ToS review for marketing redistribution.
- `docs/LEGAL.md` tracks both as a DRAFT pre-launch checklist, not a completed
  review. This ticket stays open until they are done.
