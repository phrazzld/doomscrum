# Stand up CI: every PR gated by fmt, clippy, tests

Priority: P1 · Status: blocked · Estimate: S

## Goal
No branch — human or agent — merges without the gates that run locally.

## Oracle
- [x] GitHub Actions workflow runs cargo fmt --check, clippy -D warnings, cargo test on every PR and main push.
- [ ] Branch protection requires the check; agent-opened PRs (doomscrum/*) show pass/fail.
- [ ] CI completes < 5 min with cargo caching.

## Notes
Agent PRs (left/right swipe output) currently merge on vibes. CI is the floor under the whole dispatch premise.

## Blocker
GitHub branch protection and repository rulesets are unavailable for this private repo on the current account/plan. Both APIs return HTTP 403: "Upgrade to GitHub Pro or make this repository public to enable this feature." The workflow can still run on PRs and main pushes, but GitHub cannot enforce it as a required check until the repo is public or the account/organization has branch protection for private repositories.

## Partial Closure
- Added `.github/workflows/ci.yml` with cargo cache, formatting, clippy, and test steps on PRs and main pushes.
- Formatted the Rust tree so `cargo fmt --check` is enforceable.
