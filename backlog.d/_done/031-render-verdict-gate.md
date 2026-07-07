# Promote render verification into a first-class verdict gate

Priority: P1 · Status: ready · Estimate: M

## Goal

Every paid or feed-visible render produces a durable verdict packet that proves
the audio, captions, and legibility are acceptable before the clip can enter
the feed.

## Oracle

- [ ] A single command verifies an MP4 plus expected script and emits a
      machine-readable verdict JSON next to the render.
- [ ] Verdict JSON includes audio presence/loudness, speech-end buffer,
      normalized transcript diff, word coverage, caption confidence floors,
      and caption legibility checks.
- [ ] Paid renders with non-COMPLETE verdicts are not admitted to feed state.
- [ ] The verdict packet includes reviewable artifacts: extracted audio,
      word/caption JSON, transcript diff, and a thumbnail sheet.
- [ ] The verdict checker has a test asserting its own contract against the
      embedded offline fixture — COMPLETE on the good fixture, INCOMPLETE on a
      deliberately-cut one — running in `cargo test` and spending nothing.
      (Groom 2026-06-17, test lane: turns 031 from "a script exists" into "the
      gate is itself verified"; the missing link to [[030-script-eval-harness]].)

## Notes

`scripts/check_script_fit.py` already extracts audio, transcribes, checks
coverage, and can emit word timings. Promote that into the canonical render QA
surface instead of adding a parallel checker. For deterministic TTS, the
strict path is forced alignment against the expected script; for native-audio
models, use ASR and fail if the goal/oracle phrase is missing or low
confidence.

Research 2026-06-13: market tools treat captions/transcripts as editable
timeline data and run production-style QC after generation. DoomScrum needs
the same gate because legibility and intelligibility are product behavior, not
demo polish.
