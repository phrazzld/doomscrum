# The efficiency makeover

How DoomScrum gets cheap enough to point at a real backlog (2026-06-10
research; prices verified on fal that day). The strategies stack — each
multiplies the others.

## The stack

1. **Render the viewport, not the backlog** (shipped 2026-06-16). Most specs
   are never watched, so `feed.prefetch_depth` (default 3) renders just-in-time
   as items approach the top of the feed; deeper specs cost $0 until the cursor
   nears them, and an exhausted wallet degrades to a free fixture rather than
   failing the feed. A unit-count cut (~10x), independent of unit price.
   Promoting a lingered-on spec to a hero pipeline (engagement-driven) is a
   follow-up.
2. **Render-mix portfolio** (shipped — `[[video.mix]]` in doomscrum.toml).
   Each spec deterministically draws a pipeline by content hash: most land
   cheap/short, a weighted few land hero. Current default (stills 6 / veo
   lite 3 / seedance 1) averages **$0.43/clip** vs $1.20 flat sora. Mix
   entries are config — tuning the average is a one-line change.
   *Profiles (shipped 2026-06-10):* `[profiles.<name>]` tables override
   provider/model/mix per context; `profile = "dev"` keeps everyday local
   work on the free fixture provider, `--profile content` flips to the
   paid mix only when iterating on generated media. Dev deliberately stays
   on the fixture (not stills): stills still spends $0.03/image and
   egresses spec text, and everyday iteration must do neither.
3. **Stills pipeline: image + Ken Burns + deterministic audio/captions**
   (shipped 2026-07-15 — `stills/ken-burns`, the heaviest weight in the
   content mix). Keyframe image (seedream v4, $0.03) + local ffmpeg Ken
   Burns motion + local TTS (macOS `say` by default, configurable
   `tts_cmd`) + estimated word-synced caption artifact ≈ **$0.03/clip**,
   fully bespoke per spec. The script, audio, and captions are the
   controlled layer; video models are only responsible for visuals unless
   a hero mix entry explicitly opts into native audio. Config:
   `[video.stills]` (docs/CONFIGURATION.md). Forced alignment and an
   engagement-promoted hero upgrade remain follow-ups. See
   docs/VIDEO_QUALITY_PIPELINE.md.
4. **Local open weights** (backlog 028). LTX-2 19B distilled FP8 runs on
   16GB consumer GPUs; Wan 1.3B does 5s/720p in ~45s on a 4090. Marginal
   cost ≈ electricity. The audience is developers — many own the
   hardware. `provider = "local"` makes the wallet cap irrelevant for
   them; fal-rented H100s ($1.89/hr) cover batch queues without hardware.
5. **Commercial: tinder economics** (backlog 029). N free swipes/day;
   paid tiers buy more swipes and hero-render weight. Free tier rides the
   cheapest pipelines; BYOK fal key or bundled credits carry the premium
   ones. Vendor COGS per free user ≈ pennies under strategies 1–4.

## Pipeline price ladder

Base vendor prices were verified on 2026-06-10; the deterministic
audio/caption routing decision was refreshed on 2026-06-13. Exact provider
prices must still be quoted at render time.

| Pipeline | $/clip | Notes |
|---|---|---|
| stills + Ken Burns + deterministic TTS/captions | 0.03 | shipped 2026-07-15 (`stills/ken-burns`); fully bespoke |
| cheap silent/visual video + deterministic TTS/captions | ~0.25-0.80 | motion upgrade; avoids native-audio transcript risk |
| ltx-2.3 fast 8s 1080p | 0.32 | native audio; diction unverified |
| veo3.1/lite 8s 720p | 0.40 | captions garble; no criterion at 8s |
| sora-2 12s | 1.20 | REMOVED 2026-07-15: endpoint deprecated on fal |
| seedance-2.0/fast 12s | 2.90 | showpiece; native captions |
| local LTX/Wan | ~0.00 | electricity; slow on consumer GPUs |

## Invariants that don't change

- **No unverified paid render.** Every paid clip passes the render verdict
  gate (`scripts/check_script_fit.py` today, backlog 031 after promotion)
  regardless of pipeline.
- **Captions are product data, not model decoration.** Persist a word-level
  caption artifact keyed to the expected script. Feed playback and archive
  renders consume that artifact instead of trusting native model captions.
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
