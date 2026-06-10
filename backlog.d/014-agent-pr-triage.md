# Triage policy + auto-review for agent-opened PRs

Priority: P2 · Status: pending · Estimate: M

## Goal
specifi/* PRs get an automatic first review (tests, scope vs spec, diff size) and a clear human merge/decline queue instead of rotting.

## Oracle
- [ ] Each agent PR receives an automated review comment: gate results + does-the-diff-address-the-spec verdict.
- [ ] Feed shows PR state (open/merged/closed) on the spec card.
- [ ] Stale agent PRs (14d) are flagged in the feed.
