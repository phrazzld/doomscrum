# Close the loop: turn captured outcomes into spec-readiness signal

Priority: P1 · Status: pending · Estimate: XL (epic — dispatch via ordered children)

## Goal
The three signals DoomScrum already captures — vibe ratings, PR merge/close
outcomes, and shape passes — stop being write-only telemetry and start steering
the feed, via a per-spec **readiness score** that ranks agent-ready specs higher
and sinks chronically-rejected ones. This is the VISION moat ("receipts and vibe
ratings teach which specs are agent-ready") made real. Source specs stay
authoritative; all learning lives in destroyable `.doomscrum/` state.

## Oracle (whole-arc)
- [ ] A merged PR for a spec, and a high vibe rating on its render, both
      measurably **raise** that spec's readiness; a closed-unmerged PR **lowers**
      it — proven by a test that feeds events in and asserts the score delta and
      resulting feed position.
- [ ] The feed is ordered by readiness score, not filename: two specs identical
      except for outcome history → the higher-scoring one renders first in
      `/api/state`. Filename order remains the deterministic tiebreaker.
- [ ] PR outcome (open/merged/closed) is reconciled from the real remote (`gh`)
      into receipt/feed state on a poll, and shown on the card — visibility no
      longer ends at the link (this is the build half of [[014-agent-pr-triage]]).
- [ ] Scoring is deterministic per `(spec sha, event set)`: same inputs → same
      score, recomputed from `events.ndjson` + receipts, never persisted as
      authority. Re-shaping a spec (new sha) resets its implement-outcome history.
- [ ] **No dispatch bounds introduced.** Ranking changes order and surfacing
      only; every spec stays swipeable, nothing is hidden or gated by score.
      Wallet caps unchanged.
- [ ] Deleting `.doomscrum/` destroys all learning and leaves specs untouched.

## Verification System
- Claim: captured outcomes (vibe, PR result, shape) reorder the feed toward
  agent-ready specs without ever bounding dispatch or mutating a source spec.
- Falsifier: a merged-PR spec that does NOT rise above an identical never-shipped
  spec; or any score path that hides/gates a spec; or a source `.md` mutated by
  scoring.
- Driver: a test that appends a scripted event sequence (vibe + pr_state +
  shape) to a temp `.doomscrum/` and reads `/api/state` order back.
- Grader: asserted score deltas + feed positions + spec-file byte-identity.
- Evidence packet: the test's before/after `/api/state` ordering + a deleted-
  `.doomscrum/` run showing specs intact.
- Cadence: per-child failing test first; full sequence test at the last child.

## Children (ordered, ~1 agent-hour each)
1. **PR-outcome reconciliation.** Opt-in reconcile pass reads `pr_opened`
   receipts, queries the remote (`gh pr view`) for state (OPEN/MERGED/CLOSED),
   persists `pr_state` + `pr_state_at` onto the receipt. Mock the `gh` boundary.
2. **Surface PR state on the card.** Plumb `pr_state` through `/api/state`
   (`server.rs`) and render a merged/closed/open sticker in `assets/index.html`.
   Satisfies [[014-agent-pr-triage]]'s "feed shows PR state" oracle.
3. **Readiness-score function (pure, tested).** New `readiness.rs`:
   `score(spec, &[Event], &[DispatchReceipt]) -> f64` folding PR outcome
   (merged ++, closed −−), vibe (weight LOW — it tracks render quality more than
   spec readiness), and shape-then-merge (+). Deterministic, no I/O.
4. **Rank the feed by score.** `api_state` sorts by descending readiness,
   filename order as tiebreaker; keep `priority` as raw filename index for
   provenance. Test: identical-except-history specs sort by score.
5. **Consume shape outcomes.** Make `dispatch_shape` a first-class scoring input
   (today read by nothing): a shaped spec that later merges credits the shape
   pass; a re-shaped sha resets stale implement history.
6. **Legible provenance.** Small readiness indicator on the card so the operator
   sees *why* order changed; confirm `.doomscrum/` deletion zeroes learning;
   end-to-end ordering test.

## Notes
**Why:** the moat is the closed loop, and it is open. `latest_vibe_rating`
(`server.rs:627`) and `dispatch_shape` (`server.rs:969`) write signals that only
one cosmetic display path (`server.rs:580`) or no path reads; feed order is
hardwired to `files.sort()` (`backlog.rs:54`); PR visibility terminates at
`pr_opened` (`dispatch.rs:436`) with no reconciliation anywhere in `src/`.
[[014-agent-pr-triage]] names the PR-state-on-card half but not the learning
half this epic builds; children 1–2 here are that ticket's build, so 014 should
fold in (proposal). Consumes the shipped vibe-capture (done-005) rather than
duplicating it.

**Sequencing / dependency:** children 1–2 are only *live*-provable once a real
agent-PR exists on a real remote (the Gate-0 keystone, [[016-multi-repo-sync]]
child-3) — until then they are testable against a mocked `gh` boundary. This is
the moat (Gate 2 in `VISION.md`) but it de-risks cheaply now via the pure
score function (child 3) which needs no live PR.

**Caveat:** vibe ≈ render quality, not spec readiness — weight it below PR
outcome so the loop doesn't learn the wrong lesson.
