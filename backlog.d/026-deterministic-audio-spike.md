# Stills pipeline: keyframe image + Ken Burns + TTS (~$0.05/clip)

Priority: P1 · Status: ready · Estimate: L

## Goal
A first-class render pipeline that composes a bespoke AI keyframe image,
free Ken Burns/parallax motion, TTS voiceover, and our word-synced caption
overlay into a feed clip for ~$0.05 — eligible as a `[[video.mix]]` entry.

## Oracle
- [ ] One feed clip produced end-to-end for under $0.10 with a COMPLETE
      verdict from scripts/check_script_fit.py on the first roll.
- [ ] The image is generated from the spec's scene prompt (same seeded
      ingredients as the video formats) — bespoke per ticket, not stock.
- [ ] Render provenance records image + TTS + composition costs separately.
- [ ] Pipeline is selectable from the render mix like any model.

## Notes
Strategy 3 in docs/EFFICIENCY.md. Industry pipelines land at $0.05-0.08
per 60s using exactly this stack (image ~$0.01-0.03 via Seedream/Flux,
TTS ~$0.01 via Kokoro/OpenAI, motion + captions free via ffmpeg/Remotion
math we already own — whisper word timings exist for caption styling).
Deterministic audio kills both paid re-roll classes (silent duds, garbled
diction). Upgraded from a spike to a pipeline ticket per owner call
2026-06-10; the shell-library alternative was rejected the same day
(bespoke-ness is the product). **Why:** the cheapest fully-bespoke rung
of the efficiency ladder.
