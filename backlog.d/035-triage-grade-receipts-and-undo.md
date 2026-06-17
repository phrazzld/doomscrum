# Triage-grade dispatch receipts + mis-swipe undo

Priority: P1 · Status: ready · Estimate: L

## Goal
Every dispatch is recoverable and pre-triaged, so swipe-driven dispatch can't
become a firehose of stale, duplicate, or unreviewable PRs.

## Oracle
- [ ] **Undo window:** a right/left swipe enters `queued` with a visible cancel
      affordance for a short window before the worktree/agent starts; cancel
      leaves zero git side-effects (no worktree, branch, or PR).
- [ ] **Plan + size badge:** the dispatched agent writes a one-line plan and a
      diff line-count into the receipt; the feed renders a "fast-merge" vs
      "needs-review" sticker from a size threshold.
- [ ] **Supersede-on-respec:** when a spec's sha256 changes (e.g. after a shape
      PR merges), prior implement receipts for the old sha are badged
      `superseded` rather than left dangling.

## Verification System
- Claim: a mis-swipe is recoverable, and every PR arrives pre-triaged.
- Falsifier: a cancelled swipe still created a worktree/branch/PR; or a large
  diff rendered no needs-review badge; or a re-shaped spec left a live stale receipt.
- Driver: e2e route tests (cancel-within-window; large-stub-diff badge;
  mutate-spec-file supersede) over the existing bare-remote dispatch harness.
- Grader: tests assert no git side-effects on cancel, the correct badge, and the
  `superseded` status flip.
- Evidence packet: test output + feed screenshots of the badges.
- Cadence: dispatch-lifecycle changes.

## Notes
From the groom external-exemplars lane (2026-06-17). Industry baseline: ~46% of
agent PRs are rejected, the largest bucket being relevance/abandonment, not bad
code (arXiv 2606.13468); structural triage at a 20% review budget catches ~69%
of high-effort PRs (arXiv 2601.00753). DoomScrum already has the antidotes
(durable receipts, dedup, one active dispatch per spec/action) — this makes
recoverability + triage signal first-class. Mis-swipe **undo** (not a confirm
modal) is the swipe-feel-preserving fix for fat-finger dispatch; pairs with
[[034-first-dispatch-consent-gate]]. **Not** a dispatch bound: nothing rations
volume or caps autonomy; it adds recoverability and triage signal. Reinforces
the swipe-left **shape** gesture as the quality path (see `docs/VISION.md`).
