# Burn word-synced captions into feed playback for caption-less models

Priority: P2 · Status: pending · Estimate: M

## Goal
Feed clips stay readable with sound off regardless of video model — overlay
word-synced captions from whisper timestamps instead of depending on
seedance's native caption rendering.

## Oracle
- [ ] A muted feed clip rendered on a non-seedance model shows the full
      spoken script as synced on-screen text.
- [ ] Caption timing comes from the existing whisper verification pass
      (no second transcription spend).

## Notes
Research 2026-06-10: seedance is the only fal model that renders native
on-screen captions; veo3.1/lite ($0.05/s) and sora-2 ($0.10/s) are 2–5x
cheaper but caption-less. Whisper word timestamps already exist for every
verified render (scripts/check_script_fit.py). Overlay options: ffmpeg
ASS subtitles at render-archive time, or a caption track in the feed UI
(assets/index.html). **Why:** unlocks the cheap-model cost path without
losing the sound-off legibility that makes specs readable.
