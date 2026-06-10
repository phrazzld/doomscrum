# Stream media instead of reading whole files per range request

Priority: P2 · Status: pending · Estimate: S

## Goal
A 15MB render served to a looping video element does not re-read the full file for every Range request.

## Oracle
- [ ] /media handler seeks and streams only the requested byte range.
- [ ] Memory stays flat while a client loops a video for 60s (no full-file allocations per request).

## Notes
Current handler does fs::read then slices. Fine at 5 renders; wrong shape for a real feed.
