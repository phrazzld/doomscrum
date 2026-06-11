# The efficiency makeover

How DoomScrum gets cheap enough to point at a real backlog (2026-06-10
research; prices verified on fal that day). The strategies stack — each
multiplies the others.

## The stack

1. **Render the viewport, not the backlog** (backlog 027). Most specs are
   never watched. Render just-in-time as items approach the top of the
   feed; promote a spec to a hero pipeline only when engagement proves it
   earns one. This is a unit-count cut (~10x), independent of unit price.
2. **Render-mix portfolio** (shipped — `[[video.mix]]` in doomscrum.toml).
   Each spec deterministically draws a pipeline by content hash: most land
   cheap/short, a weighted few land hero. Current default averages
   **$0.77/clip** vs $1.20 flat sora. Mix entries are config — tuning the
   average is a one-line change.
   *Profiles (shipped 2026-06-10):* `[profiles.<name>]` tables override
   provider/model/mix per context; `profile = "dev"` keeps everyday local
   work on the free fixture provider, `--profile content` flips to the
   paid mix only when iterating on generated media. Once 026 lands, the
   dev profile should point at the stills pipeline instead of the fixture.
3. **Stills pipeline: image + Ken Burns + TTS** (backlog 026). Keyframe
   image (~$0.03) + free parallax motion + TTS voiceover (~$0.01) + our
   caption/ribbon overlay ≈ **$0.05/clip**, fully bespoke per spec.
   Becomes the heaviest weight in the mix once built. Deterministic audio
   also eliminates both paid re-roll classes (silent duds, garbled
   diction).
4. **Local open weights** (backlog 028). LTX-2 19B distilled FP8 runs on
   16GB consumer GPUs; Wan 1.3B does 5s/720p in ~45s on a 4090. Marginal
   cost ≈ electricity. The audience is developers — many own the
   hardware. `provider = "local"` makes the wallet cap irrelevant for
   them; fal-rented H100s ($1.89/hr) cover batch queues without hardware.
5. **Commercial: tinder economics** (backlog 029). N free swipes/day;
   paid tiers buy more swipes and hero-render weight. Free tier rides the
   cheapest pipelines; BYOK fal key or bundled credits carry the premium
   ones. Vendor COGS per free user ≈ pennies under strategies 1–4.

## Pipeline price ladder (verified 2026-06-10)

| Pipeline | $/clip | Notes |
|---|---|---|
| stills + Ken Burns + TTS | ~0.05 | not built yet (026); fully bespoke |
| ltx-2.3 fast 8s 1080p | 0.32 | native audio; diction unverified |
| veo3.1/lite 8s 720p | 0.40 | captions garble; no criterion at 8s |
| sora-2 12s | 1.20 | speaks criterion; correct captions |
| seedance-2.0/fast 12s | 2.90 | showpiece; native captions |
| local LTX/Wan | ~0.00 | electricity; slow on consumer GPUs |

## Invariants that don't change

- **No unverified paid render.** Every paid clip passes
  `scripts/check_script_fit.py` regardless of pipeline.
- **Content-addressed caching stays.** A render is keyed by spec content
  hash; revisiting a spec replays the cached MP4. "Cheaper" never means
  "regenerate on every view." Storage is not the cost problem
  (~$0.0001/clip); generation is.
- **Determinism per spec.** Scripts, scene ingredients, and pipeline draw
  all derive from the spec hash — same spec, same video, until the spec
  meaningfully changes.

## Considered and rejected

- **Shell library** (pre-rendered spec-agnostic format loops + per-spec
  TTS swap, ~$0.01/clip): rejected 2026-06-10 (owner call). Without
  bespoke visuals per ticket it loses to the stills pipeline, which is
  nearly as cheap and still generated for *this* spec.
- **"Kill the MP4" purism** (live browser-composited brainrot): the
  zero-cost idea survives as a possible future mix rung, but it is a
  generation-cost play, not a storage play — cached MP4s/assets persist
  either way so revisits never re-pay.
