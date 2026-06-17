# JIT render lifecycle follow-ups: degrade upgrade + failure recovery

Priority: P2 · Status: ready · Estimate: M

## Goal
Just-in-time viewport rendering (027, shipped) left two lifecycle gaps found in
review: a budget-degraded fixture never upgrades to a real render once the
wallet refills, and a failed JIT render is never retried. Close both so the feed
self-heals instead of rotting until a process restart or a manual `force`.

## Oracle
- [ ] A spec that degraded to a fixture because the wallet was exhausted gets a
      real render the next time it is in the viewport window **and** budget is
      available — without re-rendering it every poll while still over budget.
      Today `prefetch_window` skips any ready render, so a degraded fixture is
      permanent; distinguish `degraded_reason.is_some()` and re-render only when
      `render_plan` would return `Real`. A route test proves the upgrade.
- [ ] A JIT render that FAILED (transient fal/scriptwriter error) is retried on
      a later poll rather than stuck forever in the in-memory `cooking` map
      (its `"failed: …"` entry is never cleared, and `prefetch_window` skips any
      `cooking` key). Use a bounded retry (TTL or attempt cap) that avoids a
      retry storm, and surface persistent failure to the feed.
- [ ] A route-level test proves the budget accumulates mid-window: with a cap
      that affords some-but-not-all of the window, the first N specs render real
      and the rest degrade — covering the per-iteration spend accounting in
      `maybe_prefetch` (today only unit-tested with fixed inputs).

## Notes
From the `/code-review` of PR #5 (027). Reviewers: a native correctness lens
(permanent-degrade + failed-render-stuck) and a codex cross-model pass
(failed-render-stuck). The generate/prefetch double-submit race they also
surfaced was **fixed in PR #5**; these are the deferred operability items.
**Why:** keep the JIT feed self-healing without operator babysitting.
