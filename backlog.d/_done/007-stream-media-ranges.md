# Stream media instead of reading whole files per range request

Priority: P2 · Status: done · Estimate: S

## Goal
A 15MB render served to a looping video element does not re-read the full file for every Range request.

## Oracle
- [x] /media handler seeks and streams only the requested byte range.
- [x] Memory stays flat while a client loops a video for 60s (no full-file allocations per request).

## Notes
Current handler does fs::read then slices. Fine at 5 renders; wrong shape for a real feed.

## Completion
Implemented 2026-06-12: `/media/{sha}/{file}` now uses file metadata,
`tokio::fs::File`, `seek`, and `ReaderStream` instead of `std::fs::read`.
`tests/server.rs::feed_renders_and_serves_video` verifies browser-facing
range behavior with `Range: bytes=4-7`, `206`, `Content-Range`, and the exact
MP4 `ftyp` bytes.
