# Persist provider-neutral caption artifacts

Priority: P1 · Status: ready · Estimate: S

## Goal

Render provenance stores word-level caption data in a provider-neutral schema
so feed playback, archived MP4 composition, and QA all consume the same timing
truth.

## Oracle

- [x] Render provenance includes `captions.json` with `source`, normalized
      expected/observed text, and `words[].text/start_ms/end_ms/confidence`.
- [x] The schema can represent forced-alignment output, Whisper/FAL word
      timings, and Deepgram-style ASR output without provider-specific fields
      leaking into feed UI code.
- [x] Existing verification can read the artifact instead of re-transcribing
      when the artifact is present and fresh for the render hash.
- [x] At least one fixture proves the schema round-trips into SRT or VTT.

## Notes

Use Remotion's `Caption` type as the external precedent but keep the persisted
shape Rust-owned. Captions are not decorative overlays; they are product data
that must survive provider changes, feed UI rendering, and archive rendering.

## Receipt

- `cargo test providers::tests::caption_artifact -- --nocapture`
- `python3 scripts/check_script_fit.py --caption-artifact /tmp/doomscrum-caption-smoke.E3wJhD/captions.json assets/fixture.mp4 'Ship the demo'`
- `cargo fmt --check`
- `cargo test`

Peer critique: Pi/Kimi flagged timestamp-based artifact freshness as a blocker.
The verifier now reuses a caption artifact only when its `render_sha256`
matches the MP4 bytes. While closing the gate, the browser e2e exposed a
swipe sequencing race; swipes now await the server action before advancing the
card, and the e2e waits for the second card before tapping it.
