# Persist provider-neutral caption artifacts

Priority: P1 · Status: ready · Estimate: S

## Goal

Render provenance stores word-level caption data in a provider-neutral schema
so feed playback, archived MP4 composition, and QA all consume the same timing
truth.

## Oracle

- [ ] Render provenance includes `captions.json` with `source`, normalized
      expected/observed text, and `words[].text/start_ms/end_ms/confidence`.
- [ ] The schema can represent forced-alignment output, Whisper/FAL word
      timings, and Deepgram-style ASR output without provider-specific fields
      leaking into feed UI code.
- [ ] Existing verification can read the artifact instead of re-transcribing
      when the artifact is present and fresh for the render hash.
- [ ] At least one fixture proves the schema round-trips into SRT or VTT.

## Notes

Use Remotion's `Caption` type as the external precedent but keep the persisted
shape Rust-owned. Captions are not decorative overlays; they are product data
that must survive provider changes, feed UI rendering, and archive rendering.
