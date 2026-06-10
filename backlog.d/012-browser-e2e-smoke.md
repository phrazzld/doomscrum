# Browser-level E2E: gestures, overlay, dispatch dry-run

Priority: P1 · Status: ready · Estimate: M

## Goal
The actual swipe surface (pointer gestures, spec overlay, sound gate, status stickers) is exercised by an automated browser, not just HTTP tests.

## Oracle
- [ ] Headless browser test: tap-in, swipe up (skip recorded), tap (overlay shows exact spec), swipe right with a stub agent command (dispatch reaches pr_opened/completed_local).
- [ ] Runs in CI behind the same stub-agent config the Rust tests use.

## Notes
Lesson from the Node era: the e2e suite once spawned a REAL codex run because dry-run env didn't reach the server. Stub via config file, never env inheritance.
