---
name: demo
description: >-
  Project-local demo path for DoomScrum: recut the Remotion marketing video
  from verified feed renders and deliver MP4s to the Desktop. Use when:
  "recut the demo", "regenerate the demo", "demo video", "make the demo
  harder/faster/more brainrot". Trigger: /demo.
---

# demo (DoomScrum)

The demo is a Remotion comp (`demo/src/Demo.tsx`, comp id `DoomScrumDemo`)
cut from verified feed renders. Energy target: 90s-infomercial ×
zoomer-brainrot hypermaximalism — never a static frame, captions readable
with sound off.

## Timing law

A clip scene lasts (whisper speech-end + ~0.7s held beat), never less.
Measure speech-end with `scripts/check_script_fit.py` per clip; hardcode the
measured numbers in the scene-duration block of `Demo.tsx`. Acceptance for
the final cut is transcribing the rendered MP4 end-to-end with the same
script — verdict must be COMPLETE.

## Brainrot FX layer conventions

- Karaoke captions driven by the whisper word timestamps already produced
  during clip verification — don't re-transcribe.
- Punch-in zooms: `spring()` scale at every scene start; shake via
  `random({seed: frame})` jitter, amplitude ≤6px.
- Impact frames: 1–2 frame white/invert `<AbsoluteFill>` flashes on cuts.
- Meme SFX (vine boom, airhorn) as staggered `<Audio>` at cut points;
  keep VO intelligible — duck SFX during spoken lines.
- Infomercial gags: starburst callouts, price-slash text, "operators are
  standing by" crawl. VHS/CRT grain only on the cold open and close.

## Build & deliver

```sh
cd demo && npm run render          # out/doomscrum-demo.mp4 (master)
ffmpeg -y -i out/doomscrum-demo.mp4 -crf 27 -movflags +faststart \
  ~/Desktop/doomscrum-demo-share.mp4
cp out/doomscrum-demo.mp4 ~/Desktop/doomscrum-demo.mp4
```

Assets are copies: `.doomscrum/renders/<id>/*.mp4` → `demo/public/<slug>.mp4`.
After any feed regen, re-copy assets and re-measure speech ends before
rendering. Contact-sheet sanity check:
`ffmpeg -i out/doomscrum-demo.mp4 -vf "fps=1/5,scale=320:-1,tile=8x2" sheet.png`.
