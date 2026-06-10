# Make offline fixtures spec-specific

Priority: P3 · Status: pending · Estimate: S

## Goal
The free offline provider produces visibly different clips per spec so demos and tests don't show five identical videos.

## Oracle
- [ ] With ffmpeg present, fixture render overlays spec title + format name (drawtext) on distinct background colors per format.
- [ ] Without ffmpeg, falls back to the embedded fixture; tests pass in both environments.
