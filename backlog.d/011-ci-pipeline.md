# Stand up CI: every PR gated by fmt, clippy, tests

Priority: P1 · Status: ready · Estimate: S

## Goal
No branch — human or agent — merges without the gates that run locally.

## Oracle
- [ ] GitHub Actions workflow runs cargo fmt --check, clippy -D warnings, cargo test on every PR and main push.
- [ ] Branch protection requires the check; agent-opened PRs (doomscrum/*) show pass/fail.
- [ ] CI completes < 5 min with cargo caching.

## Notes
Agent PRs (left/right swipe output) currently merge on vibes. CI is the floor under the whole dispatch premise.
