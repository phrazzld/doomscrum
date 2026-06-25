# Legal / safety baseline (backlog 022)

Status: **reviewed 2026-06-25** for the public-launch legal/safety baseline.
This file is the operator-facing review record, not legal advice. Re-review
before any redistribution or commercial use that this file does not cover.

## OSS license

DoomScrum is released under the **MIT License** (`LICENSE` at the repo root;
`license = "MIT"` in `Cargo.toml`). MIT was chosen because the product is a
developer tool that swipes dispatch real coding agents against private repos:
permissive licensing matches the "runs locally, BYO keys" distribution model
in `COMMERCIAL_MODEL.md` and removes friction for self-hosting. A copyleft
license would deter the exact self-hosting audience the local-first path
targets.

## Trademark sanity check on the product name

"DoomScrum" was sanity-checked 2026-06-25:

- A web search for "DoomScrum" and "Doom Scrum" returned no live software
  product, registered trademark, or prominent commercial use as of the check
  date. "Doom" alone is a well-known id Software trademark for the game
  franchise; "DoomScrum" as a compound does not collide with that mark in
  the software/services class for a backlog-triage tool, but the compound
  should not be marketed in a way that implies affiliation with id Software.
- This is a sanity check, not a formal trademark clearance search. Before any
  commercial registration or branding spend, commission a real clearance
  search and, if defending the name matters, file for registration in the
  relevant class(es).
- Re-run this check before public launch and re-record the date.

## AI-content disclosure

Videos produced by DoomScrum are **AI-generated**. They do not depict real
events or real people. The spoken and captioned content is derived from
backlog spec text; it is not verified fact about the spec's subject.

This disclosure is stated in:

- `README.md` — the "Legal / safety disclosure" section.
- `assets/index.html` — the splash screen and the `egress` disclosure panel.

## Data-egress disclosure (runtime)

The disclosure enumerates BOTH egress payloads, not one. It is available at
runtime (not just README prose) via:

- `doomscrum egress` (CLI) — prints the enumeration.
- `GET /api/egress` (HTTP) — returns the enumeration as JSON.
- The feed UI `egress` chip — surfaces the same list in a disclosure panel.

The two payloads (code-verified in `src/egress.rs`):

1. **OpenRouter (scriptwriter)** — `prd.raw` (the full raw spec markdown)
   goes to OpenRouter's chat-completions API when `script.mode = "llm"`.
   Source: `src/scriptwriter.rs` (`request_body`, ~line 102).
2. **fal.ai (render prompt)** — the spec **title** (attacker-controlled, the
   first `# ` line), **goal**, and **first acceptance criterion** are
   distilled into the provider prompt sent to fal.ai's text-to-video model.
   Source: `src/distill.rs` (`compile_with_format` → `format_prompt`),
   sent by `src/providers/fal.rs`.

The `fake` fixture provider and `templates` script mode never egress.

## fal.ai + model ToS — redistribution of generated clips in marketing

Before redistributing generated clips in marketing material, review the
then-current terms of:

- **fal.ai** (https://fal.ai/terms) — the render service ToS govern ownership,
  licensing, and permitted use of generated outputs. fal.ai's terms as of the
  review date grant the account owner rights to use generated outputs subject
  to the terms; confirm the current version permits marketing redistribution
  before any public campaign.
- **The underlying video model's terms** — each model (e.g. sora-2, veo3.1,
  seedance, ltx-2.3) on fal may carry provider-specific output-use terms.
  Marketing redistribution must satisfy the most restrictive applicable
  provider's terms for the model that produced each clip.
- **OpenRouter** (https://openrouter.ai/terms) — the scriptwriter LLM call
  produces the script/scene text, not the video. Text-output terms still
  apply if that text is redistributed (e.g. quoted in marketing).

Practical policy for marketing redistribution:

1. Re-verify each provider's then-current terms at the time of the campaign —
   terms change, and this 2026-06-25 review is a snapshot, not a standing
   grant.
2. Prefer clips generated under your own paid fal account for marketing;
   fixture/`fake` clips are not AI-generated video and are fine to use, but
   they are not representative marketing material for the AI pipeline.
3. Keep render provenance (`.doomscrum/renders/<spec-sha>/<render-id>.json`)
   for any clip used in marketing — it records the provider, model, and
   timestamp needed to map a clip back to the terms that governed its
   generation.
4. Do not imply the clip depicts real people or real events; label it as
   AI-generated in the marketing context.

This review does not constitute legal advice. For commercial marketing use,
confirm with counsel.
