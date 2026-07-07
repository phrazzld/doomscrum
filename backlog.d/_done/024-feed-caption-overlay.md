# Burn word-synced captions into feed playback for caption-less models

Priority: P2 · Status: absorbed by doomscrum-943 (Powder) · Estimate: M

## Goal
Feed clips stay readable with sound off regardless of video model - overlay
word-synced captions from the persisted caption artifact instead of depending
on native model caption rendering.

## Oracle
- [ ] A muted feed clip rendered on a caption-less model shows the full
      expected script as synced on-screen text.
- [ ] Caption timing comes from the render provenance caption artifact
      (`words[].text/start_ms/end_ms/confidence`) without an extra
      transcription pass at playback time.
- [ ] Caption pages never exceed two lines or the safe text width in the
      feed viewport.

## Notes
Research 2026-06-10: seedance is the only fal model that renders native
on-screen captions; veo3.1/lite ($0.05/s) and sora-2 ($0.10/s) are 2–5x
cheaper but caption-less. Whisper word timestamps already exist for every
verified render (scripts/check_script_fit.py). Overlay options: ffmpeg
ASS subtitles at render-archive time, or a caption track in the feed UI
(assets/index.html). **Why:** unlocks the cheap-model cost path without
losing the sound-off legibility that makes specs readable.

Research 2026-06-13: make the caption artifact provider-neutral rather than
Whisper-specific. Remotion's `Caption` shape is the useful precedent: text,
start/end milliseconds, timestamp, and confidence. Forced alignment is the
primary source when DoomScrum controls TTS; ASR is the fallback for native
audio clips.
