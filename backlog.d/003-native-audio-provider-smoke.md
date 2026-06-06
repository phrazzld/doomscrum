# Native Audio Provider Smoke

## User
Developer validating the video-generation provider.

## Problem
Some video models return silent clips or change audio support without warning.

## Goal
Add a provider smoke that records whether native audio was requested and what audio mode the render actually produced.

## Acceptance Criteria
- Render JSON contains `nativeAudioRequested` and `audioMode`.
- Provider model and job id are recorded.
- Missing credentials produce a clear waiver, not a fake pass.

## Risk
Provider pricing and moderation behavior can drift.
