# Make offline fixtures spec-specific

Priority: P3 · Status: done · Estimate: S

## Goal
The free offline provider produces visibly different clips per spec so demos and tests don't show five identical videos.

## Oracle
- [x] With ffmpeg present, fixture render overlays spec title + format name (drawtext) on distinct background colors per format.
- [x] Without ffmpeg, falls back to the embedded fixture; tests pass in both environments.

## Closure
- Fake provider now attempts an ffmpeg/drawtext-generated, spec-specific fixture and falls back to the embedded MP4 when ffmpeg or drawtext is unavailable.
- Verified fallback with a forced missing ffmpeg command, ffmpeg args for title/format/color, a simulated drawtext-capable ffmpeg invocation, and local CLI fallback on this ffmpeg build with no drawtext filter.
