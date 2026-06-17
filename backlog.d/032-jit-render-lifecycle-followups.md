# Render/dispatch crash recovery: reconcile-on-boot + in-process self-healing

Priority: P2 · Status: ready · Estimate: L

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
- [ ] **Reconcile-on-boot:** `Serve` rebuilds `cooking`/`render_reservations`/
      dispatch-status from disk truth instead of starting empty (server.rs:68-71).
      A render that survived a crash on fal's side is adopted by
      `provider_job_id`, not re-billed; a stranded `agent_running`/`queued`
      dispatch is marked `failed` (or resumed), so GC stops protecting an
      orphaned worktree (gc.rs:229-234) and the feed stops showing a status that
      never advances.
- [ ] **Panic-safe detached jobs + timeout:** a panicked render/dispatch task
      releases its reservation and records failure (no silent swallow); the fal
      `reqwest::Client` has a request timeout (fal.rs:208, today unbounded) so a
      hung connection can't pin a reservation past the 20-min poll ceiling.
- [ ] The failed-render retry (bullet 2) test asserts the specific mechanism:
      the `cooking` `"failed: …"` key is cleared and the spec re-attempted at the
      route level (not just "retried on a later poll").

## Notes
From the `/code-review` of PR #5 (027). Reviewers: a native correctness lens
(permanent-degrade + failed-render-stuck) and a codex cross-model pass
(failed-render-stuck). The generate/prefetch double-submit race they also
surfaced was **fixed in PR #5**; these are the deferred operability items.
**Why:** keep the JIT feed self-healing without operator babysitting.

Groom 2026-06-17 (runtime-reliability lane): widened from "JIT follow-ups" to a
crash-recovery epic. The original two gaps were framed as in-process self-healing,
but the dominant failure mode is a **restart**: `cooking`, reservations, AND the
dispatch queue are all in-memory with zero startup reconciliation, so a crash
mid-render/dispatch strands work permanently (orphaned worktrees, frozen
`agent_running`, abandoned-or-re-billed fal jobs). Estimate M → L. Overlaps the
recovery-runbook oracle in [[036-agent-contract-truth]] (once reconcile-on-boot
lands, the runbook just documents it).
