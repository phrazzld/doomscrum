# Triage policy + auto-review for agent-opened PRs

Priority: P2 · Status: pending · Estimate: M

## Goal
doomscrum/* PRs get an automatic first review (tests, scope vs spec, diff size) and a clear human merge/decline queue instead of rotting.

## Oracle
- [ ] Each agent PR receives an automated review comment: gate results + does-the-diff-address-the-spec verdict.
- [ ] Feed shows PR state (open/merged/closed) on the spec card.
- [ ] Stale agent PRs (14d) are flagged in the feed.

## Notes
Groom 2026-06-17: **Gate 2** — this is the loop-closing "moat" in `docs/VISION.md`
(dispatched PRs flowing back as triaged feed state), but it presumes agent PRs
reliably exist, so it is blocked-by the live dispatch proof
([[016-multi-repo-sync]] L3). Its triage-signal half overlaps
[[035-triage-grade-receipts-and-undo]] (plan+size badge, supersede-on-respec):
build the receipt-side signal in 035, the PR-side review/queue here. Industry
baseline: ~46% of agent PRs are rejected, mostly for relevance/abandonment —
this is the make-human-triage-cheap half of that defense.
