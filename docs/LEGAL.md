# Legal / safety baseline (backlog 022)

Status: **DRAFT pre-launch checklist (2026-06-25).** This is NOT a completed
legal review — it is the operator-facing list of what must be decided and
verified before a public launch, plus the disclosures already wired into the
product. It is not legal advice. Items marked TODO are unverified; complete them
(and engage counsel where noted) before relying on them.

## OSS license

DoomScrum is released under the **MIT License** (`LICENSE` at the repo root;
`license = "MIT"` in `Cargo.toml`). MIT was chosen because the product is a
developer tool that swipes dispatch real coding agents against private repos:
permissive licensing matches the "runs locally, BYO keys" distribution model
in `COMMERCIAL_MODEL.md` and removes friction for self-hosting. A copyleft
license would deter the exact self-hosting audience the local-first path
targets.

## Trademark — product name (TODO before launch)

**Not yet cleared — no trademark search has been performed.** Before any
commercial registration or branding spend:

- Run a real trademark clearance search for "DoomScrum" / "Doom Scrum" in the
  relevant software/services class(es).
- Note: "Doom" is a well-known id Software trademark for the game franchise. Do
  not market "DoomScrum" in any way that implies affiliation with id Software,
  and confirm the compound does not collide in the relevant class.
- If defending the name matters, file for registration after a clean search.

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
   Source: `src/scriptwriter.rs` (`request_body`).
2. **fal.ai (render prompt)** — the spec **title** (attacker-controlled, the
   first `# ` line), **goal**, and **first acceptance criterion** are
   distilled into the provider prompt sent to fal.ai's text-to-video model.
   Source: `src/distill.rs` (`compile_with_format` → `format_prompt`),
   sent by `src/providers/fal.rs`.

The `fake` fixture provider and `templates` script mode never egress.

## fal.ai + model ToS — redistribution of generated clips in marketing

**These terms have NOT been reviewed.** Before redistributing generated clips in
marketing material, read the then-current terms of:

- **fal.ai** (https://fal.ai/terms) — the render service ToS govern ownership,
  licensing, and permitted use of generated outputs. Read the current version to
  confirm whether it permits marketing redistribution; do not assume it does.
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

## 2026-07-07 review — marketing site launch (doomscrum-902)

Read the then-current ToS at both providers before wiring `site/` (fetched
2026-07-07, not a paraphrase from memory):

- **fal.ai** (`fal.ai/terms`): does not affirmatively grant ownership of
  Output Content — "Company does not represent, warrant, or covenant that
  any Output Content will be original, will not infringe rights of any third
  party ... or otherwise entitle Company to any intellectual property rights
  in any Output Content." Use is "at their own risk." This is a
  **no-IP-warranty** posture, not an explicit redistribution prohibition —
  materially different from a clause that bars marketing use outright.
- **OpenRouter** (`openrouter.ai/terms` §6.1): "Your ownership rights in the
  Output are set forth in the Model Terms for each Model you use" — defers
  to the underlying model provider. Applies to the scriptwriter's text
  output only; the video itself is a fal.ai render, not an OpenRouter output.

Neither provider's terms were found to explicitly prohibit displaying
generated video on a project's own marketing site. Both decline to warrant
IP-cleanliness of outputs, which is a real, standing risk (see "fal.ai +
model ToS" above) but not a blocker for a solo-operator, MIT-licensed,
AI-generation-disclosed dev tool's own site — as opposed to a paid ad
campaign or third-party redistribution.

**Waiver (recorded, dated, scoped — not silent):**

- **Provider-terms TODO — waived for the `site/` launch only.** The
  Remotion demo cartridge (`demo/out/doomscrum-demo.mp4`, embedded at
  `site/assets/demo.mp4`) splices in three real fal.ai-rendered brainrot
  clips (`infomercial.mp4`, `cryptid_vlog.mp4`, `italian_brainrot.mp4`) to
  show the actual product output — showing the brainrot format honestly is
  core to the pitch (VISION.md: "the brainrot is load-bearing, not
  decoration"). Given the no-explicit-prohibition finding above, AI
  generation is disclosed on the site itself (not just README/LEGAL.md),
  and there is no separate paid marketing campaign, this specific use is
  waived. **Re-open before any paid ad spend, sponsored post, or
  third-party redistribution** — that scenario needs the full per-model
  (sora-2 / veo3.1 / seedance / ltx-2.3) terms check this file has always
  called for. The site's autoplay hero loop
  (`site/assets/hero-loop.mp4`) was deliberately re-cut to the `prHook`
  beat only (0–3.2s, a Remotion UI composition of a real PR card) — it
  contains zero third-party model output, so it carries none of this risk.
- **Trademark TODO — waived for the `site/` launch, unaffected.** The
  clearance requirement above is scoped to "commercial registration or
  branding spend" (filing, paid campaigns). A GitHub Pages site for an
  MIT-licensed solo project is neither; using the product's own name on its
  own site does not trigger that gate. The trademark search itself stays
  open and must close before any registration or paid branding spend.

This still is not legal advice; both waivers are scoped to the specific
`site/` launch described in Powder `doomscrum-902` and do not extend to
future commercial use.
