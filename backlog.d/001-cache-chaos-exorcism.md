# Cache Chaos Exorcism

Priority: P2 · Status: in-progress · Estimate: M

Note: sharpened spec lives in PR #1 (specifi/shape-cache-chaos-exorcism-1d3c78d189); merge or fold back before implementing.

## User
Operators reviewing agent-delivered web app changes.

## Problem
The local preview sometimes shows stale render data after a provider smoke.

## Goal
Add a cache busting path for generated render metadata so the gallery always shows the latest MP4 provenance.

## Acceptance Criteria
- Gallery refresh shows the newest render after generation.
- Old render JSON is preserved for audit.
- No source PRD file is modified.

## Risk
Could hide a provider failure if stale successful render data remains selected.
