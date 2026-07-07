# Stand up CI: every PR gated by fmt, clippy, tests

Priority: P3 · Status: blocked · Estimate: S

## Goal
No branch — human or agent — merges without the gates that run locally.

## Oracle
- [x] GitHub Actions workflow runs cargo fmt --check, clippy -D warnings, cargo test on every PR and main push.
- [ ] Branch protection requires the check; agent-opened PRs (doomscrum/*) show pass/fail.
- [x] CI completes < 5 min with cargo caching.

## Notes
Agent PRs (left/right swipe output) currently merge on vibes. CI is the floor under the whole dispatch premise.

## Blocker
GitHub branch protection and repository rulesets are unavailable for this private repo on the current account/plan. Both APIs return HTTP 403: "Upgrade to GitHub Pro or make this repository public to enable this feature." The workflow can still run on PRs and main pushes, but GitHub cannot enforce it as a required check until the repo is public or the account/organization has branch protection for private repositories.

## Partial Closure
- Added `.github/workflows/ci.yml` with cargo cache, formatting, clippy, and test steps on PRs and main pushes.
- Formatted the Rust tree so `cargo fmt --check` is enforceable.

## Groom reframe (2026-06-17)
The workflow half shipped and is proven: `ci.yml` runs fmt + clippy `-D warnings`
+ test on every PR and on `master` push, green in ~64s this session (well under
the 5-min oracle). Only *enforcement* (a required status check via branch
protection) is blocked — and on an external account/plan decision (make the repo
public, GitHub Pro, or a merge-queue), not on engineering. **Demoted P1 → P3:**
the residual is a one-line owner decision, not a build. The earlier "archive
this, CI is done" read was wrong — the workflow is done, the *enforcement* is
parked. See [[036-agent-contract-truth]] (the agent contract currently
understates this live gate).
